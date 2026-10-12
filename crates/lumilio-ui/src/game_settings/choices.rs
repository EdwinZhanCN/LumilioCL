//! The keys and languages an option can be set to, with the words they read
//! as. Key tokens are the game's own (`key.keyboard.left.shift`); legends
//! printed on keyboards stay as they are in every language.
use crate::tr;

const UNBOUND: &str = "key.keyboard.unknown";

/// Every key and mouse button a binding can choose, in keyboard order.
#[must_use]
pub fn key_tokens() -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    let keyboard = |name: &str| format!("key.keyboard.{name}");
    keys.extend(('a'..='z').map(|c| keyboard(&c.to_string())));
    keys.extend(('0'..='9').map(|c| keyboard(&c.to_string())));
    keys.extend(
        [
            "space",
            "left.shift",
            "right.shift",
            "left.control",
            "right.control",
            "left.alt",
            "right.alt",
            "left.win",
            "right.win",
            "tab",
            "caps.lock",
            "enter",
            "backspace",
            "escape",
            "grave.accent",
            "minus",
            "equal",
            "left.bracket",
            "right.bracket",
            "backslash",
            "semicolon",
            "apostrophe",
            "comma",
            "period",
            "slash",
            "up",
            "down",
            "left",
            "right",
            "insert",
            "delete",
            "home",
            "end",
            "page.up",
            "page.down",
            "print.screen",
            "scroll.lock",
            "pause",
            "num.lock",
            "menu",
        ]
        .into_iter()
        .map(keyboard),
    );
    keys.extend((1..=12).map(|n| keyboard(&format!("f{n}"))));
    keys.extend((0..=9).map(|n| keyboard(&format!("keypad.{n}"))));
    keys.extend(
        [
            "keypad.add",
            "keypad.subtract",
            "keypad.multiply",
            "keypad.divide",
            "keypad.decimal",
            "keypad.enter",
            "keypad.equal",
        ]
        .into_iter()
        .map(keyboard),
    );
    keys.extend(
        ["left", "right", "middle", "4", "5", "6", "7", "8"]
            .into_iter()
            .map(|button| format!("key.mouse.{button}")),
    );
    keys.push(UNBOUND.into());
    keys
}

/// A key token as people read it: "W", "左 Shift", "鼠标右键".
#[must_use]
pub fn key_label(token: &str) -> String {
    if token == UNBOUND {
        return tr!("options-key-unbound").into();
    }
    if let Some(button) = token.strip_prefix("key.mouse.") {
        return match button {
            "left" => tr!("options-key-mouse-left").into(),
            "right" => tr!("options-key-mouse-right").into(),
            "middle" => tr!("options-key-mouse-middle").into(),
            other => tr!("options-key-mouse", button = other),
        };
    }
    let Some(name) = token.strip_prefix("key.keyboard.") else {
        return token.into();
    };
    match name {
        "left.bracket" => return "[".into(),
        "right.bracket" => return "]".into(),
        _ => {}
    }
    if let Some(side) = name.strip_prefix("left.") {
        return tr!("options-key-left", key = legend(side));
    }
    if let Some(side) = name.strip_prefix("right.") {
        return tr!("options-key-right", key = legend(side));
    }
    if let Some(pad) = name.strip_prefix("keypad.") {
        return tr!("options-key-keypad", key = legend(pad));
    }
    if name == "space" {
        return tr!("options-key-space").into();
    }
    legend(name)
}

/// The legend printed on a key.
fn legend(name: &str) -> String {
    let printed = match name {
        "shift" => "Shift",
        "control" => "Ctrl",
        "alt" => "Alt",
        "win" => "Win",
        "tab" => "Tab",
        "caps.lock" => "Caps Lock",
        "enter" => "Enter",
        "backspace" => "Backspace",
        "escape" => "Esc",
        "grave.accent" => "`",
        "minus" => "-",
        "equal" => "=",
        "backslash" => "\\",
        "semicolon" => ";",
        "apostrophe" => "'",
        "comma" => ",",
        "period" => ".",
        "slash" => "/",
        "up" => "↑",
        "down" => "↓",
        "insert" => "Insert",
        "delete" => "Delete",
        "home" => "Home",
        "end" => "End",
        "page.up" => "Page Up",
        "page.down" => "Page Down",
        "print.screen" => "Print Screen",
        "scroll.lock" => "Scroll Lock",
        "pause" => "Pause",
        "num.lock" => "Num Lock",
        "menu" => "Menu",
        "add" => "+",
        "subtract" => "-",
        "multiply" => "*",
        "divide" => "/",
        "decimal" => ".",
        _ => "",
    };
    match (name, printed) {
        // "left" and "right" alone are the arrow keys, not a side.
        ("left", _) => "←".into(),
        ("right", _) => "→".into(),
        (_, "") => name.to_uppercase(),
        (_, printed) => printed.into(),
    }
}

