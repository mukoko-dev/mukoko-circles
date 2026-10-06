//! The fetch handler: route the request, call the Nyuchi API, fill the Astro
//! shells, and set the headers. Everything it decides is in the native
//! modules; this file is the I/O around them.

use crate::ap;
use crate::api::{self, Discover};
use crate::config::Config;
use crate::model::{Category, Circle, Page, Post};
use crate::og;
use crate::pages::{self, ListPage, Templates};
use crate::route::{Route, route};
use crate::sitemap;
use futures_util::future::join3;
use serde::de::DeserializeOwned;
use std::cell::RefCell;
use worker::*;

const CSP: &str = "default-src 'self'; img-src 'self' https: data:; style-src 'self'; font-src 'self'; script-src 'none'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; upgrade-insecure-requests";

thread_local! {
    /// The shells, read once per isolate. They only change with a deploy,
    /// and a deploy is a new isolate.
    static TEMPLATES: RefCell<Option<Templates>> = const { RefCell::new(None) };
}

fn var(env: &Env, name: &str) -> Option<String> {
    env.var(name)
        .ok()
        .map(|v| v.to_string())
        .filter(|v| !v.trim().is_empty())
}

fn secret(env: &Env, name: &str) -> Option<String> {
    env.secret(name)
        .ok()
        .map(|v| v.to_string())
        .filter(|v| !v.trim().is_empty())
}

fn config(env: &Env) -> Config {
    let mut cfg =
        Config::new(&var(env, "SITE_URL").unwrap_or_else(|| "https://circles.mukoko.com".into()));
    cfg.api_url = var(env, "NYUCHI_API_URL");
    if let Some(v) = var(env, "JOIN_URL_TEMPLATE") {
        cfg.join_url_template = v;
    }
    if let Some(v) = var(env, "WEB_JOIN_URL_TEMPLATE") {
        cfg.web_join_url_template = v;
    }
    if let Some(v) = var(env, "CREATE_URL") {
        cfg.create_url = v;
    }
    if let Some(v) = var(env, "APP_WEB_URL") {
        cfg.app_web_url = v;
    }
    cfg.app_store_url = var(env, "APP_STORE_URL");
    cfg.play_store_url = var(env, "PLAY_STORE_URL");
    cfg
}

/// What a call to the API came back with.
enum Upstream<T> {
    Ok(T),
    NotFound,
    /// Not configured, unreachable, a 5xx, a 501, or a body we cannot parse.
    Unavailable(String),
}

async fn api_get<T: DeserializeOwned>(env: &Env, url: &str) -> Upstream<T> {
    let headers = Headers::new();
    let _ = headers.set("accept", "application/json");
    let _ = headers.set(
        "user-agent",
        "mukoko-circles/0.1 (+https://circles.mukoko.com)",
    );
    // First-party apps call the Nyuchi API with their own client pair. The
    // discovery endpoint is public, so the pair is optional: it raises rate
    // limits and identifies the caller, and nothing breaks without it.
    if let (Some(id), Some(sec)) = (
        secret(env, "NYUCHI_API_CLIENT_ID"),
        secret(env, "NYUCHI_API_CLIENT_SECRET"),
    ) {
        let _ = headers.set("x-client-id", &id);
        let _ = headers.set("x-client-secret", &sec);
    }
    let mut cf = CfProperties::new();
    cf.cache_ttl = Some(60);
    let mut init = RequestInit::new();
    init.with_headers(headers).with_cf_properties(cf);
    let req = match Request::new_with_init(url, &init) {
        Ok(r) => r,
        Err(e) => return Upstream::Unavailable(e.to_string()),
    };
    let mut resp = match Fetch::Request(req).send().await {
        Ok(r) => r,
        Err(e) => return Upstream::Unavailable(format!("fetch failed: {e}")),
    };
    match resp.status_code() {
        200..=299 => match resp.json::<T>().await {
            Ok(v) => Upstream::Ok(v),
            Err(e) => Upstream::Unavailable(format!("unexpected response: {e}")),
        },
        404 => Upstream::NotFound,
        s => Upstream::Unavailable(format!("HTTP {s}")),
    }
}

