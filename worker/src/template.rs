//! Filling the Astro-built page shells.
//!
//! The pages are designed in Astro (`src/pages/tpl/*.astro`, on `@bundu/ui`)
//! and built to static HTML with placeholders in them. This Worker fills the
//! placeholders with live data from the API. The markup, the styles and the
//! design system stay in Astro; the logic stays in Rust.
//!
//! Two kinds of placeholder:
//!
//! - `{{key}}` is replaced by the value, HTML-escaped. Safe in text and in
//!   quoted attributes.
//! - `{{{key}}}` is replaced by trusted HTML, verbatim: a fragment this
//!   module already filled (a list of cards), or JSON-LD from
//!   [`json_for_script`].
//!
//! Filling is a single pass over the template, so nothing inserted is ever
//! scanned again: a circle called `{{title}}` stays a circle called that.
//! A key with no value becomes the empty string.

use std::collections::HashMap;

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Values for one fill.
#[derive(Default)]
pub struct Vars {
    text: HashMap<String, String>,
    html: HashMap<String, String>,
}

impl Vars {
    pub fn new() -> Self {
        Self::default()
    }

    /// A plain-text value for `{{key}}`.
    pub fn text(mut self, key: &str, value: impl Into<String>) -> Self {
        self.text.insert(key.into(), value.into());
        self
    }

    /// Trusted HTML for `{{{key}}}`.
    pub fn html(mut self, key: &str, value: impl Into<String>) -> Self {
        self.html.insert(key.into(), value.into());
        self
    }
}

pub fn fill(tpl: &str, vars: &Vars) -> String {
    let mut out = String::with_capacity(tpl.len() + 1024);
    let mut rest = tpl;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if let Some(inner) = after.strip_prefix("{{{")
            && let Some(end) = inner.find("}}}")
        {
            let key = inner[..end].trim();
            if let Some(v) = vars.html.get(key) {
                out.push_str(v);
            }
            rest = &inner[end + 3..];
            continue;
        }
        let inner = &after[2..];
        match inner.find("}}") {
            Some(end) if is_key(&inner[..end]) => {
                if let Some(v) = vars.text.get(inner[..end].trim()) {
                    out.push_str(&escape(v));
                }
                rest = &inner[end + 2..];
            }
            _ => {
                out.push_str("{{");
                rest = inner;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_key(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// JSON for an inline `<script type="application/ld+json">`. `<`, `>` and
/// `&` are escaped as JSON unicode escapes so no string can close the script
/// element.
pub fn json_for_script(v: &serde_json::Value) -> String {
    serde_json::to_string(v)
        .unwrap_or_else(|_| "{}".into())
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

/// The Astro build emits the fragment templates (a card, a chip) as their own
/// pages; this takes the part between the markers so any wrapper Astro added
/// is ignored.
pub fn fragment(page: &str) -> &str {
    const START: &str = "<!--fragment-->";
    const END: &str = "<!--/fragment-->";
    match (page.find(START), page.rfind(END)) {
        (Some(s), Some(e)) if e > s => page[s + START.len()..e].trim(),
        _ => page.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_text_and_keeps_html() {
        let v = Vars::new()
            .text("name", "<b>\"Tom & Jerry's\"</b>")
            .html("cards", "<li>ok</li>");
        assert_eq!(
            fill(
                "<h1 title=\"{{name}}\">{{ name }}</h1><ul>{{{cards}}}</ul>",
                &v
            ),
            "<h1 title=\"&lt;b&gt;&quot;Tom &amp; Jerry&#39;s&quot;&lt;/b&gt;\">&lt;b&gt;&quot;Tom &amp; Jerry&#39;s&quot;&lt;/b&gt;</h1><ul><li>ok</li></ul>"
        );
    }

    #[test]
    fn never_rescans_values() {
        let v = Vars::new().text("a", "{{b}}").text("b", "boom");
        assert_eq!(fill("{{a}}", &v), "{{b}}");
    }

    #[test]
    fn missing_keys_are_empty_and_stray_braces_survive() {
        let v = Vars::new();
        assert_eq!(fill("x{{nope}}y{{{nope}}}z", &v), "xyz");
        assert_eq!(
            fill("a {{ not a key! }} b {{", &v),
            "a {{ not a key! }} b {{"
        );
    }

    #[test]
    fn script_json_cannot_close_the_element() {
        let s = json_for_script(&serde_json::json!({"n": "</script><x>&"}));
        assert!(!s.contains('<') && !s.contains('>'));
    }

    #[test]
    fn fragments() {
        assert_eq!(
            fragment("<html><!--fragment--> <li>{{x}}</li> <!--/fragment--></html>"),
            "<li>{{x}}</li>"
        );
        assert_eq!(fragment(" <li/> "), "<li/>");
    }
}
