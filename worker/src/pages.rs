//! The HTML pages: data from the API, poured into the Astro-built shells.

use crate::ap::encode;
use crate::config::Config;
use crate::model::{Category, Circle, Page, Post};
use crate::template::{Vars, escape, fill, fragment, json_for_script};
use serde_json::json;

/// The built shells, read from the static assets (`/tpl/*.html`).
#[derive(Clone, Debug, Default)]
pub struct Templates {
    pub home: String,
    pub list: String,
    pub circle: String,
    pub message: String,
    pub card: String,
    pub chip: String,
    pub post: String,
}

impl Templates {
    pub const PATHS: [&'static str; 7] = [
        "/tpl/home.html",
        "/tpl/list.html",
        "/tpl/circle.html",
        "/tpl/message.html",
        "/tpl/card.html",
        "/tpl/chip.html",
        "/tpl/post.html",
    ];

    /// From the seven files, in [`Self::PATHS`] order.
    pub fn from_files(files: [String; 7]) -> Self {
        let [home, list, circle, message, card, chip, post] = files;
        Templates {
            home,
            list,
            circle,
            message,
            card: fragment(&card).to_string(),
            chip: fragment(&chip).to_string(),
            post: fragment(&post).to_string(),
        }
    }
}

/// The values every page shares: head metadata and the site-wide links.
struct Head<'a> {
    title: String,
    description: String,
    path: &'a str,
    og_image: String,
    og_type: &'a str,
    noindex: bool,
    extra_head: String,
}

fn base_vars(cfg: &Config, h: Head) -> Vars {
    let canonical = format!("{}{}", cfg.site_url, h.path);
    let mut stores = String::new();
    if let Some(u) = cfg
        .app_store_url
        .as_deref()
        .filter(|u| crate::config::is_safe_https(u))
    {
        stores.push_str(&format!(
            r#"<a class="link" href="{}">Get Mukoko on the App Store</a>"#,
            escape(u)
        ));
    }
    if let Some(u) = cfg
        .play_store_url
        .as_deref()
        .filter(|u| crate::config::is_safe_https(u))
    {
        stores.push_str(&format!(
            r#"<a class="link" href="{}">Get Mukoko on Google Play</a>"#,
            escape(u)
        ));
    }
    Vars::new()
        .text("title", h.title)
        .text("description", h.description)
        .text("canonical", canonical)
        .text("og_image", h.og_image)
        .text("og_type", h.og_type)
        .text(
            "robots",
            if h.noindex {
                "noindex, follow"
            } else {
                "index, follow"
            },
        )
        .text("create_href", cfg.create_url.clone())
        .text("app_href", cfg.app_web_url.clone())
        .text("host", cfg.host.clone())
        .html("head_extra", h.extra_head)
        .html("store_links", stores)
}

pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

pub fn count(n: u64, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{} {many}", thousands(n))
    }
}

/// At most `max` characters, cut at a word, with an ellipsis.
pub fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    let cut = match cut.rfind(char::is_whitespace) {
        Some(i) if i > max / 2 => &cut[..i],
        _ => &cut[..],
    };
    format!(
        "{}…",
        cut.trim_end_matches(|c: char| c.is_whitespace() || c == ',' || c == '.')
    )
}

pub fn language_name(code: &str) -> String {
    let base = code
        .split(['-', '_'])
        .next()
        .unwrap_or(code)
        .to_ascii_lowercase();
    match base.as_str() {
        "en" => "English",
        "sn" => "Shona",
        "nd" => "Ndebele",
        "sw" => "Swahili",
        "zu" => "Zulu",
        "xh" => "Xhosa",
        "af" => "Afrikaans",
        "ny" => "Chichewa",
        "st" => "Sesotho",
        "tn" => "Setswana",
        "yo" => "Yoruba",
        "ig" => "Igbo",
        "ha" => "Hausa",
        "am" => "Amharic",
        "fr" => "French",
        "pt" => "Portuguese",
        "ar" => "Arabic",
        _ => return code.to_string(),
    }
    .to_string()
}

/// `2026-01-02T…` to `January 2026`.
pub fn month_year(iso: &str) -> Option<String> {
    let mut parts = iso.get(..7)?.split('-');
    let year = parts.next()?;
    let month: usize = parts.next()?.parse().ok()?;
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    if year.len() != 4 || !year.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{} {year}", MONTHS.get(month.checked_sub(1)?)?))
}