async fn templates(env: &Env, origin: &str) -> Result<Templates> {
    if let Some(t) = TEMPLATES.with(|t| t.borrow().clone()) {
        return Ok(t);
    }
    let assets = env.assets("ASSETS")?;
    let mut files: [String; 8] = Default::default();
    for (i, path) in Templates::PATHS.iter().enumerate() {
        let mut r = assets.fetch(format!("{origin}{path}"), None).await?;
        if r.status_code() != 200 {
            return Err(Error::RustError(format!(
                "{path} is missing from the build ({}): run the Astro build",
                r.status_code()
            )));
        }
        files[i] = r.text().await?;
    }
    let t = Templates::from_files(files);
    TEMPLATES.with(|c| *c.borrow_mut() = Some(t.clone()));
    Ok(t)
}

fn secure(mut resp: Response) -> Response {
    let h = resp.headers_mut();
    let _ = h.set("x-content-type-options", "nosniff");
    let _ = h.set("referrer-policy", "strict-origin-when-cross-origin");
    let _ = h.set(
        "permissions-policy",
        "camera=(), microphone=(), geolocation=(), interest-cohort=()",
    );
    let _ = h.set(
        "strict-transport-security",
        "max-age=31536000; includeSubDomains",
    );
    resp
}

fn html(body: String, status: u16, cache: &str) -> Result<Response> {
    let mut r = Response::from_html(body)?.with_status(status);
    let h = r.headers_mut();
    h.set("content-security-policy", CSP)?;
    h.set("cache-control", cache)?;
    Ok(secure(r))
}

fn json_response(v: &serde_json::Value, status: u16, content_type: &str) -> Result<Response> {
    let mut r = Response::from_bytes(serde_json::to_vec(v)?)?.with_status(status);
    let h = r.headers_mut();
    h.set("content-type", content_type)?;
    h.set("access-control-allow-origin", "*")?;
    h.set(
        "cache-control",
        if status == 200 {
            "public, max-age=300"
        } else {
            "no-store"
        },
    )?;
    Ok(secure(r))
}

fn activity(v: &serde_json::Value, status: u16) -> Result<Response> {
    let mut r = json_response(v, status, "application/activity+json; charset=utf-8")?;
    r.headers_mut().set("vary", "Accept")?;
    Ok(r)
}

fn json_error(status: u16, error: &str, message: &str) -> Result<Response> {
    json_response(
        &serde_json::json!({ "error": error, "message": message }),
        status,
        "application/json; charset=utf-8",
    )
}

fn redirect(to: &str, status: u16) -> Result<Response> {
    // Built by hand: `Response::redirect` returns immutable headers.
    let url = Url::parse(to).map_err(|e| Error::RustError(e.to_string()))?;
    let h = Headers::new();
    h.set("location", url.as_str())?;
    h.set("cache-control", "public, max-age=60")?;
    Ok(secure(
        Response::empty()?.with_status(status).with_headers(h),
    ))
}

fn png(bytes: Vec<u8>) -> Result<Response> {
    let mut r = Response::from_bytes(bytes)?;
    let h = r.headers_mut();
    h.set("content-type", "image/png")?;
    h.set("cache-control", "public, max-age=86400")?;
    Ok(secure(r))
}

/// Shared page-level answers.
struct Ctx<'a> {
    env: &'a Env,
    cfg: Config,
    t: Templates,
    path: String,
}

impl Ctx<'_> {
    fn not_found(&self) -> Result<Response> {
        html(
            pages::message(
                &self.t,
                &self.cfg,
                &self.path,
                "404",
                "We couldn't find that circle",
                "It may have been renamed, made private, or never existed. Open circles are all listed on the home page.",
            ),
            404,
            "public, max-age=60",
        )
    }

    fn unavailable(&self, why: &str) -> Result<Response> {
        console_error!("upstream unavailable on {}: {why}", self.path);
        let mut r = html(
            pages::message(
                &self.t,
                &self.cfg,
                &self.path,
                "503",
                "Circles are taking a breather",
                "We can't reach the Mukoko directory right now. Please try again in a minute.",
            ),
            503,
            "no-store",
        )?;
        r.headers_mut().set("retry-after", "60")?;
        Ok(r)
    }

    fn api(&self) -> std::result::Result<&str, String> {
        self.cfg
            .api_url
            .as_deref()
            .ok_or_else(|| "NYUCHI_API_URL is not configured on this Worker".into())
    }

    async fn circle(&self, slug: &str) -> Upstream<Circle> {
        let api = match self.api() {
            Ok(a) => a,
            Err(e) => return Upstream::Unavailable(e),
        };
        match api_get::<Circle>(self.env, &api::circle(api, slug)).await {
            // Defence in depth: never show or federate a circle that is not
            // discoverable, or one that does not answer to the name asked
            // for (its handle, its slug, or a retired handle).
            Upstream::Ok(c) if c.is_discoverable() && c.answers_to(slug) => Upstream::Ok(c),
            Upstream::Ok(_) => Upstream::NotFound,
            other => other,
        }
    }

    async fn discover(&self, d: Discover<'_>) -> Upstream<Page<Circle>> {
        let api = match self.api() {
            Ok(a) => a,
            Err(e) => return Upstream::Unavailable(e),
        };
        match api_get::<Page<Circle>>(self.env, &api::discover(api, &d)).await {
            Upstream::Ok(p) => Upstream::Ok(p.discoverable()),
            other => other,
        }
    }

    async fn categories(&self) -> Vec<Category> {
        let Ok(api) = self.api() else {
            return Vec::new();
        };
        match api_get::<Page<Category>>(self.env, &api::categories(api)).await {
            Upstream::Ok(p) => p.data,
            _ => Vec::new(),
        }
    }

    async fn posts(&self, slug: &str, limit: u32, cursor: Option<&str>) -> Upstream<Page<Post>> {
        let api = match self.api() {
            Ok(a) => a,
            Err(e) => return Upstream::Unavailable(e),
        };
        api_get(self.env, &api::posts(api, slug, limit, cursor)).await
    }
}

