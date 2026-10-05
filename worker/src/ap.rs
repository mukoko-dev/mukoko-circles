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

//!
//! The protocol plumbing (context, content negotiation, escaping, WebFinger
//! parsing, NodeInfo, collection pages) is the shared Mzizi crate
//! `mzizi-activitypub` (decision Q5, mukoko-dev/kweli#171), which kweli.mukoko.com
//! uses too. What is left here is what makes a circle a circle.

use crate::config::Config;
use crate::model::{Circle, Page, Post};
use mzizi_activitypub as map;
use serde_json::{Value, json};

pub use map::{
    ACTIVITY_JSON, AS_CONTEXT, AS_PUBLIC, JRD_JSON, NODEINFO_PROFILE, WebfingerError, encode,
    text_to_html, wants_activity_json,
};

/// The JSON-LD context for documents this site serves: AS2, plus three
/// widely used extension terms (Mastodon's), each mapped to its IRI so a
/// JSON-LD processor expands them rather than dropping them.
pub fn context() -> Value {
    map::context(json!({
        "toot": "http://joinmastodon.org/ns#",
        "discoverable": "toot:discoverable",
        "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
        "Hashtag": "as:Hashtag"
    }))
}

/// The circle as an ActivityStreams `Group` actor.
///
/// `id`, `inbox` and `outbox` hang off the stored actor id; `url` and
/// `preferredUsername` follow the current handle. After a rename they differ,
/// which is exactly what keeps the actor (and its followers) in place.
pub fn actor(cfg: &Config, c: &Circle) -> Value {
    let id = cfg.circle_actor(c);
    let page = cfg.circle_url(c);
    let mut actor = json!({
        "@context": context(),
        "id": id,
        "type": "Group",
        "preferredUsername": c.username(),
        "name": c.name,
        "summary": text_to_html(&c.summary()),
        "url": page,
        "inbox": format!("{id}/inbox"),
        "outbox": format!("{id}/outbox"),
        "discoverable": true,
        "manuallyApprovesFollowers": false,
        "icon": {
            "type": "Image",
            "mediaType": "image/png",
            "url": format!("{}/og/{}.png", cfg.site_url, c.key())
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
            "href": format!("{page}/join"),
            "mediaType": "text/html"
        }]),
    );
    actor
}

/// The outbox, as a collection pointing at its first page.
pub fn outbox(cfg: &Config, c: &Circle, total: Option<u64>) -> Value {
    let id = format!("{}/outbox", cfg.circle_actor(c));
    map::ordered_collection(context(), &id, total.unwrap_or(c.post_count))
}

/// One page of the outbox: a `Create` for each public post, newest first.
pub fn outbox_page(cfg: &Config, c: &Circle, page: &Page<Post>, cursor: Option<&str>) -> Value {
    let outbox = format!("{}/outbox", cfg.circle_actor(c));
    let items: Vec<Value> = page.data.iter().map(|p| create(cfg, c, p)).collect();
    map::ordered_collection_page(
        context(),
        &outbox,
        cursor,
        items,
        page.next_cursor.as_deref(),
    )
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
            map::escape_html(h.trim())
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

/// Parse `resource` into a circle key (lower-cased). Accepts
/// `acct:{handle}@{host}` (the leading `acct:` is optional, as some clients
/// drop it) and a circle URL, `https://{host}/c/{key}`. The key may be a
/// current handle, the slug or a retired handle: the API resolves all three,
/// and the JRD always answers with the current one.
pub fn webfinger_slug(cfg: &Config, resource: Option<&str>) -> Result<String, WebfingerError> {
    let slug = map::webfinger_user(resource, &cfg.host, "c/")?.to_ascii_lowercase();
    if crate::model::is_valid_key(&slug) {
        Ok(slug)
    } else {
        Err(WebfingerError::NotFound)
    }
}

/// The JRD for a circle (RFC 7033). Whatever name the lookup used (a
/// retired handle, the slug), the `subject` is the current handle and `self`
/// is the stored actor id.
pub fn webfinger(cfg: &Config, c: &Circle) -> Value {
    let actor = cfg.circle_actor(c);
    let page = cfg.circle_url(c);
    let avatar = format!("{}/og/{}.png", cfg.site_url, c.key());
    let aliases: Vec<&str> = if page == actor {
        vec![&actor]
    } else {
        vec![&actor, &page]
    };
    map::jrd(
        &format!("acct:{}@{}", c.username(), cfg.host),
        &aliases,
        vec![
            map::self_link(&actor),
            map::profile_page_link(&page),
            json!({ "rel": "http://webfinger.net/rel/avatar", "type": "image/png", "href": avatar }),
        ],
    )
}

/// `/.well-known/nodeinfo`: where the NodeInfo document lives.
pub fn nodeinfo_links(cfg: &Config) -> Value {
    map::nodeinfo_links(&cfg.site_url)
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
    map::nodeinfo(&map::NodeInfo {
        software: "mukoko-circles",
        version: &cfg.version,
        repository: "https://github.com/mukoko-dev/mukoko-circles",
        homepage: &cfg.site_url,
        metadata,
    })
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
            "https://circles.mukoko.com/c/harare-runners/join"
        );
    }

    #[test]
    fn renamed_circle_keeps_its_actor_id() {
        let mut c = circle();
        c.handle = Some("HarareRunClub".into());
        c.aliases = vec!["harare-runners".into()];
        c.actor_uri = Some("https://circles.mukoko.com/c/harare-runners".into());
        let a = actor(&cfg(), &c);
        assert_eq!(a["id"], "https://circles.mukoko.com/c/harare-runners");
        assert_eq!(a["preferredUsername"], "HarareRunClub");
        assert_eq!(a["url"], "https://circles.mukoko.com/c/hararerunclub");
        assert_eq!(
            a["inbox"],
            "https://circles.mukoko.com/c/harare-runners/inbox"
        );
        let j = webfinger(&cfg(), &c);
        assert_eq!(j["subject"], "acct:HarareRunClub@circles.mukoko.com");
        assert_eq!(
            j["links"][0]["href"],
            "https://circles.mukoko.com/c/harare-runners"
        );
        assert_eq!(
            j["links"][1]["href"],
            "https://circles.mukoko.com/c/hararerunclub"
        );
        assert_eq!(j["aliases"].as_array().unwrap().len(), 2);
        let n = note(
            &cfg(),
            &c,
            &Post {
                id: "p1".into(),
                ..Default::default()
            },
        );
        assert_eq!(
            n["id"],
            "https://circles.mukoko.com/c/harare-runners/posts/p1"
        );
        assert_eq!(
            n["attributedTo"],
            "https://circles.mukoko.com/c/harare-runners"
        );
    }

    #[test]
    fn webfinger_accepts_handles() {
        let c = cfg();
        assert_eq!(
            webfinger_slug(&c, Some("acct:HarareRunClub@circles.mukoko.com")),
            Ok("hararerunclub".into())
        );
        assert_eq!(
            webfinger_slug(&c, Some("acct:harare_run@circles.mukoko.com")),
            Ok("harare_run".into())
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
