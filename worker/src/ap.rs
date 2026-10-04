//! ActivityPub, WebFinger and NodeInfo documents.
//!
//! Phase 1 is read-only federation: every discoverable circle is a `Group`
//! actor that other servers can look up (WebFinger), fetch (the actor) and
//! read (the outbox of its public posts). Following a circle from Mastodon
//! and friends needs the inbox, which needs HTTP signatures and a key pair
//! per actor. That is phase 2, and until it lands the inbox answers 501 with
//! a message that says so, never a 200 that pretends to accept a follow.
//!
//! Everything here is a pure function of the data the API returned, tested
//! natively; `tests/as2.rs` checks every term against the vendored AS2 context.

use crate::config::Config;
use crate::model::{Circle, Page, Post};
use serde_json::{Value, json};

pub const AS_PUBLIC: &str = "https://www.w3.org/ns/activitystreams#Public";
pub const AS_CONTEXT: &str = "https://www.w3.org/ns/activitystreams";
pub const ACTIVITY_JSON: &str = "application/activity+json";
pub const JRD_JSON: &str = "application/jrd+json";
pub const NODEINFO_PROFILE: &str = "http://nodeinfo.diaspora.software/ns/schema/2.1";

/// The JSON-LD context for documents this site serves: AS2, plus three
/// widely used extension terms (Mastodon's), each mapped to its IRI so a
/// JSON-LD processor expands them rather than dropping them.
pub fn context() -> Value {
    json!([
        AS_CONTEXT,
        {
            "toot": "http://joinmastodon.org/ns#",
            "discoverable": "toot:discoverable",
            "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
            "Hashtag": "as:Hashtag"
        }
    ])
}

/// Does this `Accept` header ask for ActivityPub rather than a web page?
///
/// Only an explicit ActivityPub media type counts; `*/*` never does, so a
/// browser (which sends `text/html,…,*/*;q=0.8`) always gets HTML. When both
/// are named, the higher `q` wins and a tie goes to ActivityPub, which is what
/// a federating server that lists both means.
pub fn wants_activity_json(accept: Option<&str>) -> bool {
    let Some(accept) = accept else { return false };
    let mut ap_q: f32 = 0.0;
    let mut html_q: f32 = 0.0;
    for range in accept.split(',') {
        let mut parts = range.split(';').map(str::trim);
        let media = parts.next().unwrap_or("").to_ascii_lowercase();
        let mut q: f32 = 1.0;
        let mut profile: Option<String> = None;
        for p in parts {
            if let Some((k, v)) = p.split_once('=') {
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim().trim_matches('"');
                if k == "q" {
                    q = v.parse().unwrap_or(0.0);
                } else if k == "profile" {
                    profile = Some(v.to_string());
                }
            }
        }
        let is_ap = media == ACTIVITY_JSON
            || (media == "application/ld+json"
                && profile
                    .as_deref()
                    .is_none_or(|p| p.split(' ').any(|x| x == AS_CONTEXT)));
        if is_ap {
            ap_q = ap_q.max(q);
        } else if matches!(
            media.as_str(),
            "text/html" | "application/xhtml+xml" | "text/*" | "*/*"
        ) {
            html_q = html_q.max(q);
        }
    }
    ap_q > 0.0 && ap_q >= html_q
}

/// Plain text to the small HTML subset ActivityPub `content` and `summary`
/// carry: escaped, paragraphs from blank lines, `<br>` for single breaks.
pub fn text_to_html(text: &str) -> String {
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| {
            format!(
                "<p>{}</p>",
                crate::template::escape(p).replace('\n', "<br>")
            )
        })
        .collect()
}

/// The circle as an ActivityStreams `Group` actor.
pub fn actor(cfg: &Config, c: &Circle) -> Value {
    let id = cfg.circle_actor(c);
    let mut actor = json!({
        "@context": context(),
        "id": id,
        "type": "Group",
        "preferredUsername": c.slug,
        "name": c.name,
        "summary": text_to_html(&c.summary()),
        "url": id,
        "inbox": format!("{id}/inbox"),
        "outbox": format!("{id}/outbox"),
        "discoverable": true,
        "manuallyApprovesFollowers": false,
        "icon": {
            "type": "Image",
            "mediaType": "image/png",
            "url": format!("{}/og/{}.png", cfg.site_url, c.slug)
        },
        "tag": c.categories.iter().map(|cat| json!({
            "type": "Link",
            "name": cat.name,
            "href": format!("{}/categories/{}", cfg.site_url, cat.slug)
        })).collect::<Vec<_>>(),
    });
    let obj = actor.as_object_mut().expect("object");
    if let Some(p) = &c.created_at {
        obj.insert("published".into(), json!(p));
    }
    if let Some(u) = &c.updated_at {
        obj.insert("updated".into(), json!(u));
    }
    if let Some(img) = c
        .image_url
        .as_deref()
        .filter(|u| crate::config::is_safe_https(u))
    {
        obj.insert("image".into(), json!({ "type": "Image", "url": img }));
    }
    // The "Join" link, as a profile attachment people see on Mastodon.
    obj.insert(
        "attachment".into(),
        json!([{
            "type": "Link",
            "name": "Join in Mukoko",
            "href": cfg.join_url(c),
            "mediaType": "text/html"
        }]),
    );
    actor
}

