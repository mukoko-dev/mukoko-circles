//! Deployment settings, read from Worker vars by the entry point and passed
//! in as plain values so everything else can be tested natively.

use crate::model::Circle;

#[derive(Clone, Debug)]
pub struct Config {
    /// `https://circles.mukoko.com`, no trailing slash. The ActivityPub
    /// identity domain: actor ids are minted under it.
    pub site_url: String,
    /// The host part of `site_url`, the WebFinger domain.
    pub host: String,
    /// The Nyuchi API base, e.g. `https://api.nyuchi.com`. `None` means not
    /// configured, and every data page answers 503 saying so.
    pub api_url: Option<String>,
    /// Where "Join" goes when the API sends no `links.join`. `{id}` and
    /// `{slug}` are substituted.
    pub join_url_template: String,
    /// Where "Create a circle" goes: the super app's create flow.
    pub create_url: String,
    /// The super app on the web, the fallback for everything.
    pub app_web_url: String,
    /// Store listings, shown only when set (the mobile super app is not
    /// published yet).
    pub app_store_url: Option<String>,
    pub play_store_url: Option<String>,
    pub version: String,
}

impl Config {
    pub fn new(site_url: &str) -> Self {
        let site_url = site_url.trim_end_matches('/').to_string();
        let host = url::Url::parse(&site_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_else(|| "circles.mukoko.com".into());
        Config {
            site_url,
            host,
            api_url: None,
            // Circles lives inside Mukoko Events (nhimbe) until the super app
            // ships; its circle page is keyed by the circle id.
            join_url_template: "https://events.mukoko.com/circles/{id}".into(),
            create_url: "https://events.mukoko.com/circles?create=1".into(),
            app_web_url: "https://events.mukoko.com".into(),
            app_store_url: None,
            play_store_url: None,
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    pub fn actor_uri(&self, slug: &str) -> String {
        format!("{}/c/{}", self.site_url, slug)
    }

    /// The circle's actor id. Always minted from this site's own domain: an
    /// `actorUri` from the API that points anywhere else is ignored, because
    /// an actor id is only meaningful on the host that serves it.
    pub fn circle_actor(&self, c: &Circle) -> String {
        self.actor_uri(&c.slug)
    }

    /// The link behind a circle's "Join" button: the API's universal link
    /// when it sent a safe one, else the configured template.
    pub fn join_url(&self, c: &Circle) -> String {
        if let Some(j) = c.links.join.as_deref().filter(|u| is_safe_https(u)) {
            return j.to_string();
        }
        self.join_url_template
            .replace("{id}", &c.id)
            .replace("{slug}", &c.slug)
    }

    /// The custom-scheme deep link, only when the API sent one under the
    /// `mukoko:` scheme.
    pub fn app_link(&self, c: &Circle) -> Option<String> {
        c.links
            .app
            .as_deref()
            .filter(|u| u.starts_with("mukoko://") && !u.contains(['"', '<', '>', ' ']))
            .map(str::to_string)
    }
}

/// `https:` with a host, and nothing that could break out of an attribute.
pub fn is_safe_https(u: &str) -> bool {
    match url::Url::parse(u) {
        Ok(p) => p.scheme() == "https" && p.host_str().is_some() && !u.contains(['"', '<', '>']),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Links;

    fn circle() -> Circle {
        Circle {
            id: "6f1c".into(),
            slug: "harare-runners".into(),
            name: "Harare Runners".into(),
            circle_type: "public".into(),
            ..Default::default()
        }
    }

    #[test]
    fn join_falls_back_to_template() {
        let cfg = Config::new("https://circles.mukoko.com/");
        assert_eq!(cfg.site_url, "https://circles.mukoko.com");
        assert_eq!(cfg.host, "circles.mukoko.com");
        assert_eq!(
            cfg.join_url(&circle()),
            "https://events.mukoko.com/circles/6f1c"
        );
    }

    #[test]
    fn join_uses_api_link_only_when_safe() {
        let cfg = Config::new("https://circles.mukoko.com");
        let mut c = circle();
        c.links = Links {
            join: Some("https://app.mukoko.com/circles/harare-runners".into()),
            app: Some("mukoko://circles/harare-runners".into()),
        };
        assert_eq!(
            cfg.join_url(&c),
            "https://app.mukoko.com/circles/harare-runners"
        );
        assert_eq!(cfg.app_link(&c).unwrap(), "mukoko://circles/harare-runners");
        c.links.join = Some("javascript:alert(1)".into());
        c.links.app = Some("javascript:alert(1)".into());
        assert_eq!(cfg.join_url(&c), "https://events.mukoko.com/circles/6f1c");
        assert!(cfg.app_link(&c).is_none());
    }

    #[test]
    fn actor_is_always_on_our_domain() {
        let cfg = Config::new("https://circles.mukoko.com");
        let mut c = circle();
        c.actor_uri = Some("https://evil.example/c/harare-runners".into());
        assert_eq!(
            cfg.circle_actor(&c),
            "https://circles.mukoko.com/c/harare-runners"
        );
    }
}
