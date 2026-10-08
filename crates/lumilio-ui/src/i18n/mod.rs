//! The launcher's own words in the language the person chose. The catalogs
//! are Fluent files under `crates/lumilio-ui/i18n/<language>/`, compiled in;
//! `tr!` looks a message up and checks its id and arguments against the
//! Simplified Chinese catalog at compile time. Game logs, crash reports and
//! project text from content sources never go through here.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::sync::{LazyLock, PoisonError, RwLock};

use gpui::App;
use i18n_embed::fluent::FluentLanguageLoader;
use i18n_embed::unic_langid::LanguageIdentifier;
use i18n_embed::{DesktopLanguageRequester, LanguageLoader};
use lumilio_core::Language;
use rust_embed::RustEmbed;

#[doc(hidden)]
pub use i18n_embed_fl::fl as __fl;

#[cfg(test)]
mod tests;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Catalogs;

/// The catalog's file name inside each language folder.
const DOMAIN: &str = "lumilio-ui";

/// A language the launcher has a complete catalog for.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Locale {
    SimplifiedChinese = 0,
    English = 1,
}

impl Locale {
    /// The tag of the catalog folder, which gpui-component also uses.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::SimplifiedChinese => "zh-CN",
            Self::English => "en",
        }
    }

    fn identifier(self) -> LanguageIdentifier {
        self.tag()
            .parse()
            .unwrap_or_else(|_| unreachable!("the tags are valid"))
    }

    /// The locale a preference means on this system.
    #[must_use]
    pub fn of(language: Language) -> Self {
        match language {
            Language::SimplifiedChinese => Self::SimplifiedChinese,
            Language::English => Self::English,
            Language::System => Self::first_known(&DesktopLanguageRequester::requested_languages()),
        }
    }

    /// The first of the system's languages the launcher has; any other
    /// language falls back to Simplified Chinese.
    fn first_known(requested: &[LanguageIdentifier]) -> Self {
        requested
            .iter()
            .find_map(|language| match language.language.as_str() {
                "zh" => Some(Self::SimplifiedChinese),
                "en" => Some(Self::English),
                _ => None,
            })
            .unwrap_or(Self::SimplifiedChinese)
    }
}

static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| loader(Locale::SimplifiedChinese));

/// The locale in use; `UNSET` until the first `set_locale`, which reads as
/// Simplified Chinese but still has gpui-component to switch.
static CURRENT: AtomicU8 = AtomicU8::new(UNSET);
const UNSET: u8 = u8::MAX;

/// Counts language switches, so a view that keeps words in its state (a
/// placeholder, a dropdown's items) knows to take them again.
static GENERATION: AtomicU32 = AtomicU32::new(0);

/// Messages without arguments, formatted once per language and kept.
static TEXTS: LazyLock<RwLock<HashMap<(Locale, &'static str), &'static str>>> =
    LazyLock::new(RwLock::default);

/// Lists of such messages, kept the same way.
type Lists = HashMap<(Locale, &'static [&'static str]), &'static [&'static str]>;
static LISTS: LazyLock<RwLock<Lists>> = LazyLock::new(RwLock::default);

/// A loader showing `locale`, with Simplified Chinese behind any gap.
fn loader(locale: Locale) -> FluentLanguageLoader {
    let loader = FluentLanguageLoader::new(DOMAIN, Locale::SimplifiedChinese.identifier());
    load(&loader, locale);
    loader
}

fn load(loader: &FluentLanguageLoader, locale: Locale) {
    loader
        .load_languages(&Catalogs, &[locale.identifier()])
        .unwrap_or_else(|error| unreachable!("the catalogs are compiled in: {error}"));
    // Fluent wraps every argument in bidirectional isolation marks. The
    // launcher has no right-to-left language, and the marks would end up in
    // copied text.
    loader.set_use_isolating(false);
}

/// The loader `tr!` reads.
#[doc(hidden)]
pub fn current_loader() -> &'static FluentLanguageLoader {
    &LOADER
}

/// The language the launcher speaks now.
#[must_use]
pub fn locale() -> Locale {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Locale::English,
        _ => Locale::SimplifiedChinese,
    }
}

/// Speaks `locale` from now on, gpui-component's own words included.
pub fn set_locale(locale: Locale) {
    load(&LOADER, locale);
    CURRENT.store(locale as u8, Ordering::Relaxed);
    GENERATION.fetch_add(1, Ordering::Relaxed);
    gpui_component::set_locale(locale.tag());
}

/// Which language switch the words in use come from; it changes with every
/// [`set_locale`].
#[must_use]
pub fn generation() -> u32 {
    GENERATION.load(Ordering::Relaxed)
}

/// Switches to the language the preference means and redraws every window;
/// nothing happens when it is the language already in use.
pub fn apply_language(language: Language, cx: &mut App) {
    let next = Locale::of(language);
    if CURRENT.load(Ordering::Relaxed) != next as u8 {
        set_locale(next);
        cx.refresh_windows();
    }
}

/// A message without arguments, kept for the rest of the process so it can
/// go wherever a literal went. Bounded by the catalog size per language.
#[doc(hidden)]
pub fn text(id: &'static str) -> &'static str {
    let key = (locale(), id);
    if let Some(text) = TEXTS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
    {
        return text;
    }
    let text: &'static str = LOADER.get(id).leak();
    TEXTS
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(key)
        .or_insert(text)
}

/// A message whose id is only known at run time, such as `tag-<category>`
/// for a category a content source sends; `None` when the catalog has none,
/// so the caller can show the raw name. Unlike `tr!`, nothing checks the id
/// at compile time.
#[must_use]
pub fn lookup(id: &str) -> Option<String> {
    LOADER.has(id).then(|| LOADER.get(id))
}

/// Several messages without arguments, as one kept slice: choice labels go
/// where a `&'static [&'static str]` went.
#[doc(hidden)]
pub fn texts(ids: &'static [&'static str]) -> &'static [&'static str] {
    let key = (locale(), ids);
    if let Some(texts) = LISTS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
    {
        return texts;
    }
    let texts: &'static [&'static str] = ids.iter().map(|id| text(id)).collect::<Vec<_>>().leak();
    LISTS
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(key)
        .or_insert(texts)
}

/// The launcher's words for a message id in the current language. The id
/// and the argument names are checked against the Simplified Chinese
/// catalog at compile time.
///
/// Without arguments it is a `&'static str`; with `name = value` arguments
/// it is a `String`, so a count can choose its plural form in the catalog.
#[macro_export]
macro_rules! tr {
    ($id:literal) => {{
        let _checked = || $crate::i18n::__fl!($crate::i18n::current_loader(), $id);
        $crate::i18n::text($id)
    }};
    ($id:literal, $($name:ident = $value:expr),+ $(,)?) => {
        // `fl!` pastes each value before `.into()`; the parentheses keep
        // `a / b` from becoming `a / b.into()`.
        $crate::i18n::__fl!($crate::i18n::current_loader(), $id, $($name = ($value)),+)
    };
}

/// Several messages without arguments as a `&'static [&'static str]`, for
/// choice labels; each id is checked like `tr!`.
#[macro_export]
macro_rules! tr_all {
    ($($id:literal),+ $(,)?) => {{
        let _checked = || ($($crate::i18n::__fl!($crate::i18n::current_loader(), $id)),+);
        $crate::i18n::texts(&[$($id),+])
    }};
}