/// The outbox, as a collection pointing at its first page.
pub fn outbox(cfg: &Config, c: &Circle, total: Option<u64>) -> Value {
    let id = format!("{}/outbox", cfg.circle_actor(c));
    json!({
        "@context": context(),
        "id": id,
        "type": "OrderedCollection",
        "totalItems": total.unwrap_or(c.post_count),
        "first": format!("{id}?page=true"),
    })
}

/// One page of the outbox: a `Create` for each public post, newest first.
pub fn outbox_page(cfg: &Config, c: &Circle, page: &Page<Post>, cursor: Option<&str>) -> Value {
    let outbox = format!("{}/outbox", cfg.circle_actor(c));
    let id = match cursor {
        Some(cur) => format!("{outbox}?page=true&cursor={}", encode(cur)),
        None => format!("{outbox}?page=true"),
    };
    let items: Vec<Value> = page.data.iter().map(|p| create(cfg, c, p)).collect();
    let mut v = json!({
        "@context": context(),
        "id": id,
        "type": "OrderedCollectionPage",
        "partOf": outbox,
        "orderedItems": items,
    });
    if let Some(next) = &page.next_cursor {
        v["next"] = json!(format!("{outbox}?page=true&cursor={}", encode(next)));
    }
    v
}

pub fn note_id(cfg: &Config, c: &Circle, post_id: &str) -> String {
    format!("{}/posts/{post_id}", cfg.circle_actor(c))
}

/// A post as a `Note`, attributed to the circle (never to a person: the
/// API does not expose authors to the public, and neither does this site).
pub fn note(cfg: &Config, c: &Circle, p: &Post) -> Value {
    let actor = cfg.circle_actor(c);
    let mut body = String::new();
    if let Some(h) = p.headline.as_deref().filter(|h| !h.trim().is_empty()) {
        body.push_str(&format!(
            "<p><strong>{}</strong></p>",
            crate::template::escape(h.trim())
        ));
    }
    body.push_str(&text_to_html(&p.article_body));
    let mut n = json!({
        "@context": context(),
        "id": note_id(cfg, c, &p.id),
        "type": "Note",
        "attributedTo": actor,
        "to": [AS_PUBLIC],
        "cc": [],
        "content": body,
        "url": note_id(cfg, c, &p.id),
        "tag": p.tags.iter().map(|t| json!({ "type": "Hashtag", "name": format!("#{t}") })).collect::<Vec<_>>(),
    });
    if let Some(d) = &p.date_published {
        n["published"] = json!(d);
    }
    if let Some(l) = &p.in_language {
        n["contentMap"] = json!({ l.clone(): n["content"].clone() });
    }
    n
}

fn create(cfg: &Config, c: &Circle, p: &Post) -> Value {
    let mut object = note(cfg, c, p);
    object.as_object_mut().expect("object").remove("@context");
    let mut v = json!({
        "id": format!("{}/activity", note_id(cfg, c, &p.id)),
        "type": "Create",
        "actor": cfg.circle_actor(c),
        "to": [AS_PUBLIC],
        "cc": [],
        "object": object,
    });
    if let Some(d) = &p.date_published {
        v["published"] = json!(d);
    }
    v
}

/// The 501 body for the inbox: what is missing, and where to go instead.
/// `join` is where a person can join the circle today.
pub fn inbox_not_implemented(join: &str) -> Value {
    json!({
        "error": "not_implemented",
        "message": "This inbox does not accept activities yet. Federation on circles.mukoko.com is read-only for now: you can look circles up and read their public posts, but following a circle or replying from another server comes in phase 2, with HTTP signatures. To join, use the Mukoko app.",
        "join": join,
        "tracking": "https://github.com/mukoko-dev/mukoko-circles/issues/8"
    })
}

