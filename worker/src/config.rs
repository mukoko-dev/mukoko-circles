//! Deployment settings, read from Worker vars by the entry point and passed
//! in as plain values so everything else can be tested natively.

use crate::model::{Circle, is_valid_key};

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
    /// Where "Join" sends a phone: the Mukoko link,
    /// `https://mukoko.com/open/circles/{handle}`, whose choice page opens
    /// the super app (or its store listing). `{id}`, `{slug}` and `{handle}`
    /// (the circle's path key) are substituted.
    pub join_url_template: String,
    /// Where "Join" sends everything else: the circle in the web super app.
    /// A desktop browser must not go through `/open`, which sends desktops to
    /// the public page, i.e. back here. Circles live inside Mukoko Events
    /// until the super app ships, so today this is Events' circle page.
    pub web_join_url_template: String,
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
            join_url_template: "https://mukoko.com/open/circles/{handle}".into(),
            // Circles lives inside Mukoko Events until the super app ships;
            // its circle page is keyed by the circle id.
            web_join_url_template: "https://events.mukoko.com/circles/{id}".into(),
            create_url: "https://events.mukoko.com/circles?create=1".into(),
            app_web_url: "https://events.mukoko.com".into(),
            app_store_url: None,
            play_store_url: None,
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    /// The circle's page, `/c/{key}`: its current handle (or slug).
    pub fn circle_url(&self, c: &Circle) -> String {
        format!("{}/c/{}", self.site_url, c.key())
    }

    /// The circle's actor id. The API stores it once, minted from the
    /// circle's first address, and never rewrites it: other servers key
    /// follows and objects to it, so a renamed circle keeps it. It is used
    /// only when it is a circle path on this site; anything else (absent, or
    /// pointing at another host) falls back to the page URL, because an
    /// actor id is only meaningful on the host that serves it.
    pub fn circle_actor(&self, c: &Circle) -> String {
        let prefix = format!("{}/c/", self.site_url);
        match c.actor_uri.as_deref() {
            Some(a) if a.strip_prefix(&prefix).is_some_and(is_valid_key) => a.to_string(),
            _ => self.circle_url(c),
        }
    }

    /// The link behind a circle's "Join" button, by device: a phone gets the
    /// Mukoko link (`/open`, which offers the app), anything else the circle
    /// in the web super app.
    pub fn join_url(&self, c: &Circle, phone: bool) -> String {
        let template = if phone {
            &self.join_url_template
        } else {
            &self.web_join_url_template
        };
        template
            .replace("{id}", &c.id)
            .replace("{slug}", &c.slug)
            .replace("{handle}", &c.key())
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

/// A phone or tablet, by `User-Agent`: the devices where the Mukoko app can
/// be installed. Everything else (desktops, crawlers) is not.
pub fn is_phone(user_agent: Option<&str>) -> bool {
    let ua = user_agent.unwrap_or_default();
    ["iPhone", "iPad", "iPod", "Android", "Mobile"]
        .iter()
        .any(|m| ua.contains(m))
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
    fn join_by_device() {
        let cfg = Config::new("https://circles.mukoko.com/");
        assert_eq!(cfg.site_url, "https://circles.mukoko.com");
        assert_eq!(cfg.host, "circles.mukoko.com");
        let mut c = circle();
        assert_eq!(
            cfg.join_url(&c, true),
            "https://mukoko.com/open/circles/harare-runners"
        );
        assert_eq!(
            cfg.join_url(&c, false),
            "https://events.mukoko.com/circles/6f1c"
        );
        c.handle = Some("HarareRunners".into());
        assert_eq!(
            cfg.join_url(&c, true),
            "https://mukoko.com/open/circles/hararerunners"
        );
        // The API's own join link no longer decides: the Mukoko link does.
        c.links.join = Some("https://elsewhere.example/x".into());
        assert_eq!(
            cfg.join_url(&c, true),
            "https://mukoko.com/open/circles/hararerunners"
        );
    }

    #[test]
    fn phones() {
        assert!(is_phone(Some(
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Mobile/15E148 Safari/604.1"
        )));
        assert!(is_phone(Some(
            "Mozilla/5.0 (Linux; Android 14; Pixel 8) Chrome/129.0 Mobile Safari/537.36"
        )));
        assert!(!is_phone(Some(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Chrome/129.0 Safari/537.36"
        )));
        assert!(!is_phone(None));
    }

    #[test]
    fn app_link_only_mukoko_scheme() {
        let cfg = Config::new("https://circles.mukoko.com");
        let mut c = circle();
        c.links = Links {
            join: None,
            app: Some("mukoko://circles/harare-runners".into()),
        };
        assert_eq!(cfg.app_link(&c).unwrap(), "mukoko://circles/harare-runners");
        c.links.app = Some("javascript:alert(1)".into());
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
        c.actor_uri = Some("https://circles.mukoko.com/c/../x".into());
        assert_eq!(
            cfg.circle_actor(&c),
            "https://circles.mukoko.com/c/harare-runners"
        );
    }

    #[test]
    fn a_renamed_circle_keeps_its_stored_actor() {
        let cfg = Config::new("https://circles.mukoko.com");
        let mut c = circle();
        c.handle = Some("HarareRunClub".into());
        c.actor_uri = Some("https://circles.mukoko.com/c/harare-runners".into());
        assert_eq!(
            cfg.circle_actor(&c),
            "https://circles.mukoko.com/c/harare-runners"
        );
        assert_eq!(
            cfg.circle_url(&c),
            "https://circles.mukoko.com/c/hararerunclub"
        );
    }
}