/// Languages the game ships, by code, with each language's own name.
const LANGUAGES: &[(&str, &str)] = &[
    ("af_za", "Afrikaans"),
    ("ar_sa", "العربية"),
    ("ast_es", "Asturianu"),
    ("az_az", "Azərbaycanca"),
    ("be_by", "Беларуская"),
    ("bg_bg", "Български"),
    ("br_fr", "Brezhoneg"),
    ("bs_ba", "Bosanski"),
    ("ca_es", "Català"),
    ("cs_cz", "Čeština"),
    ("cy_gb", "Cymraeg"),
    ("da_dk", "Dansk"),
    ("de_at", "Deutsch (Österreich)"),
    ("de_ch", "Deutsch (Schweiz)"),
    ("de_de", "Deutsch (Deutschland)"),
    ("el_gr", "Ελληνικά"),
    ("en_au", "English (Australia)"),
    ("en_ca", "English (Canada)"),
    ("en_gb", "English (UK)"),
    ("en_nz", "English (New Zealand)"),
    ("en_us", "English (US)"),
    ("eo_uy", "Esperanto"),
    ("es_ar", "Español (Argentina)"),
    ("es_cl", "Español (Chile)"),
    ("es_ec", "Español (Ecuador)"),
    ("es_es", "Español (España)"),
    ("es_mx", "Español (México)"),
    ("es_uy", "Español (Uruguay)"),
    ("es_ve", "Español (Venezuela)"),
    ("et_ee", "Eesti"),
    ("eu_es", "Euskara"),
    ("fa_ir", "فارسی"),
    ("fi_fi", "Suomi"),
    ("fil_ph", "Filipino"),
    ("fo_fo", "Føroyskt"),
    ("fr_ca", "Français (Canada)"),
    ("fr_fr", "Français (France)"),
    ("fy_nl", "Frysk"),
    ("ga_ie", "Gaeilge"),
    ("gd_gb", "Gàidhlig"),
    ("gl_es", "Galego"),
    ("he_il", "עברית"),
    ("hi_in", "हिंदी"),
    ("hr_hr", "Hrvatski"),
    ("hu_hu", "Magyar"),
    ("hy_am", "Հայերեն"),
    ("id_id", "Bahasa Indonesia"),
    ("is_is", "Íslenska"),
    ("it_it", "Italiano"),
    ("ja_jp", "日本語"), // i18n-exempt: the language's own name
    ("ka_ge", "ქართული"),
    ("kk_kz", "Қазақша"),
    ("kn_in", "ಕನ್ನಡ"),
    ("ko_kr", "한국어"),
    ("la_la", "Latina"),
    ("lb_lu", "Lëtzebuergesch"),
    ("lt_lt", "Lietuvių"),
    ("lv_lv", "Latviešu"),
    ("lzh", "文言"), // i18n-exempt: the language's own name
    ("mk_mk", "Македонски"),
    ("mn_mn", "Монгол"),
    ("ms_my", "Bahasa Melayu"),
    ("mt_mt", "Malti"),
    ("nl_be", "Vlaams"),
    ("nl_nl", "Nederlands"),
    ("nn_no", "Norsk nynorsk"),
    ("oc_fr", "Occitan"),
    ("pl_pl", "Polski"),
    ("pt_br", "Português (Brasil)"),
    ("pt_pt", "Português (Portugal)"),
    ("ro_ro", "Română"),
    ("ru_ru", "Русский"),
    ("sk_sk", "Slovenčina"),
    ("sl_si", "Slovenščina"),
    ("sq_al", "Shqip"),
    ("sr_sp", "Српски"),
    ("sv_se", "Svenska"),
    ("ta_in", "தமிழ்"),
    ("th_th", "ไทย"),
    ("tr_tr", "Türkçe"),
    ("uk_ua", "Українська"),
    ("vi_vn", "Tiếng Việt"),
    ("zh_cn", "简体中文（中国大陆）"), // i18n-exempt: the language's own name
    ("zh_hk", "繁體中文（香港特別行政區）"), // i18n-exempt: the language's own name
    ("zh_tw", "繁體中文（台灣）"),     // i18n-exempt: the language's own name
];

/// Every language code the editor offers; a code the game file holds that
/// is not listed stays choosable as it is.
#[must_use]
pub fn language_codes(current: Option<&str>) -> Vec<String> {
    let mut codes: Vec<String> = LANGUAGES.iter().map(|(code, _)| (*code).into()).collect();
    if let Some(current) = current
        && !codes.iter().any(|code| code == current)
    {
        codes.insert(0, current.into());
    }
    codes
}

#[must_use]
pub fn language_label(code: &str) -> String {
    LANGUAGES
        .iter()
        .find(|(known, _)| *known == code)
        .map_or_else(|| code.to_owned(), |(_, name)| (*name).to_owned())
}
