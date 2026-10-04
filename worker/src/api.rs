//! The Nyuchi API calls this site makes, as URLs. The only data source:
//! no database, no other service. The contract is in `docs/api-contract.md`.

use crate::ap::encode;

#[derive(Default)]
pub struct Discover<'a> {
    pub q: Option<&'a str>,
    pub category: Option<&'a str>,
    pub featured: bool,
    pub limit: u32,
    pub cursor: Option<&'a str>,
}

fn base(api: &str) -> String {
    format!("{}/v1/circles/discover", api.trim_end_matches('/'))
}

/// `GET /v1/circles/discover`
pub fn discover(api: &str, d: &Discover) -> String {
    let mut params: Vec<String> = Vec::new();
    if let Some(q) = d.q.filter(|q| !q.is_empty()) {
        params.push(format!("q={}", encode(q)));
    }
    if let Some(c) = d.category {
        params.push(format!("category={}", encode(c)));
    }
    if d.featured {
        params.push("featured=true".into());
    }
    params.push(format!("limit={}", d.limit.clamp(1, 50)));
    if let Some(c) = d.cursor {
        params.push(format!("cursor={}", encode(c)));
    }
    format!("{}?{}", base(api), params.join("&"))
}

/// `GET /v1/circles/discover/categories`
pub fn categories(api: &str) -> String {
    format!("{}/categories", base(api))
}

/// `GET /v1/circles/discover/{slug}`
pub fn circle(api: &str, slug: &str) -> String {
    format!("{}/{}", base(api), encode(slug))
}

/// `GET /v1/circles/discover/{slug}/posts`
pub fn posts(api: &str, slug: &str, limit: u32, cursor: Option<&str>) -> String {
    let mut u = format!(
        "{}/{}/posts?limit={}",
        base(api),
        encode(slug),
        limit.clamp(1, 50)
    );
    if let Some(c) = cursor {
        u.push_str(&format!("&cursor={}", encode(c)));
    }
    u
}

/// `GET /v1/circles/discover/{slug}/posts/{id}`
pub fn post(api: &str, slug: &str, id: &str) -> String {
    format!("{}/{}/posts/{}", base(api), encode(slug), encode(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        let api = "https://api.nyuchi.com/";
        assert_eq!(
            discover(
                api,
                &Discover {
                    q: Some("run & cycle"),
                    limit: 24,
                    ..Default::default()
                }
            ),
            "https://api.nyuchi.com/v1/circles/discover?q=run+%26+cycle&limit=24"
        );
        assert_eq!(
            discover(
                api,
                &Discover {
                    category: Some("sport"),
                    featured: true,
                    limit: 500,
                    cursor: Some("a=b"),
                    ..Default::default()
                }
            ),
            "https://api.nyuchi.com/v1/circles/discover?category=sport&featured=true&limit=50&cursor=a%3Db"
        );
        assert_eq!(
            categories(api),
            "https://api.nyuchi.com/v1/circles/discover/categories"
        );
        assert_eq!(
            circle(api, "a"),
            "https://api.nyuchi.com/v1/circles/discover/a"
        );
        assert_eq!(
            posts(api, "a", 20, Some("x")),
            "https://api.nyuchi.com/v1/circles/discover/a/posts?limit=20&cursor=x"
        );
        assert_eq!(
            post(api, "a", "p1"),
            "https://api.nyuchi.com/v1/circles/discover/a/posts/p1"
        );
    }
}