/// `1 January 2026`.
pub fn day_month_year(iso: &str) -> Option<String> {
    let my = month_year(iso)?;
    let day: u32 = iso.get(8..10)?.parse().ok()?;
    Some(format!("{day} {my}"))
}

fn type_label(c: &Circle) -> (&'static str, &'static str, &'static str) {
    if c.is_broadcast() {
        (
            "Broadcast",
            "badge-accent",
            "Hosts post, everyone can follow along.",
        )
    } else {
        ("Public", "badge-premium", "Anyone can join and take part.")
    }
}

fn initial(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "•".into())
}

pub fn card(t: &Templates, cfg: &Config, c: &Circle) -> String {
    let (label, class, _) = type_label(c);
    let cats = c
        .categories
        .iter()
        .map(|x| x.name.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
    fill(
        &t.card,
        &Vars::new()
            .text("name", c.name.clone())
            .text("href", format!("/c/{}", c.slug))
            .text("summary", truncate(&c.summary(), 140))
            .text("members", count(c.member_count, "member", "members"))
            .text("type_label", label)
            .text("type_class", class)
            .text("initial", initial(&c.name))
            .text("categories", cats)
            .text("handle", format!("@{}@{}", c.slug, cfg.host)),
    )
}

pub fn cards(t: &Templates, cfg: &Config, circles: &[Circle]) -> String {
    circles.iter().map(|c| card(t, cfg, c)).collect()
}

pub fn chip(t: &Templates, slug: &str, name: &str, n: Option<u64>) -> String {
    fill(
        &t.chip,
        &Vars::new()
            .text("href", format!("/categories/{slug}"))
            .text("name", name)
            .text("count", n.map(thousands).unwrap_or_default()),
    )
}

pub fn chips(t: &Templates, cats: &[Category]) -> String {
    cats.iter()
        .filter(|c| crate::model::is_valid_slug(&c.slug))
        .map(|c| chip(t, &c.slug, &c.name, Some(c.circle_count)))
        .collect()
}

fn site_jsonld(cfg: &Config) -> serde_json::Value {
    json!({
        "@type": "WebSite",
        "@id": format!("{}/#website", cfg.site_url),
        "url": cfg.site_url,
        "name": "Mukoko Circles",
        "inLanguage": "en",
        "publisher": {
            "@type": "Organization",
            "name": "Mukoko",
            "url": "https://mukoko.com",
            "slogan": "Africa's privacy-first social super-app"
        },
        "potentialAction": {
            "@type": "SearchAction",
            "target": format!("{}/search?q={{search_term_string}}", cfg.site_url),
            "query-input": "required name=search_term_string"
        }
    })
}

fn ld(nodes: Vec<serde_json::Value>) -> String {
    format!(
        r#"<script type="application/ld+json">{}</script>"#,
        json_for_script(&json!({ "@context": "https://schema.org", "@graph": nodes }))
    )
}

pub fn home(
    t: &Templates,
    cfg: &Config,
    featured: &[Circle],
    latest: &Page<Circle>,
    categories: &[Category],
) -> String {
    let example = featured
        .first()
        .or(latest.data.first())
        .map(|c| c.slug.clone())
        .unwrap_or_else(|| "harare-runners".into());
    let total = latest.total.unwrap_or(latest.data.len() as u64);
    let vars = base_vars(
        cfg,
        Head {
            title: "Mukoko Circles: find your people".into(),
            description: "Circles are communities on Mukoko, Africa's privacy-first social super-app. Browse open circles, then join in the Mukoko app, or follow them from the fediverse.".into(),
            path: "/",
            og_image: format!("{}/og/home.png", cfg.site_url),
            og_type: "website",
            noindex: false,
            extra_head: ld(vec![site_jsonld(cfg)]),
        },
    )
    .html("featured", cards(t, cfg, featured))
    .html("latest", cards(t, cfg, &latest.data))
    .html("categories", chips(t, categories))
    .text("total", count(total, "open circle", "open circles"))
    .text("example_handle", format!("@{example}@{}", cfg.host))
    .text(
        "featured_state",
        if featured.is_empty() { "empty" } else { "ok" },
    )
    .text(
        "latest_state",
        if latest.data.is_empty() {
            "empty"
        } else {
            "ok"
        },
    )
    .text(
        "categories_state",
        if categories.is_empty() { "empty" } else { "ok" },
    );
    fill(&t.home, &vars)
}

/// A list of circles: everything, one category, or search results.
pub struct ListPage<'a> {
    pub path: &'a str,
    pub eyebrow: String,
    pub heading: String,
    pub lead: String,
    pub q: &'a str,
    pub page: &'a Page<Circle>,
    /// The query string for the next page, minus `cursor`.
    pub next_base: String,
    pub categories: &'a [Category],
    pub noindex: bool,
    pub breadcrumb: Option<(&'a str, &'a str)>,
}

