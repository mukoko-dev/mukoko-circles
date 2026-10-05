//! The shapes the Nyuchi API's public discovery endpoint returns.
//!
//! The contract is `GET /v1/circles/discover` and friends, proposed on
//! nyuchi/api-gateway#197 and written out in `docs/api-contract.md`. Parsing
//! is lenient on purpose: an optional field the API has not shipped yet
//! defaults rather than failing the page, and anything this site derives
//! itself (the actor URI, the join link) is filled in here when it is absent.
//!
//! What parsing is NOT lenient about is privacy. A circle whose `circleType`
//! is anything but `public` or `broadcast` is dropped by [`Circle::is_discoverable`]
//! even if the API sent it, so a bug upstream cannot publish a private circle.

use serde::{Deserialize, Serialize};

/// One discoverable circle, as the discovery endpoint returns it.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Circle {
    pub id: String,
    /// The permanent URL key (`a-z0-9-`). The API resolves it for ever, so
    /// every follow-up API call (posts, one post) uses it.
    pub slug: String,
    /// The circle's current Mukoko handle (one namespace with persons and
    /// entities, 3 to 30 of `A-Za-z0-9_`, in the case its admins chose).
    /// When present it is the circle's address: `/c/{handle}` (lower-cased)
    /// and `acct:{handle}@circles.mukoko.com`. Until the API sends one, the
    /// slug is the address.
    pub handle: Option<String>,
    /// Other names this circle answers to: retired handles (a retired claim
    /// is never reclaimable) and, once it has a handle, its slug. A request
    /// on an alias is redirected (HTML) or answered with the same actor.
    pub aliases: Vec<String>,
    pub name: String,
    pub description: Option<String>,
    /// `public` or `broadcast`. Anything else never renders.
    pub circle_type: String,
    pub member_count: u64,
    pub post_count: u64,
    pub in_language: Option<String>,
    pub categories: Vec<CategoryRef>,
    pub image_url: Option<String>,
    pub featured: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub place: Option<Place>,
    /// The stored actor id, minted once from the circle's first address and
    /// never rewritten (`https://circles.mukoko.com/c/{first handle or slug}`).
    /// Used as-is when it is on this site; derived from [`Circle::key`] when
    /// absent.
    pub actor_uri: Option<String>,
    pub links: Links,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Links {
    /// The https link that opens this circle in the Mukoko super app (a
    /// universal / app link: the app opens it when installed, the web app
    /// otherwise).
    pub join: Option<String>,
    /// The custom-scheme deep link, e.g. `mukoko://circles/{slug}`.
    pub app: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Place {
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CategoryRef {
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Category {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub circle_count: u64,
}

/// A public post: approved by the circle's moderators, in a public or
/// broadcast circle. The API never sends the author's identity here, and this
/// site never asks for it: on the fediverse a post is the circle's.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Post {
    pub id: String,
    pub headline: Option<String>,
    pub article_body: String,
    pub date_published: Option<String>,
    pub in_language: Option<String>,
    pub tags: Vec<String>,
}

/// A page of results.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Page<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
    pub total: Option<u64>,
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Page {
            data: Vec::new(),
            next_cursor: None,
            total: None,
        }
    }
}

impl Circle {
    /// Only `public` and `broadcast` circles are ever shown or federated.
    pub fn is_discoverable(&self) -> bool {
        matches!(self.circle_type.as_str(), "public" | "broadcast")
            && is_valid_slug(&self.slug)
            && is_valid_key(&self.key())
    }

    /// The handle as people see it (`preferredUsername`, the `acct:`):
    /// the current handle in its chosen case, else the slug.
    pub fn username(&self) -> String {
        match self.handle.as_deref().map(str::trim) {
            Some(h) if is_valid_handle(h) => h.to_string(),
            _ => self.slug.clone(),
        }
    }

    /// The path key: `/c/{key}`. The username, lower-cased.
    pub fn key(&self) -> String {
        self.username().to_ascii_lowercase()
    }

    /// Whether `requested` (already lower-cased) names this circle: its key,
    /// its slug, or one of its aliases. Anything else means the API answered
    /// for a different circle, and the page is a 404.
    pub fn answers_to(&self, requested: &str) -> bool {
        requested == self.key()
            || requested == self.slug
            || self
                .aliases
                .iter()
                .any(|a| a.trim().eq_ignore_ascii_case(requested))
    }

