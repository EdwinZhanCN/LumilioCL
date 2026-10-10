//! The look in force: the catalog of themes the launcher knows, what the
//! person chose, and the one theme that results for the appearance showing
//! now. Pages never read any of this directly; they ask [`Body::current`] and
//! [`ShellColors::current`](super::ShellColors::current).

use std::path::{Path, PathBuf};

use gpui::{App, Global};
use lumilio_core::LookPreferences;

use super::spec::{Resolved, ThemeSpec, Tone, parse_file};
use super::{Body, MONO_FONT, SANS_FONT};

/// Name of the built-in light theme.
pub const ALUMINIUM: &str = "Aluminium";
/// Name of the built-in dark theme.
pub const NIGHT: &str = "Night";

/// Where a theme came from.
#[derive(Clone, Debug, PartialEq)]
pub enum Origin {
    /// Shipped with the launcher.
    Builtin,
    /// A file in the launcher's `themes` directory.
    Local(PathBuf),
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub spec: ThemeSpec,
    pub origin: Origin,
}

/// A theme file (or one theme inside it) the launcher could not use, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Rejected {
    pub file: PathBuf,
    pub reason: String,
}

/// Every theme the launcher knows. Set once at start and again whenever the
/// `themes` directory is rescanned.
#[derive(Clone, Debug)]
pub struct Catalog {
    pub entries: Vec<Entry>,
    pub rejected: Vec<Rejected>,
}

impl Global for Catalog {}

impl Default for Catalog {
    fn default() -> Self {
        Self::with_local(Vec::new(), Vec::new())
    }
}

impl Catalog {
    /// The built-in themes followed by what a scan of the local directory found.
    #[must_use]
    pub fn with_local(local: Vec<Entry>, rejected: Vec<Rejected>) -> Self {
        let mut entries = builtin();
        for entry in local {
            // A local theme never replaces a built-in or an earlier local one
            // of the same name and tone.
            let taken = entries.iter().any(|known| {
                known.spec.name == entry.spec.name && known.spec.appearance == entry.spec.appearance
            });
            if !taken {
                entries.push(entry);
            }
        }
        Self { entries, rejected }
    }

    /// The themes that fit one appearance, built-ins first.
    pub fn of_tone(&self, tone: Tone) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(move |entry| entry.spec.appearance == tone)
    }

    /// The named theme of this tone, or the built-in one when the name is
    /// absent or no longer resolves.
    #[must_use]
    pub fn resolve(&self, name: Option<&str>, tone: Tone) -> Resolved {
        let found = name.and_then(|name| self.of_tone(tone).find(|entry| entry.spec.name == name));
        let entry = found.unwrap_or_else(|| {
            let builtin = if tone.is_dark() { NIGHT } else { ALUMINIUM };
            self.of_tone(tone)
                .find(|entry| entry.spec.name == builtin)
                .expect("the built-in themes are always in the catalog")
        });
        Resolved::new(&entry.spec)
    }
}

/// Theme files shipped with the launcher: other people's palettes, kept as
/// they published them. Each theme names its source and licence.
const BUNDLED: [&str; 2] = [
    include_str!("../../themes/catppuccin.json"),
    include_str!("../../themes/rose-pine.json"),
];

/// The launcher's own themes, written through the same structure as any
/// other, then the bundled ones.
fn builtin() -> Vec<Entry> {
    let own = [
        ThemeSpec::of_body(ALUMINIUM, Body::of(false)),
        ThemeSpec::of_body(NIGHT, Body::of(true)),
    ];
    let bundled = BUNDLED.iter().flat_map(|text| {
        parse_file(text)
            .expect("bundled theme files are valid (tested)")
            .into_iter()
            .map(|theme| theme.expect("bundled themes are valid (tested)"))
    });
    own.into_iter()
        .chain(bundled)
        .map(|spec| Entry {
            spec,
            origin: Origin::Builtin,
        })
        .collect()
}

/// Reads every `*.json` theme file in `dir`. Blocking: call it off the UI
/// thread. A missing directory is an empty catalog, not an error.
#[must_use]
pub fn scan_dir(dir: &Path) -> (Vec<Entry>, Vec<Rejected>) {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    let (mut entries, mut rejected) = (Vec::new(), Vec::new());
    for path in paths {
        let reject = |reason: String| Rejected {
            file: path.clone(),
            reason,
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                rejected.push(reject(error.to_string()));
                continue;
            }
        };
        match parse_file(&text) {
            Err(error) => rejected.push(reject(error)),
            Ok(themes) => {
                for theme in themes {
                    match theme {
                        Ok(spec) => entries.push(Entry {
                            spec,
                            origin: Origin::Local(path.clone()),
                        }),
                        Err(error) => rejected.push(reject(error)),
                    }
                }
            }
        }
    }
    (entries, rejected)
}

/// What the person chose, as the preferences last said.
#[derive(Clone, Debug, Default)]
pub struct Choice(pub LookPreferences);

impl Global for Choice {}

/// The picture behind Home, once it is known to be there.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wallpaper {
    /// The file to show; `None` shows the procedural world.
    pub path: Option<PathBuf>,
    /// A picture was chosen but its file is gone.
    pub missing: bool,
}

impl Global for Wallpaper {}

/// The look for the appearance showing now.
#[derive(Clone, Debug)]
pub struct Look {
    pub theme: Resolved,
    pub sans: String,
    pub mono: String,
    /// The Chinese face to try before the system's, when one is chosen and
    /// installed.
    pub cjk: Option<String>,
    /// A Chinese face was asked for but is not installed.
    pub cjk_missing: bool,
    /// Interface scale in percent, inside the allowed range.
    pub scale_percent: u16,
}

impl Global for Look {}

impl Look {
    /// The look for `tone` from the catalog and the person's choice. Fonts
    /// are taken in this order: the person's, the theme's, the launcher's.
    #[must_use]
    pub fn resolve(cx: &App, tone: Tone) -> Self {
        let choice = cx.try_global::<Choice>().map(|choice| &choice.0);
        let defaults = LookPreferences::default();
        let choice = choice.unwrap_or(&defaults);
        let name = if tone.is_dark() {
            choice.dark_theme.as_deref()
        } else {
            choice.light_theme.as_deref()
        };
        let theme = match cx.try_global::<Catalog>() {
            Some(catalog) => catalog.resolve(name, tone),
            None => Catalog::default().resolve(name, tone),
        };
        let wanted =
            |own: &Option<String>, theme: &Option<String>| own.clone().or_else(|| theme.clone());
        let sans = wanted(&choice.sans_font, &theme.fonts.sans);
        let mono = wanted(&choice.mono_font, &theme.fonts.mono);
        let cjk = wanted(&choice.cjk_font, &theme.fonts.cjk);
        let installed = (sans.is_some() || mono.is_some() || cjk.is_some())
            .then(|| cx.text_system().all_font_names());
        let has = |family: &str| {
            installed
                .as_ref()
                .is_some_and(|names| names.iter().any(|name| name == family))
        };
        let usable = |family: Option<String>, default: &str| {
            family
                .filter(|family| has(family))
                .unwrap_or_else(|| default.to_owned())
        };
        let cjk_missing = cjk.as_ref().is_some_and(|family| !has(family));
        Self {
            sans: usable(sans, SANS_FONT),
            mono: usable(mono, MONO_FONT),
            cjk: cjk.filter(|family| has(family)),
            cjk_missing,
            scale_percent: choice.scale_percent(),
            theme,
        }
    }
}