async fn og_cached(
    key: &str,
    ctx: &Context,
    draw: impl FnOnce() -> std::result::Result<Vec<u8>, String>,
) -> Result<Response> {
    let cache = Cache::default();
    if let Some(hit) = cache.get(key, false).await? {
        return Ok(hit);
    }
    let bytes = draw().map_err(Error::RustError)?;
    let mut resp = png(bytes)?;
    let copy = resp.cloned()?;
    let key = key.to_string();
    ctx.wait_until(async move {
        if let Err(e) = Cache::default().put(key.as_str(), copy).await {
            console_error!("og cache put failed: {e}");
        }
    });
    Ok(resp)
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, wctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path = url.path().to_string();
    let query = url.query().unwrap_or("").to_string();
    let r = route(&path, &query);

    if r == Route::Asset {
        let resp = env.assets("ASSETS")?.fetch_request(req).await?;
        // Re-headed so static pages carry the same security headers as the
        // rendered ones (the asset server's own headers are kept).
        let h = Headers::new();
        for (k, v) in resp.headers().entries() {
            h.set(&k, &v)?;
        }
        let is_html = h
            .get("content-type")?
            .is_some_and(|c| c.starts_with("text/html"));
        if is_html {
            h.set("content-security-policy", CSP)?;
        }
        return Ok(secure(resp.with_headers(h)));
    }

    let cfg = config(&env);
    let method = req.method();

    // The inbox answers every method, honestly.
    if let Route::Inbox { slug } = &r {
        let join = match slug {
            Some(s) => format!("{}/c/{s}/join", cfg.site_url),
            None => cfg.app_web_url.clone(),
        };
        let mut resp = json_response(
            &ap::inbox_not_implemented(&join),
            501,
            "application/json; charset=utf-8",
        )?;
        resp.headers_mut().set("allow", "GET, HEAD")?;
        return Ok(resp);
    }
    if !matches!(method, Method::Get | Method::Head) {
        let mut resp = json_error(
            405,
            "method_not_allowed",
            "Only GET and HEAD are served here.",
        )?;
        resp.headers_mut().set("allow", "GET, HEAD")?;
        return Ok(resp);
    }

    // Machine-readable endpoints that need no shell.
    match &r {
        Route::NodeinfoLinks => {
            return json_response(
                &ap::nodeinfo_links(&cfg),
                200,
                "application/json; charset=utf-8",
            );
        }
        Route::HostMeta => {
            let xrd = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<XRD xmlns=\"http://docs.oasis-open.org/ns/xri/xrd-1.0\">\n  <Link rel=\"lrdd\" type=\"application/jrd+json\" template=\"{}/.well-known/webfinger?resource={{uri}}\"/>\n</XRD>\n",
                cfg.site_url
            );
            let mut resp = Response::ok(xrd)?;
            let h = resp.headers_mut();
            h.set("content-type", "application/xrd+xml; charset=utf-8")?;
            h.set("access-control-allow-origin", "*")?;
            h.set("cache-control", "public, max-age=86400")?;
            return Ok(secure(resp));
        }
        Route::Create => return redirect(&cfg.create_url, 302),
        Route::Lowercase { location } => {
            return redirect(&format!("{}{location}", cfg.site_url), 301);
        }
        Route::OgHome => {
            let key = format!("{}/og/home.png?v={}", cfg.site_url, cfg.version);
            let content = og::home_content(&cfg);
            return og_cached(&key, &wctx, || og::render_png(&content)).await;
        }
        _ => {}
    }

    let origin = url.origin().ascii_serialization();
    let t = match templates(&env, &origin).await {
        Ok(t) => t,
        Err(e) => {
            console_error!("templates: {e}");
            return json_error(
                500,
                "build_incomplete",
                "The page shells are missing from this deploy.",
            );
        }
    };
    let ctx = Ctx {
        env: &env,
        cfg,
        t,
        path: path.clone(),
    };
    let cfg = &ctx.cfg;
    let wants_ap = ap::wants_activity_json(req.headers().get("accept")?.as_deref());

    match r {
        Route::Hidden | Route::NotFound => ctx.not_found(),

        Route::Home => {
            let (featured, latest, cats) = join3(
                ctx.discover(Discover {
                    featured: true,
                    limit: 6,
                    ..Default::default()
                }),
                ctx.discover(Discover {
                    limit: 12,
                    ..Default::default()
                }),
                ctx.categories(),
            )
            .await;
            let latest = match latest {
                Upstream::Ok(p) => p,
                Upstream::NotFound => Page::default(),
                Upstream::Unavailable(e) => return ctx.unavailable(&e),
            };
            let featured = match featured {
                Upstream::Ok(p) if !p.data.is_empty() => p.data,
                // Nothing featured yet: lead with the biggest circles.
                _ => latest.data.iter().take(6).cloned().collect(),
            };
            html(
                pages::home(&ctx.t, cfg, &featured, &latest, &cats),
                200,
                "public, max-age=60",
            )
        }

        Route::All { cursor } => {
            let (page, cats) = futures_util::future::join(
                ctx.discover(Discover {
                    limit: 24,
                    cursor: cursor.as_deref(),
                    ..Default::default()
                }),
                ctx.categories(),
            )
            .await;
            let page = match page {
                Upstream::Ok(p) => p,
                Upstream::NotFound => Page::default(),
                Upstream::Unavailable(e) => return ctx.unavailable(&e),
            };
            html(
                pages::list(&ctx.t, cfg, ListPage {
                    path: "/circles",
                    eyebrow: "Every open circle".into(),
                    heading: "All circles".into(),
                    lead: "Every public and broadcast circle on Mukoko, biggest first. Join one in the Mukoko app, or follow it from the fediverse.".into(),
                    q: "",
                    page: &page,
                    next_base: "/circles".into(),
                    categories: &cats,
                    // Later pages are reachable from the first and the sitemap.
                    noindex: cursor.is_some(),
                    breadcrumb: Some(("All circles", "/circles")),
                    current_category: None,
                }),
                200,
                "public, max-age=60",
            )
        }

        Route::Category { slug, cursor } => {
            let (page, cats) = futures_util::future::join(
                ctx.discover(Discover {
                    category: Some(&slug),
                    limit: 24,
                    cursor: cursor.as_deref(),
                    ..Default::default()
                }),
                ctx.categories(),
            )
            .await;
            let page = match page {
                Upstream::Ok(p) => p,
                Upstream::NotFound => return ctx.not_found(),
                Upstream::Unavailable(e) => return ctx.unavailable(&e),
            };
            let cat = cats.iter().find(|c| c.slug == slug);
            if cat.is_none() && page.data.is_empty() {
                return ctx.not_found();
            }
            let name = cat
                .map(|c| c.name.clone())
                .or_else(|| {
                    page.data
                        .iter()
                        .flat_map(|c| c.categories.iter())
                        .find(|c| c.slug == slug)
                        .map(|c| c.name.clone())
                })
                .unwrap_or_else(|| slug.replace('-', " "));
            let lead = cat
                .and_then(|c| c.description.clone())
                .filter(|d| !d.trim().is_empty())
                .unwrap_or_else(|| {
                    format!("Open circles on Mukoko about {}.", name.to_lowercase())
                });
            let path = format!("/categories/{slug}");
            html(
                pages::list(
                    &ctx.t,
                    cfg,
                    ListPage {
                        path: &path,
                        eyebrow: "Category".into(),
                        heading: name.clone(),
                        lead,
                        q: "",
                        page: &page,
                        next_base: path.clone(),
                        categories: &cats,
                        noindex: cursor.is_some(),
                        breadcrumb: Some((&name, &path)),
                        current_category: Some(slug.as_str()),
                    },
                ),
                200,
                "public, max-age=60",
            )
        }

        Route::Search { q, cursor } => {
            if q.is_empty() {
                return redirect(&format!("{}/circles", cfg.site_url), 302);
            }
            let (page, cats) = futures_util::future::join(
                ctx.discover(Discover {
                    q: Some(&q),
                    limit: 24,
                    cursor: cursor.as_deref(),
                    ..Default::default()
                }),
                ctx.categories(),
            )
            .await;
            let page = match page {
                Upstream::Ok(p) => p,
                Upstream::NotFound => Page::default(),
                Upstream::Unavailable(e) => return ctx.unavailable(&e),
            };
            html(
                pages::list(
                    &ctx.t,
                    cfg,
                    ListPage {
                        path: "/search",
                        eyebrow: "Search".into(),
                        heading: format!("Circles matching “{q}”"),
                        lead: "Open circles whose name or description matches your search.".into(),
                        q: &q,
                        page: &page,
                        next_base: format!("/search?q={}", ap::encode(&q)),
                        categories: &cats,
                        noindex: true,
                        breadcrumb: None,
                        current_category: None,
                    },
                ),
                200,
                "private, max-age=30",
            )
        }

        Route::Circle { slug } => match ctx.circle(&slug).await {
            // An AS2 request on any name gets the actor with its stored id.
            Upstream::Ok(c) if wants_ap => activity(&ap::actor(cfg, &c), 200),
            // A browser on a former handle or the slug goes to the current
            // address, so there is one page per circle.
            Upstream::Ok(c) if slug != c.key() => {
                let mut r = redirect(&cfg.circle_url(&c), 301)?;
                r.headers_mut().set("vary", "Accept")?;
                Ok(r)
            }
            Upstream::Ok(c) => {
                let posts = match ctx.posts(&c.slug, 5, None).await {
                    Upstream::Ok(p) => Some(p),
                    _ => None,
                };
                let mut r = html(
                    pages::circle(&ctx.t, cfg, &c, posts.as_ref()),
                    200,
                    "public, max-age=60",
                )?;
                r.headers_mut().set("vary", "Accept")?;
                r.headers_mut().set(
                    "link",
                    &format!(
                        "<{}>; rel=\"alternate\"; type=\"application/activity+json\"",
                        cfg.circle_actor(&c)
                    ),
                )?;
                Ok(r)
            }
            Upstream::NotFound if wants_ap => {
                json_error(404, "not_found", "No discoverable circle has that name.")
            }
            Upstream::NotFound => ctx.not_found(),
            Upstream::Unavailable(e) if wants_ap => {
                console_error!("actor {slug}: {e}");
                json_error(
                    503,
                    "unavailable",
                    "The Mukoko directory is unreachable. Try again shortly.",
                )
            }
            Upstream::Unavailable(e) => ctx.unavailable(&e),
        },

        Route::Join { slug } => match ctx.circle(&slug).await {
            Upstream::Ok(c) => {
                let phone = crate::config::is_phone(req.headers().get("user-agent")?.as_deref());
                // Where it goes depends on the device, so no shared cache.
                let mut r = redirect(&cfg.join_url(&c, phone), 302)?;
                r.headers_mut().set("cache-control", "private, no-store")?;
                r.headers_mut().set("vary", "User-Agent")?;
                Ok(r)
            }
            Upstream::NotFound => ctx.not_found(),
            // The directory is down; the super app may not be.
            Upstream::Unavailable(_) => redirect(&cfg.app_web_url, 302),
        },

        Route::Webfinger { resource } => match ap::webfinger_slug(cfg, resource.as_deref()) {
            Err(ap::WebfingerError::BadRequest(m)) => json_error(400, "bad_request", m),
            Err(ap::WebfingerError::NotFound) => json_error(
                404,
                "not_found",
                "No circle with that address on this server.",
            ),
            Ok(slug) => match ctx.circle(&slug).await {
                Upstream::Ok(c) => json_response(&ap::webfinger(cfg, &c), 200, ap::JRD_JSON),
                Upstream::NotFound => json_error(
                    404,
                    "not_found",
                    "No circle with that address on this server.",
                ),
                Upstream::Unavailable(e) => {
                    console_error!("webfinger {slug}: {e}");
                    json_error(
                        503,
                        "unavailable",
                        "The Mukoko directory is unreachable. Try again shortly.",
                    )
                }
            },
        },

        Route::Nodeinfo => {
            let total = match ctx
                .discover(Discover {
                    limit: 1,
                    ..Default::default()
                })
                .await
            {
                Upstream::Ok(p) => p.total,
                _ => None,
            };
            json_response(
                &ap::nodeinfo(cfg, total),
                200,
                "application/json; profile=\"http://nodeinfo.diaspora.software/ns/schema/2.1#\"",
            )
        }

        Route::Outbox { slug, page, cursor } => {
            let c = match ctx.circle(&slug).await {
                Upstream::Ok(c) => c,
                Upstream::NotFound => {
                    return json_error(404, "not_found", "No discoverable circle has that name.");
                }
                Upstream::Unavailable(e) => {
                    console_error!("outbox {slug}: {e}");
                    return json_error(
                        503,
                        "unavailable",
                        "The Mukoko directory is unreachable. Try again shortly.",
                    );
                }
            };
            if !page {
                return activity(&ap::outbox(cfg, &c, None), 200);
            }
            match ctx.posts(&c.slug, 20, cursor.as_deref()).await {
                Upstream::Ok(p) => activity(&ap::outbox_page(cfg, &c, &p, cursor.as_deref()), 200),
                Upstream::NotFound => {
                    json_error(404, "not_found", "No discoverable circle has that name.")
                }
                Upstream::Unavailable(e) => {
                    console_error!("outbox posts {slug}: {e}");
                    json_error(
                        503,
                        "unavailable",
                        "The Mukoko directory is unreachable. Try again shortly.",
                    )
                }
            }
        }

        Route::Post { slug, id } => {
            if !wants_ap {
                return redirect(&format!("{}/c/{slug}#post-{id}", cfg.site_url), 303);
            }
            let c = match ctx.circle(&slug).await {
                Upstream::Ok(c) => c,
                Upstream::NotFound => return json_error(404, "not_found", "No such post."),
                Upstream::Unavailable(_) => {
                    return json_error(
                        503,
                        "unavailable",
                        "The Mukoko directory is unreachable. Try again shortly.",
                    );
                }
            };
            let api = match ctx.api() {
                Ok(a) => a.to_string(),
                Err(e) => return ctx.unavailable(&e),
            };
            match api_get::<Post>(&env, &api::post(&api, &c.slug, &id)).await {
                Upstream::Ok(p) if p.id == id => activity(&ap::note(cfg, &c, &p), 200),
                Upstream::Ok(_) | Upstream::NotFound => {
                    json_error(404, "not_found", "No such post.")
                }
                Upstream::Unavailable(_) => json_error(
                    503,
                    "unavailable",
                    "The Mukoko directory is unreachable. Try again shortly.",
                ),
            }
        }

        Route::OgCircle { slug } => match ctx.circle(&slug).await {
            Upstream::Ok(c) => {
                let v = c.updated_at.clone().unwrap_or_default();
                let key = format!(
                    "{}/og/{}.png?v={}&b={}",
                    cfg.site_url,
                    c.key(),
                    ap::encode(&v),
                    cfg.version
                );
                let content = og::circle_content(cfg, &c);
                og_cached(&key, &wctx, || og::render_png(&content)).await
            }
            Upstream::NotFound => ctx.not_found(),
            Upstream::Unavailable(e) => ctx.unavailable(&e),
        },

        Route::Sitemap => {
            let mut circles: Vec<Circle> = Vec::new();
            let mut cursor: Option<String> = None;
            // Up to 1,000 circles. Past that, this becomes a sitemap index.
            for _ in 0..20 {
                match ctx
                    .discover(Discover {
                        limit: 50,
                        cursor: cursor.as_deref(),
                        ..Default::default()
                    })
                    .await
                {
                    Upstream::Ok(p) => {
                        circles.extend(p.data);
                        cursor = p.next_cursor;
                        if cursor.is_none() {
                            break;
                        }
                    }
                    Upstream::NotFound => break,
                    Upstream::Unavailable(e) => return ctx.unavailable(&e),
                }
            }
            let cats = ctx.categories().await;
            let mut resp = Response::ok(sitemap::sitemap(cfg, &cats, &circles))?;
            let h = resp.headers_mut();
            h.set("content-type", "application/xml; charset=utf-8")?;
            h.set("cache-control", "public, max-age=3600")?;
            Ok(secure(resp))
        }

        Route::Asset
        | Route::Inbox { .. }
        | Route::NodeinfoLinks
        | Route::HostMeta
        | Route::Create
        | Route::Lowercase { .. }
        | Route::OgHome => {
            unreachable!("handled above")
        }
    }
}