pub fn list(t: &Templates, cfg: &Config, p: ListPage) -> String {
    let next = p
        .page
        .next_cursor
        .as_deref()
        .map(|cur| {
            let sep = if p.next_base.contains('?') { '&' } else { '?' };
            format!(
                r#"<a class="btn-outline" rel="next" href="{}{sep}cursor={}">More circles</a>"#,
                escape(&p.next_base),
                escape(&encode(cur))
            )
        })
        .unwrap_or_default();
    let shown = p.page.data.len() as u64;
    let results = match p.page.total {
        Some(n) => count(n, "circle", "circles"),
        None => count(shown, "circle", "circles"),
    };
    let mut nodes = vec![site_jsonld(cfg)];
    let mut crumbs = vec![
        json!({"@type":"ListItem","position":1,"name":"Circles","item":format!("{}/", cfg.site_url)}),
    ];
    if let Some((name, path)) = p.breadcrumb {
        crumbs.push(json!({"@type":"ListItem","position":2,"name":name,"item":format!("{}{}", cfg.site_url, path)}));
    }
    nodes.push(json!({"@type":"BreadcrumbList","itemListElement":crumbs}));
    nodes.push(json!({
        "@type": "CollectionPage",
        "url": format!("{}{}", cfg.site_url, p.path),
        "name": p.heading,
        "isPartOf": {"@id": format!("{}/#website", cfg.site_url)},
        "mainEntity": {
            "@type": "ItemList",
            "itemListElement": p.page.data.iter().enumerate().map(|(i, c)| json!({
                "@type": "ListItem", "position": i + 1, "url": cfg.circle_actor(c), "name": c.name
            })).collect::<Vec<_>>()
        }
    }));
    let vars = base_vars(
        cfg,
        Head {
            title: format!("{} · Mukoko Circles", p.heading),
            description: p.lead.clone(),
            path: p.path,
            og_image: format!("{}/og/home.png", cfg.site_url),
            og_type: "website",
            noindex: p.noindex,
            extra_head: ld(nodes),
        },
    )
    .text("eyebrow", p.eyebrow)
    .text("heading", p.heading)
    .text("lead", p.lead)
    .text("q", p.q)
    .text("results", results)
    .text(
        "results_state",
        if p.page.data.is_empty() {
            "empty"
        } else {
            "ok"
        },
    )
    .html("cards", cards(t, cfg, &p.page.data))
    .html("next", next)
    .html("categories", chips(t, p.categories));
    fill(&t.list, &vars)
}

pub fn post(t: &Templates, cfg: &Config, c: &Circle, p: &Post) -> String {
    fill(
        &t.post,
        &Vars::new()
            .text("id", format!("post-{}", p.id))
            .text("headline", p.headline.clone().unwrap_or_default())
            .html(
                "body",
                crate::ap::text_to_html(&truncate(&p.article_body, 600)),
            )
            .text(
                "date",
                p.date_published
                    .as_deref()
                    .and_then(day_month_year)
                    .unwrap_or_default(),
            )
            .text("datetime", p.date_published.clone().unwrap_or_default())
            .text("href", crate::ap::note_id(cfg, c, &p.id)),
    )
}

