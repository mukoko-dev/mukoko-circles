//! Which handler a request goes to. Pure, so the whole URL space is tested
//! natively.

use crate::model::{is_valid_id, is_valid_key, is_valid_slug};

#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Home,
    All {
        cursor: Option<String>,
    },
    Category {
        slug: String,
        cursor: Option<String>,
    },
    Search {
        q: String,
        cursor: Option<String>,
    },
    /// A circle path in the wrong case: handles keep the case their admins
    /// chose in `preferredUsername`, but paths are lower-case. 301 here.
    Lowercase {
        location: String,
    },
    /// HTML or the ActivityPub actor, by `Accept`. `slug` is the requested
    /// key: the circle's handle (lower-cased), its slug, or an alias.
    Circle {
        slug: String,
    },
    /// Redirect into the super app.
    Join {
        slug: String,
    },
    /// Redirect to the super app's create flow.
    Create,
    Outbox {
        slug: String,
        page: bool,
        cursor: Option<String>,
    },
    /// Any method. 501 until phase 2. `None` is the shared inbox.
    Inbox {
        slug: Option<String>,
    },
    Post {
        slug: String,
        id: String,
    },
    OgCircle {
        slug: String,
    },
    OgHome,
    Webfinger {
        resource: Option<String>,
    },
    HostMeta,
    NodeinfoLinks,
    Nodeinfo,
    Sitemap,
    /// The Astro shells are build inputs for this Worker, not pages.
    Hidden,
    /// A path that cannot be a circle (bad slug): a plain 404.
    NotFound,
    /// Anything else is a static asset.
    Asset,
}

