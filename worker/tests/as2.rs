//! Every ActivityPub document this site serves, checked against the
//! ActivityStreams 2.0 JSON-LD context.
//!
//! `fixtures/activitystreams.jsonld` is the context as published at
//! https://www.w3.org/ns/activitystreams (vendored, so the test is offline and
//! deterministic). A term that is in neither that context nor the document's
//! own extension context would be silently dropped by a JSON-LD processor, so
//! it fails here instead. The same goes for `type` values, and for the
//! properties AS2 and ActivityPub require of each kind of object.

use mukoko_circles_worker::ap;
use mukoko_circles_worker::config::Config;
use mukoko_circles_worker::model::{Circle, Page, Post};
use serde_json::Value;
use std::collections::BTreeSet;

fn as2_terms() -> BTreeSet<String> {
    let ctx: Value = serde_json::from_str(include_str!("fixtures/activitystreams.jsonld")).unwrap();
    ctx["@context"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|k| !k.starts_with('@'))
        .cloned()
        .collect()
}

/// Terms the document defines itself, from the object entries of its
/// `@context` array.
fn local_terms(doc: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Some(arr) = doc["@context"].as_array() {
        for c in arr {
            if let Some(o) = c.as_object() {
                out.extend(o.keys().cloned());
            }
        }
    }
    out
}

fn check(doc: &Value) {
    let ctx = &doc["@context"];
    let first = ctx
        .as_array()
        .and_then(|a| a.first())
        .or(Some(ctx))
        .and_then(Value::as_str);
    assert_eq!(
        first,
        Some("https://www.w3.org/ns/activitystreams"),
        "the AS2 context comes first"
    );
    let mut known = as2_terms();
    known.extend(local_terms(doc));
    walk(doc, &known, "$");
}

fn walk(v: &Value, known: &BTreeSet<String>, at: &str) {
    match v {
        Value::Object(o) => {
            for (k, child) in o {
                if k == "@context" {
                    continue;
                }
                assert!(
                    known.contains(k),
                    "{at}.{k} is not defined by the AS2 context or the document's own"
                );
                if k == "type" {
                    let t = child.as_str().expect("type is a string");
                    assert!(known.contains(t), "{at}.type {t} is not an AS2 type");
                }
                // IRIs must be absolute https.
                if matches!(
                    k.as_str(),
                    "id" | "url"
                        | "href"
                        | "inbox"
                        | "outbox"
                        | "first"
                        | "next"
                        | "partOf"
                        | "attributedTo"
                        | "actor"
                ) && let Some(s) = child.as_str()
                {
                    assert!(s.starts_with("https://"), "{at}.{k} = {s} is not https");
                }
                // Language maps (`contentMap`, `nameMap`, `summaryMap`) are
                // `@container: @language`: their keys are BCP 47 tags, not terms.
                if k.ends_with("Map") {
                    let m = child.as_object().expect("a language map is an object");
                    assert!(m.values().all(Value::is_string), "{at}.{k} holds strings");
                    continue;
                }
                walk(child, known, &format!("{at}.{k}"));
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                walk(x, known, &format!("{at}[{i}]"));
            }
        }
        _ => {}
    }
}

fn cfg() -> Config {
    Config::new("https://circles.mukoko.com")
}

fn circle() -> Circle {
    serde_json::from_str(
        r#"{"id":"c1","slug":"harare-runners","name":"Harare Runners",
            "description":"Saturday long runs.","circleType":"public","memberCount":312,
            "postCount":2,"createdAt":"2026-01-02T03:04:05Z","updatedAt":"2026-09-30T10:00:00Z",
            "imageUrl":"https://assets.mukoko.com/circles/harare-runners.png",
            "categories":[{"slug":"sport","name":"Sport"}]}"#,
    )
    .unwrap()
}

fn posts() -> Page<Post> {
    serde_json::from_str(
        r#"{"data":[
            {"id":"p1","headline":"Long run","articleBody":"See you at 6.","datePublished":"2026-09-01T06:00:00Z","inLanguage":"en","tags":["running"]},
            {"id":"p2","articleBody":"Rest week.","datePublished":"2026-08-25T06:00:00Z"}
        ],"nextCursor":"n1","total":2}"#,
    )
    .unwrap()
}

#[test]
fn actor_is_valid_as2() {
    let a = ap::actor(&cfg(), &circle());
    check(&a);
    // ActivityPub §4.1: an actor MUST have inbox and outbox, and the id must
    // be what it is fetched from.
    for k in ["id", "type", "inbox", "outbox", "preferredUsername", "name"] {
        assert!(a.get(k).is_some(), "actor.{k} is required");
    }
    assert_eq!(a["type"], "Group");
}

#[test]
fn outbox_is_valid_as2() {
    let o = ap::outbox(&cfg(), &circle(), Some(2));
    check(&o);
    assert_eq!(o["type"], "OrderedCollection");
    assert_eq!(o["totalItems"], 2);

    let p = ap::outbox_page(&cfg(), &circle(), &posts(), None);
    check(&p);
    assert_eq!(p["orderedItems"].as_array().unwrap().len(), 2);
}

#[test]
fn note_is_valid_as2() {
    let n = ap::note(&cfg(), &circle(), &posts().data[0]);
    check(&n);
    assert_eq!(n["to"][0], ap::AS_PUBLIC);
}

#[test]
fn extension_terms_map_to_iris() {
    // A local term must expand to an IRI through a prefix both contexts know.
    let ctx = ap::context();
    let local = ctx[1].as_object().unwrap();
    for (k, v) in local {
        let v = v.as_str().unwrap();
        assert!(
            v.starts_with("http") || v.starts_with("as:") || v.starts_with("toot:"),
            "{k} -> {v}"
        );
    }
    assert!(
        as2_terms().contains("as"),
        "the AS2 context defines the as: prefix"
    );
}
