//! The website's theme studio starts from the launcher's own themes and checks
//! the launcher's contrast rules. It reads them from
//! `web/src/data/themes.generated.json`, written from here so the two cannot
//! drift: `just website-themes` rewrites it.

use std::path::Path;

use serde::Serialize;

use super::super::spec::{DEFAULT_RADIUS, DEFAULT_RADIUS_LG, MAX_RADIUS};
use super::super::{Catalog, Palette, ThemeSpec};
use super::{RULES, Rule};

const FILE: &str = "../../web/src/data/themes.generated.json";

#[derive(Serialize)]
struct Website {
    /// Colour names in the order the file and the docs list them.
    colors: &'static [&'static str],
    radius: f32,
    radius_lg: f32,
    max_radius: f32,
    contrast: &'static [Rule],
    /// The built-in themes, in the order the launcher lists them.
    themes: Vec<ThemeSpec>,
}

fn website() -> String {
    let themes = Catalog::default()
        .entries
        .into_iter()
        .map(|entry| entry.spec)
        .collect();
    let website = Website {
        colors: Palette::NAMES,
        radius: DEFAULT_RADIUS,
        radius_lg: DEFAULT_RADIUS_LG,
        max_radius: MAX_RADIUS,
        contrast: RULES,
        themes,
    };
    serde_json::to_string_pretty(&website).expect("plain data serializes") + "\n"
}

#[test]
fn the_website_has_the_current_builtin_themes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(FILE);
    let wanted = website();
    if std::env::var_os("LUMILIO_BLESS_THEMES").is_some() {
        std::fs::write(&path, &wanted).expect("the website data folder is writable");
    }
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        current == wanted,
        "{FILE} is not current; run `just website-themes`"
    );
}