fn query(q: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(q.trim_start_matches('?').as_bytes())
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

fn cursor(q: &str) -> Option<String> {
    query(q, "cursor").filter(|c| !c.is_empty() && c.len() <= 512)
}

pub fn route(path: &str, q: &str) -> Route {
    // `/c/HarareRunners` → `/c/hararerunners` (and the same under /og/).
    if (path.starts_with("/c/") || path.starts_with("/og/"))
        && path.bytes().any(|b| b.is_ascii_uppercase())
    {
        let lower = path.to_ascii_lowercase();
        return match route(&lower, q) {
            Route::NotFound | Route::Asset | Route::Hidden => Route::NotFound,
            _ => {
                let q = q.trim_start_matches('?');
                Route::Lowercase {
                    location: if q.is_empty() {
                        lower
                    } else {
                        format!("{lower}?{q}")
                    },
                }
            }
        };
    }
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    match segs.as_slice() {
        [""] => Route::Home,
        ["circles"] => Route::All { cursor: cursor(q) },
        ["search"] => Route::Search {
            q: query(q, "q")
                .map(|s| s.trim().chars().take(100).collect())
                .unwrap_or_default(),
            cursor: cursor(q),
        },
        ["create"] => Route::Create,
        ["categories", slug] if is_valid_slug(slug) => Route::Category {
            slug: slug.to_string(),
            cursor: cursor(q),
        },
        ["categories", _] => Route::NotFound,
        ["c", slug] => valid(slug, |slug| Route::Circle { slug }),
        ["c", slug, "join"] => valid(slug, |slug| Route::Join { slug }),
        ["c", slug, "outbox"] => valid(slug, |slug| Route::Outbox {
            slug,
            page: query(q, "page").is_some_and(|p| p == "true" || p == "1"),
            cursor: cursor(q),
        }),
        ["c", slug, "inbox"] => valid(slug, |slug| Route::Inbox { slug: Some(slug) }),
        ["c", slug, "posts", id] if is_valid_id(id) => valid(slug, |slug| Route::Post {
            slug,
            id: id.to_string(),
        }),
        ["c", ..] => Route::NotFound,
        ["inbox"] => Route::Inbox { slug: None },
        ["og", "home.png"] => Route::OgHome,
        ["og", file] => match file.strip_suffix(".png") {
            Some(slug) => valid(slug, |slug| Route::OgCircle { slug }),
            None => Route::NotFound,
        },
        [".well-known", "webfinger"] => Route::Webfinger {
            resource: query(q, "resource"),
        },
        [".well-known", "host-meta"] => Route::HostMeta,
        [".well-known", "nodeinfo"] => Route::NodeinfoLinks,
        ["nodeinfo", "2.1"] => Route::Nodeinfo,
        ["sitemap.xml"] => Route::Sitemap,
        ["tpl", ..] => Route::Hidden,
        _ => Route::Asset,
    }
}

fn valid(slug: &str, f: impl FnOnce(String) -> Route) -> Route {
    if is_valid_key(slug) {
        f(slug.to_string())
    } else {
        Route::NotFound
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes() {
        assert_eq!(route("/", ""), Route::Home);
        assert_eq!(
            route("/circles", "?cursor=abc"),
            Route::All {
                cursor: Some("abc".into())
            }
        );
        assert_eq!(
            route("/search", "q=+run%20club+&cursor="),
            Route::Search {
                q: "run club".into(),
                cursor: None
            }
        );
        assert_eq!(
            route("/categories/sport", ""),
            Route::Category {
                slug: "sport".into(),
                cursor: None
            }
        );
        assert_eq!(
            route("/c/harare-runners", ""),
            Route::Circle {
                slug: "harare-runners".into()
            }
        );
        assert_eq!(
            route("/c/Harare", ""),
            Route::Lowercase {
                location: "/c/harare".into()
            }
        );
        assert_eq!(
            route("/c/HarareRunners/outbox", "page=true"),
            Route::Lowercase {
                location: "/c/hararerunners/outbox?page=true".into()
            }
        );
        assert_eq!(
            route("/c/harare_runners", ""),
            Route::Circle {
                slug: "harare_runners".into()
            }
        );
        assert_eq!(route("/c/Bad.Name", ""), Route::NotFound);
        assert_eq!(route("/c/-a", ""), Route::NotFound);
        assert_eq!(route("/categories/a_b", ""), Route::NotFound);
        assert_eq!(route("/c/a/join", ""), Route::Join { slug: "a".into() });
        assert_eq!(
            route("/c/a/outbox", "page=true&cursor=x"),
            Route::Outbox {
                slug: "a".into(),
                page: true,
                cursor: Some("x".into())
            }
        );
        assert_eq!(
            route("/c/a/inbox", ""),
            Route::Inbox {
                slug: Some("a".into())
            }
        );
        assert_eq!(route("/inbox", ""), Route::Inbox { slug: None });
        assert_eq!(
            route("/c/a/posts/6f1c-22", ""),
            Route::Post {
                slug: "a".into(),
                id: "6f1c-22".into()
            }
        );
        assert_eq!(route("/c/a/posts/..%2f", ""), Route::NotFound);
        assert_eq!(route("/og/home.png", ""), Route::OgHome);
        assert_eq!(route("/og/a.png", ""), Route::OgCircle { slug: "a".into() });
        assert_eq!(route("/og/a.jpg", ""), Route::NotFound);
        assert_eq!(
            route(
                "/.well-known/webfinger",
                "resource=acct%3Aa%40circles.mukoko.com"
            ),
            Route::Webfinger {
                resource: Some("acct:a@circles.mukoko.com".into())
            }
        );
        assert_eq!(route("/.well-known/nodeinfo", ""), Route::NodeinfoLinks);
        assert_eq!(route("/nodeinfo/2.1", ""), Route::Nodeinfo);
        assert_eq!(route("/sitemap.xml", ""), Route::Sitemap);
        assert_eq!(route("/tpl/home.html", ""), Route::Hidden);
        assert_eq!(route("/about", ""), Route::Asset);
        assert_eq!(route("/_astro/x.css", ""), Route::Asset);
    }
}
