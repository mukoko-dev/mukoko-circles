//! Open Graph cards, rendered by the nyuchi-tools image pipeline.
//!
//! One renderer for the estate: `nyuchi-imaging` (resvg, the embedded Noto
//! faces, the `og` preset at 1200×630) draws the card from a headline, a line
//! of text and a few points. This module only decides what goes on it, in a
//! Mukoko theme: tanzanite for the super app, terracotta for Circles.
//!
//! The entry point caches each PNG at the edge, keyed by the circle's slug and
//! `updatedAt`, so a card is drawn once per change, not once per unfurl.

use crate::config::Config;
use crate::model::Circle;
use crate::pages::{count, truncate};
use nyuchi_imaging::layout::Content;
use nyuchi_imaging::preset;
use nyuchi_imaging::privacy::Privacy;
use nyuchi_imaging::render::{self, Request};
use nyuchi_imaging::theme::{Mode, Surface, Theme};
use std::sync::OnceLock;

/// The Mukoko Circles theme. Colours from Mzizi (`mzizi_get_tokens`):
/// the background ladder as nyuchi-tools' own themes use it, tanzanite
/// (Mukoko) and terracotta (Circles) for the mark and the mesh.
fn theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| Theme {
        id: "mukoko-circles".into(),
        name: "Mukoko Circles".into(),
        eyebrow: "Mukoko Circles".into(),
        byline: "mukoko".into(),
        mark: vec!["tanzanite".into(), "terracotta".into(), "tanzanite".into()],
        mesh: vec!["tanzanite".into(), "terracotta".into(), "cobalt".into()],
        accent: "tanzanite".into(),
        logo: None,
        light: Surface {
            bg: "#F3F3F1".into(),
            fg: "#0C0C0A".into(),
            muted: "#4F4E4A".into(),
            card: "#FFFFFF".into(),
            edge: "#E5E4E1".into(),
            skeleton: "#E5E4E1".into(),
            mesh_opacity: 0.2,
        },
        dark: Surface {
            bg: "#0E0D0C".into(),
            fg: "#F0EFE9".into(),
            muted: "#A8A6A0".into(),
            card: "#FFFFFF".into(),
            edge: "#2E2C29".into(),
            skeleton: "#E5E4E1".into(),
            mesh_opacity: 0.3,
        },
    })
}

/// What the card for one circle says.
pub fn circle_content(cfg: &Config, c: &Circle) -> Content {
    let mut points = vec![count(c.member_count, "member", "members")];
    points.push(if c.is_broadcast() {
        "Broadcast circle".into()
    } else {
        "Open to everyone".into()
    });
    if let Some(cat) = c.categories.first() {
        points.push(cat.name.clone());
    }
    Content {
        eyebrow: Some("Mukoko Circles".into()),
        headline: truncate(&c.name, 60),
        sub: Some(truncate(&c.summary(), 120)),
        points,
        cta: Some(format!("{}/c/{}", cfg.host, c.slug)),
        ..Content::default()
    }
}

/// The card for the home page and every list page.
pub fn home_content(cfg: &Config) -> Content {
    Content {
        eyebrow: Some("Mukoko Circles".into()),
        headline: "Find your people. Keep them close.".into(),
        sub: Some("Communities on Mukoko, Africa's privacy-first social super-app.".into()),
        points: vec![
            "Fan clubs, neighbourhoods, professions".into(),
            "Join in the Mukoko app".into(),
            "Follow from the fediverse".into(),
        ],
        cta: Some(cfg.host.clone()),
        ..Content::default()
    }
}

/// Draw a card as PNG bytes.
pub fn render_png(content: &Content) -> Result<Vec<u8>, String> {
    let preset = preset::get("og").ok_or("the og preset is missing from nyuchi-imaging")?;
    let rendered = render::render(&Request {
        preset,
        theme: theme(),
        mode: Mode::Light,
        content,
        // No screenshot is ever drawn, so there is nothing to attest; circle
        // names are public by definition (public and broadcast circles only).
        privacy: &Privacy::default(),
        guides: false,
    })
    .map_err(|e| e.to_string())?;
    Ok(rendered.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_1200_by_630_png() {
        let cfg = Config::new("https://circles.mukoko.com");
        let c: Circle = serde_json::from_str(
            r#"{"id":"c1","slug":"harare-runners","name":"Harare Runners","circleType":"public",
                "description":"Saturday long runs and coffee after.","memberCount":1234,
                "categories":[{"slug":"sport","name":"Sport"}]}"#,
        )
        .unwrap();
        let png = render_png(&circle_content(&cfg, &c)).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // IHDR width and height, big-endian.
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1200);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 630);
        let home = render_png(&home_content(&cfg)).unwrap();
        assert_eq!(&home[..4], b"\x89PNG");
    }
}
