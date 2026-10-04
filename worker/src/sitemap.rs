//! `/sitemap.xml`: the static pages, every category and every discoverable
//! circle.

use crate::config::Config;
use crate::model::{Category, Circle};
use crate::template::escape;

/// The static pages Astro builds, besides the home page.
pub const STATIC_PATHS: [&str; 3] = ["/circles", "/about", "/fediverse"];

pub fn sitemap(cfg: &Config, categories: &[Category], circles: &[Circle]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    let mut url = |loc: String, lastmod: Option<&str>| {
        out.push_str("  <url><loc>");
        out.push_str(&escape(&loc));
        out.push_str("</loc>");
        if let Some(m) = lastmod.and_then(|m| m.get(..10)) {
            out.push_str("<lastmod>");
            out.push_str(&escape(m));
            out.push_str("</lastmod>");
        }
        out.push_str("</url>\n");
    };
    url(format!("{}/", cfg.site_url), None);
    for p in STATIC_PATHS {
        url(format!("{}{p}", cfg.site_url), None);
    }
    for c in categories
        .iter()
        .filter(|c| crate::model::is_valid_slug(&c.slug))
    {
        url(format!("{}/categories/{}", cfg.site_url, c.slug), None);
    }
    for c in circles.iter().filter(|c| c.is_discoverable()) {
        url(
            cfg.circle_actor(c),
            c.updated_at.as_deref().or(c.created_at.as_deref()),
        );
    }
    out.push_str("</urlset>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_discoverable_circles() {
        let cfg = Config::new("https://circles.mukoko.com");
        let circles: Vec<Circle> = serde_json::from_str(
            r#"[{"slug":"a","name":"A","circleType":"public","updatedAt":"2026-09-30T10:00:00Z"},
                {"slug":"b","name":"B","circleType":"secret"}]"#,
        )
        .unwrap();
        let cats = vec![Category {
            slug: "sport".into(),
            name: "Sport".into(),
            ..Default::default()
        }];
        let xml = sitemap(&cfg, &cats, &circles);
        assert!(
            xml.contains("<loc>https://circles.mukoko.com/c/a</loc><lastmod>2026-09-30</lastmod>")
        );
        assert!(xml.contains("<loc>https://circles.mukoko.com/categories/sport</loc>"));
        assert!(!xml.contains("/c/b"));
        assert!(xml.ends_with("</urlset>\n"));
    }
}
