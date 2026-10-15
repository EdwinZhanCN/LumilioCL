//! What the studio calls blocks, entities and block-state properties (plan
//! schematic-studio, T26/D28): the game's own names in the launcher's
//! language, and the launcher's words for properties. An ID is the label
//! only when no name is known; otherwise it is a mono caption.

use lumilio_core::BlockNames;

use crate::i18n::{self, Locale};
use crate::tr;

/// The game language matching the launcher's.
pub(crate) fn language() -> &'static str {
    match i18n::locale() {
        Locale::English => "en_us",
        Locale::SimplifiedChinese => "zh_cn",
    }
}

/// Block `id`'s name, else the ID itself.
pub(crate) fn block_name(names: &BlockNames, id: &str) -> String {
    names
        .block(id, language())
        .map_or_else(|| id.to_owned(), str::to_owned)
}

/// Entity `id`'s name, else the ID itself.
pub(crate) fn entity_name(names: &BlockNames, id: &str) -> String {
    names
        .entity(id, language())
        .map_or_else(|| id.to_owned(), str::to_owned)
}

/// The launcher's word for one block-state property, or `None` when it is
/// not worth showing (a boolean that is false). A value with words of its
/// own (`facing=east` → 朝东) uses them; a true boolean is the property's
/// word (含水); anything else is the property's word and the value
/// (信号强度 15).
pub(crate) fn property_word(key: &str, value: &str) -> Option<String> {
    if value == "false" {
        return None;
    }
    if value == "true" {
        return Some(key_word(key));
    }
    if let Some(word) = i18n::lookup(&format!("studio-prop-{key}-{value}")) {
        return Some(word);
    }
    Some(tr!("studio-prop-pair", key = key_word(key), value = value))
}

/// The word for a property in the inspect card, which lists every one (plan
/// schematic-studio T32): as [`property_word`], and a false boolean as its
/// word and 否 (含水 否).
pub(crate) fn property_line(key: &str, value: &str) -> String {
    property_word(key, value).unwrap_or_else(|| {
        tr!(
            "studio-prop-pair",
            key = key_word(key),
            value = tr!("studio-prop-no")
        )
    })
}

fn key_word(key: &str) -> String {
    i18n::lookup(&format!("studio-prop-{key}")).unwrap_or_else(|| key.to_owned())
}

/// The words for every property worth showing, in file order.
pub(crate) fn property_words(properties: &[(String, String)]) -> Vec<String> {
    properties
        .iter()
        .filter_map(|(key, value)| property_word(key, value))
        .collect()
}

/// `n` items as Minecraft stacks them: "1 组 + 22", "3 组", "22".
pub(crate) fn stacks(count: u64) -> String {
    let (full, rest) = (count / 64, count % 64);
    match (full, rest) {
        (0, rest) => rest.to_string(),
        (full, 0) => tr!("studio-stacks", count = full),
        (full, rest) => tr!("studio-stacks-rest", count = full, rest = rest),
    }
}
