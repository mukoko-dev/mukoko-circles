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
    pub slug: String,
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
    /// `https://circles.mukoko.com/c/{slug}`. Derived when absent.
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
        matches!(self.circle_type.as_str(), "public" | "broadcast") && is_valid_slug(&self.slug)
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
    fn missing_optional_fields_default() {
        let c: Circle =
            serde_json::from_str(r#"{"slug":"a","name":"A","circleType":"public"}"#).unwrap();
        assert_eq!(c.member_count, 0);
        assert!(c.links.join.is_none());
        assert_eq!(c.summary(), "A is a circle on Mukoko.");
    }
}
