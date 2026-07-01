//! Parser for the Vault's system list (`#subMenu` on `/vault`).
//!
//! [`System`] is defined in `model.rs`; this module provides the
//! HTML scraping logic and a snapshot test against a local fixture.

use regex::Regex;

use crate::model::System;

/// Parse the 33 consoles from the `/vault` page's `#subMenu` section.
///
/// Extracts slug (`/vault/{slug}`), display name (link text), and
/// launch year (`title="Launched YYYY"`).
///
/// # Panics
///
/// Panics if `#subMenu a` is not a valid CSS selector (it is a
/// compile-time constant and therefore always valid).
#[must_use]
pub fn parse(html: &str) -> Vec<System> {
    let doc = scraper::Html::parse_document(html);
    let selector =
        scraper::Selector::parse("#subMenu a").expect("'#subMenu a' is a valid CSS selector");
    let year_re = Regex::new(r"Launched (\d{4})").expect("valid regex");
    let base_url = url::Url::parse("https://vimm.net").expect("valid base URL");

    doc.select(&selector)
        .filter_map(|el| {
            let href = el.value().attr("href")?;
            // Resolve both relative and absolute URLs, then validate the path.
            let resolved = base_url.join(href).ok()?;
            let path = resolved.path().to_string();
            let slug = path.strip_prefix("/vault/")?;
            if slug.is_empty() || slug.contains('/') {
                return None;
            }
            let name = el.text().collect::<String>().trim().to_string();
            let launch_year = el
                .value()
                .attr("title")
                .and_then(|t| year_re.captures(t))
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            Some(System {
                slug: slug.to_string(),
                name,
                launch_year,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_systems_from_vault_submenu() {
        let html = include_str!("../../../tests/fixtures/vault_home.html");
        let systems = parse(html);
        assert_eq!(systems.len(), 8);

        // First entry in document order.
        assert_eq!(systems[0].slug, "NES");
        assert_eq!(systems[0].name, "Nintendo Entertainment System");
        assert_eq!(systems[0].launch_year, 1983);

        // Middle entries.
        let nes = systems
            .iter()
            .find(|s| s.slug == "NES")
            .expect("NES present");
        assert_eq!(nes.name, "Nintendo Entertainment System");
        assert_eq!(nes.launch_year, 1983);

        let x360 = systems
            .iter()
            .find(|s| s.slug == "X360-D")
            .expect("X360-D present");
        assert_eq!(x360.name, "Xbox 360 (Digital)");
        assert_eq!(x360.launch_year, 2010);
    }

    #[test]
    fn returns_empty_on_no_submenu() {
        let html = "<html><body>no submenu</body></html>";
        assert!(parse(html).is_empty());
    }
}