pub fn circle(t: &Templates, cfg: &Config, c: &Circle, posts: Option<&Page<Post>>) -> String {
    let (label, class, hint) = type_label(c);
    let actor = cfg.circle_actor(c);
    let summary = c.summary();
    let cats: String = c
        .categories
        .iter()
        .filter(|x| crate::model::is_valid_slug(&x.slug))
        .map(|x| chip(t, &x.slug, &x.name, None))
        .collect();
    let posts_html: String = posts
        .map(|p| p.data.iter().take(5).map(|x| post(t, cfg, c, x)).collect())
        .unwrap_or_default();
    let mut about = json!({
        "@type": "Organization",
        "additionalType": "https://www.w3.org/ns/activitystreams#Group",
        "name": c.name,
        "description": summary,
        "url": actor,
        "parentOrganization": {"@type": "Organization", "name": "Mukoko", "url": "https://mukoko.com"},
    });
    if let Some(img) = c
        .image_url
        .as_deref()
        .filter(|u| crate::config::is_safe_https(u))
    {
        about["logo"] = json!(img);
    }
    let nodes = vec![
        site_jsonld(cfg),
        json!({"@type":"BreadcrumbList","itemListElement":[
            {"@type":"ListItem","position":1,"name":"Circles","item":format!("{}/", cfg.site_url)},
            {"@type":"ListItem","position":2,"name":c.name,"item":actor}
        ]}),
        json!({
            "@type": "WebPage",
            "url": actor,
            "name": c.name,
            "description": summary,
            "isPartOf": {"@id": format!("{}/#website", cfg.site_url)},
            "about": about
        }),
    ];
    let extra = format!(
        r#"<link rel="alternate" type="application/activity+json" href="{}">{}"#,
        escape(&actor),
        ld(nodes)
    );
    let join_label = if c.is_broadcast() {
        "Follow in Mukoko"
    } else {
        "Join in Mukoko"
    };
    let vars = base_vars(
        cfg,
        Head {
            title: format!("{} · Mukoko Circles", c.name),
            description: truncate(&summary, 160),
            path: &format!("/c/{}", c.slug),
            og_image: format!("{}/og/{}.png", cfg.site_url, c.slug),
            og_type: "profile",
            noindex: false,
            extra_head: extra,
        },
    )
    .text("name", c.name.clone())
    .text("slug", c.slug.clone())
    .text("summary", summary)
    .text("type_label", label)
    .text("type_class", class)
    .text("type_hint", hint)
    .text("initial", initial(&c.name))
    .text("members", count(c.member_count, "member", "members"))
    .text("posts", count(c.post_count, "post", "posts"))
    .text(
        "language",
        c.in_language
            .as_deref()
            .map(language_name)
            .unwrap_or_else(|| "English".into()),
    )
    .text(
        "place",
        c.place.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
    )
    .text(
        "place_state",
        if c.place.is_some() { "ok" } else { "empty" },
    )
    .text(
        "since",
        c.created_at
            .as_deref()
            .and_then(month_year)
            .map(|d| format!("Since {d}"))
            .unwrap_or_default(),
    )
    .text("join_href", format!("/c/{}/join", c.slug))
    .text("join_label", join_label)
    .text("handle", format!("@{}@{}", c.slug, cfg.host))
    .text("actor", actor)
    .html("categories", cats)
    .text(
        "categories_state",
        if c.categories.is_empty() {
            "empty"
        } else {
            "ok"
        },
    )
    .html("posts", posts_html)
    .text(
        "posts_state",
        if posts.is_some_and(|p| !p.data.is_empty()) {
            "ok"
        } else {
            "empty"
        },
    );
    fill(&t.circle, &vars)
}

/// Errors and empty states: 404, 503 and friends.
pub fn message(
    t: &Templates,
    cfg: &Config,
    path: &str,
    eyebrow: &str,
    heading: &str,
    body: &str,
) -> String {
    let vars = base_vars(
        cfg,
        Head {
            title: format!("{heading} · Mukoko Circles"),
            description: body.to_string(),
            path,
            og_image: format!("{}/og/home.png", cfg.site_url),
            og_type: "website",
            noindex: true,
            extra_head: String::new(),
        },
    )
    .text("eyebrow", eyebrow)
    .text("heading", heading)
    .text("message", body);
    fill(&t.message, &vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_words() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(count(1, "member", "members"), "1 member");
        assert_eq!(count(2500, "member", "members"), "2,500 members");
    }

    #[test]
    fn truncation() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(
            truncate("The quick brown fox jumps over the lazy dog", 20),
            "The quick brown fox…"
        );
        assert_eq!(truncate("ẞßßßßßßßßßßß", 5), "ẞßßßß…");
    }

    #[test]
    fn dates_and_languages() {
        assert_eq!(month_year("2026-01-02T03:04:05Z").unwrap(), "January 2026");
        assert_eq!(
            day_month_year("2026-09-01T06:00:00Z").unwrap(),
            "1 September 2026"
        );
        assert!(month_year("nonsense").is_none());
        assert!(month_year("2026-13-01").is_none());
        assert_eq!(language_name("sn"), "Shona");
        assert_eq!(language_name("en-GB"), "English");
        assert_eq!(language_name("xx"), "xx");
    }
}