    pub fn is_broadcast(&self) -> bool {
        self.circle_type == "broadcast"
    }

    /// The description, or a sentence that still says something true.
    pub fn summary(&self) -> String {
        match self.description.as_deref().map(str::trim) {
            Some(d) if !d.is_empty() => d.to_string(),
            _ => format!("{} is a circle on Mukoko.", self.name),
        }
    }
}

impl Page<Circle> {
    /// Drop anything that is not discoverable. Defence in depth: the API
    /// filters too, but this site is the one publishing to search engines and
    /// the fediverse.
    pub fn discoverable(mut self) -> Self {
        self.data.retain(Circle::is_discoverable);
        self
    }
}

/// `a-z`, `0-9` and inner hyphens, 1 to 64 characters. Also the WebFinger
/// user part, so it must stay URL-safe and case-folded.
pub fn is_valid_slug(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

/// A Mukoko handle: 3 to 30 of `A-Za-z0-9_` (the API's handle rules).
pub fn is_valid_handle(s: &str) -> bool {
    (3..=30).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// A circle's path key, lower-case: a slug or a lower-cased handle. `a-z`,
/// `0-9`, `-` and `_`, 1 to 64 characters, not starting or ending with `-`.
/// Also the WebFinger user part, so it stays URL-safe.
pub fn is_valid_key(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-' || *c == b'_')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

/// Opaque ids from the API (UUIDs today) that end up in URLs.
pub fn is_valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        for ok in ["a", "harare-runners", "zim-tech-2026", "x1"] {
            assert!(is_valid_slug(ok), "{ok}");
        }
        for bad in [
            "",
            "-a",
            "a-",
            "A",
            "a_b",
            "a.b",
            "a/b",
            "ä",
            &"a".repeat(65),
        ] {
            assert!(!is_valid_slug(bad), "{bad}");
        }
    }

    #[test]
    fn private_and_secret_circles_are_dropped() {
        let page: Page<Circle> = serde_json::from_str(
            r#"{"data":[
                {"slug":"open","name":"Open","circleType":"public"},
                {"slug":"loud","name":"Loud","circleType":"broadcast"},
                {"slug":"closed","name":"Closed","circleType":"private"},
                {"slug":"hidden","name":"Hidden","circleType":"secret"},
                {"slug":"Bad Slug","name":"Bad","circleType":"public"}
            ]}"#,
        )
        .unwrap();
        let slugs: Vec<_> = page
            .discoverable()
            .data
            .into_iter()
            .map(|c| c.slug)
            .collect();
        assert_eq!(slugs, ["open", "loud"]);
    }

    #[test]
    fn handles_are_the_address() {
        let c: Circle = serde_json::from_str(
            r#"{"slug":"harare-runners","handle":"HarareRunners","aliases":["HRunners"],"name":"H","circleType":"public"}"#,
        )
        .unwrap();
        assert_eq!(c.username(), "HarareRunners");
        assert_eq!(c.key(), "hararerunners");
        assert!(c.is_discoverable());
        assert!(c.answers_to("hararerunners"));
        assert!(c.answers_to("harare-runners"));
        assert!(c.answers_to("hrunners"));
        assert!(!c.answers_to("someone-else"));
        // No handle (or a malformed one): the slug is the address.
        let s: Circle = serde_json::from_str(
            r#"{"slug":"harare-runners","handle":"no spaces!","name":"H","circleType":"public"}"#,
        )
        .unwrap();
        assert_eq!(s.username(), "harare-runners");
        assert_eq!(s.key(), "harare-runners");
    }

    #[test]
    fn keys() {
        for ok in ["a", "harare-runners", "harare_runners", "x1_"] {
            assert!(is_valid_key(ok), "{ok}");
        }
        for bad in ["", "-a", "a-", "A", "a.b", "a/b", "ä", &"a".repeat(65)] {
            assert!(!is_valid_key(bad), "{bad}");
        }
        assert!(is_valid_handle("Tendai_M"));
        assert!(!is_valid_handle("ab"));
        assert!(!is_valid_handle("a-b-c"));
    }

    #[test]
    fn missing_optional_fields_default() {
        let c: Circle =
            serde_json::from_str(r#"{"slug":"a","name":"A","circleType":"public"}"#).unwrap();
        assert_eq!(c.member_count, 0);
        assert!(c.links.join.is_none());
        assert_eq!(c.summary(), "A is a circle on Mukoko.");
    }
}