/// Why a WebFinger lookup failed.
#[derive(Debug, PartialEq, Eq)]
pub enum WebfingerError {
    /// No `resource`, or one we cannot parse: 400.
    BadRequest(&'static str),
    /// A well-formed resource on another host, or not a circle: 404.
    NotFound,
}

/// Parse `resource` into a circle slug. Accepts `acct:{slug}@{host}` (the
/// leading `acct:` is optional, as some clients drop it) and the actor URL
/// itself, `https://{host}/c/{slug}`.
pub fn webfinger_slug(cfg: &Config, resource: Option<&str>) -> Result<String, WebfingerError> {
    let r = resource
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .ok_or(WebfingerError::BadRequest(
            "the resource parameter is required",
        ))?;
    let slug = if let Some(rest) = r.strip_prefix("https://") {
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if !host.eq_ignore_ascii_case(&cfg.host) {
            return Err(WebfingerError::NotFound);
        }
        path.strip_prefix("c/")
            .ok_or(WebfingerError::NotFound)?
            .to_string()
    } else {
        let acct = r.strip_prefix("acct:").unwrap_or(r);
        let acct = acct.strip_prefix('@').unwrap_or(acct);
        let (user, host) = acct
            .rsplit_once('@')
            .ok_or(WebfingerError::BadRequest("expected acct:{circle}@{host}"))?;
        if !host.eq_ignore_ascii_case(&cfg.host) {
            return Err(WebfingerError::NotFound);
        }
        user.to_ascii_lowercase()
    };
    if crate::model::is_valid_slug(&slug) {
        Ok(slug)
    } else {
        Err(WebfingerError::NotFound)
    }
}

/// The JRD for a circle (RFC 7033).
pub fn webfinger(cfg: &Config, c: &Circle) -> Value {
    let actor = cfg.circle_actor(c);
    json!({
        "subject": format!("acct:{}@{}", c.slug, cfg.host),
        "aliases": [actor],
        "links": [
            { "rel": "self", "type": ACTIVITY_JSON, "href": actor },
            { "rel": "http://webfinger.net/rel/profile-page", "type": "text/html", "href": actor },
            { "rel": "http://webfinger.net/rel/avatar", "type": "image/png", "href": format!("{}/og/{}.png", cfg.site_url, c.slug) }
        ]
    })
}

/// `/.well-known/nodeinfo`: where the NodeInfo document lives.
pub fn nodeinfo_links(cfg: &Config) -> Value {
    json!({
        "links": [
            { "rel": NODEINFO_PROFILE, "href": format!("{}/nodeinfo/2.1", cfg.site_url) }
        ]
    })
}

/// NodeInfo 2.1. Circles are groups, not user accounts, so `users` is empty
/// and registrations are closed here: people sign up in the Mukoko app.
pub fn nodeinfo(cfg: &Config, circles: Option<u64>) -> Value {
    let mut metadata = json!({
        "nodeName": "Mukoko Circles",
        "nodeDescription": "Communities on Mukoko, Africa's privacy-first social super-app.",
        "federation": { "inbox": "phase 2", "readOnly": true }
    });
    if let Some(n) = circles {
        metadata["circles"] = json!(n);
    }
    json!({
        "version": "2.1",
        "software": {
            "name": "mukoko-circles",
            "version": cfg.version,
            "repository": "https://github.com/mukoko-dev/mukoko-circles",
            "homepage": cfg.site_url
        },
        "protocols": ["activitypub"],
        "services": { "inbound": [], "outbound": [] },
        "openRegistrations": false,
        "usage": { "users": {} },
        "metadata": metadata
    })
}

/// Percent-encode a query value.
pub fn encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::new("https://circles.mukoko.com")
    }

    fn circle() -> Circle {
        serde_json::from_str(
            r#"{"id":"c1","slug":"harare-runners","name":"Harare Runners",
                "description":"Saturday long runs <and> coffee.","circleType":"public",
                "memberCount":312,"postCount":40,"createdAt":"2026-01-02T03:04:05Z",
                "categories":[{"slug":"sport","name":"Sport"}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn conneg() {
        let browser = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
        assert!(!wants_activity_json(Some(browser)));
        assert!(!wants_activity_json(None));
        assert!(!wants_activity_json(Some("*/*")));
        assert!(wants_activity_json(Some("application/activity+json")));
        assert!(wants_activity_json(Some(
            r#"application/ld+json; profile="https://www.w3.org/ns/activitystreams""#
        )));
        assert!(wants_activity_json(Some(
            "application/activity+json, application/ld+json"
        )));
        assert!(!wants_activity_json(Some(
            r#"application/ld+json; profile="https://example.com/other""#
        )));
        assert!(!wants_activity_json(Some(
            "text/html, application/activity+json;q=0.5"
        )));
        assert!(!wants_activity_json(Some("application/activity+json;q=0")));
    }

    #[test]
    fn actor_shape() {
        let a = actor(&cfg(), &circle());
        assert_eq!(a["id"], "https://circles.mukoko.com/c/harare-runners");
        assert_eq!(a["type"], "Group");
        assert_eq!(a["preferredUsername"], "harare-runners");
        assert_eq!(
            a["inbox"],
            "https://circles.mukoko.com/c/harare-runners/inbox"
        );
        assert_eq!(
            a["outbox"],
            "https://circles.mukoko.com/c/harare-runners/outbox"
        );
        assert_eq!(
            a["summary"],
            "<p>Saturday long runs &lt;and&gt; coffee.</p>"
        );
        assert_eq!(a["published"], "2026-01-02T03:04:05Z");
        assert_eq!(
            a["attachment"][0]["href"],
            "https://events.mukoko.com/circles/c1"
        );
    }

    #[test]
    fn webfinger_parsing() {
        let c = cfg();
        assert_eq!(
            webfinger_slug(&c, Some("acct:harare-runners@circles.mukoko.com")),
            Ok("harare-runners".into())
        );
        assert_eq!(
            webfinger_slug(&c, Some("acct:Harare-Runners@CIRCLES.mukoko.com")),
            Ok("harare-runners".into())
        );
        assert_eq!(
            webfinger_slug(&c, Some("https://circles.mukoko.com/c/harare-runners")),
            Ok("harare-runners".into())
        );
        assert_eq!(
            webfinger_slug(&c, Some("acct:harare-runners@mastodon.social")),
            Err(WebfingerError::NotFound)
        );
        assert!(matches!(
            webfinger_slug(&c, None),
            Err(WebfingerError::BadRequest(_))
        ));
        assert!(matches!(
            webfinger_slug(&c, Some("acct:nohost")),
            Err(WebfingerError::BadRequest(_))
        ));
        assert_eq!(
            webfinger_slug(&c, Some("acct:../x@circles.mukoko.com")),
            Err(WebfingerError::NotFound)
        );
    }

    #[test]
    fn webfinger_jrd() {
        let j = webfinger(&cfg(), &circle());
        assert_eq!(j["subject"], "acct:harare-runners@circles.mukoko.com");
        assert_eq!(j["links"][0]["rel"], "self");
        assert_eq!(j["links"][0]["type"], ACTIVITY_JSON);
        assert_eq!(
            j["links"][0]["href"],
            "https://circles.mukoko.com/c/harare-runners"
        );
    }

    #[test]
    fn outbox_paging() {
        let page: Page<Post> = serde_json::from_str(
            r#"{"data":[{"id":"p1","headline":"Long run","articleBody":"See you at 6.\n\nBring water.","datePublished":"2026-09-01T06:00:00Z","inLanguage":"en","tags":["running"]}],"nextCursor":"abc=="}"#,
        )
        .unwrap();
        let p = outbox_page(&cfg(), &circle(), &page, None);
        assert_eq!(p["type"], "OrderedCollectionPage");
        assert_eq!(
            p["next"],
            "https://circles.mukoko.com/c/harare-runners/outbox?page=true&cursor=abc%3D%3D"
        );
        let item = &p["orderedItems"][0];
        assert_eq!(item["type"], "Create");
        assert_eq!(item["object"]["type"], "Note");
        assert_eq!(
            item["object"]["attributedTo"],
            "https://circles.mukoko.com/c/harare-runners"
        );
        assert_eq!(
            item["object"]["content"],
            "<p><strong>Long run</strong></p><p>See you at 6.</p><p>Bring water.</p>"
        );
        assert!(item["object"].get("@context").is_none());
    }

    #[test]
    fn nodeinfo_shape() {
        let n = nodeinfo(&cfg(), Some(3));
        assert_eq!(n["version"], "2.1");
        assert_eq!(n["protocols"][0], "activitypub");
        assert_eq!(n["openRegistrations"], false);
        assert!(n["usage"]["users"].is_object());
        let l = nodeinfo_links(&cfg());
        assert_eq!(l["links"][0]["rel"], NODEINFO_PROFILE);
    }
}
