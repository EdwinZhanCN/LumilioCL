//! A server's message of the day with the look the game gives it.
//!
//! A status answer carries a chat component (`description`); a legacy answer
//! carries plain text with `§` codes, and a component's text may hold those
//! codes too. Both follow the game's rules: a part inherits its parent's style
//! and overrides what it states; the first element of a JSON array is the
//! parent of the rest; a `§` colour code sets the colour and clears bold,
//! italic, underline, strikethrough and obfuscation; `§r` returns to the style
//! the text started with; an unknown code is skipped with its letter.
//! Everything is untrusted, so depth and length are capped.

use serde_json::Value;

/// Longest message kept, in characters; the game shows two short lines.
const MAX_CHARS: usize = 1024;
/// Deepest component nesting followed.
const MAX_DEPTH: usize = 32;

/// One run of a message with a single look.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MotdSpan {
    pub text: String,
    /// `0xRRGGBB`; `None` is the colour the place drawing it uses by default.
    pub color: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
    /// The game draws these characters as ever-changing random glyphs (`§k`).
    pub obfuscated: bool,
}

/// A message of the day: runs of text, each with its own look.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Motd {
    pub spans: Vec<MotdSpan>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Style {
    color: Option<u32>,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
    obfuscated: bool,
}

impl Motd {
    /// Text with `§` codes, as a legacy answer sends it.
    #[must_use]
    pub fn from_legacy(text: &str) -> Self {
        let mut motd = Self::default();
        motd.push_formatted(text, Style::default());
        motd
    }

    /// A chat component, as a status answer's `description` holds it.
    #[must_use]
    pub fn from_component(component: &Value) -> Self {
        let mut motd = Self::default();
        motd.walk(component, Style::default(), 0);
        motd
    }

    /// The words alone, without their look and the space around them.
    #[must_use]
    pub fn plain(&self) -> String {
        let text: String = self.spans.iter().map(|span| span.text.as_str()).collect();
        text.trim().to_owned()
    }

    /// The message split at its line breaks, each line its own runs. An empty
    /// line has no runs.
    #[must_use]
    pub fn lines(&self) -> Vec<Vec<MotdSpan>> {
        let mut lines = vec![Vec::new()];
        for span in &self.spans {
            for (index, piece) in span.text.split('\n').enumerate() {
                if index > 0 {
                    lines.push(Vec::new());
                }
                if !piece.is_empty()
                    && let Some(line) = lines.last_mut()
                {
                    line.push(MotdSpan {
                        text: piece.to_owned(),
                        ..span.clone()
                    });
                }
            }
        }
        lines
    }

    fn len(&self) -> usize {
        self.spans
            .iter()
            .map(|span| span.text.chars().count())
            .sum()
    }

    fn walk(&mut self, value: &Value, inherited: Style, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        match value {
            Value::String(text) => self.push_formatted(text, inherited),
            Value::Number(number) => self.push_formatted(&number.to_string(), inherited),
            Value::Bool(flag) => self.push_formatted(&flag.to_string(), inherited),
            // The first element is the parent; the rest are its children.
            Value::Array(parts) => {
                let Some((first, rest)) = parts.split_first() else {
                    return;
                };
                let style = own_style(first, inherited);
                self.walk(first, inherited, depth + 1);
                for part in rest {
                    self.walk(part, style, depth + 1);
                }
            }
            Value::Object(map) => {
                let style = own_style(value, inherited);
                match map.get("text") {
                    Some(Value::String(text)) => self.push_formatted(text, style),
                    Some(other @ (Value::Number(_) | Value::Bool(_))) => {
                        self.walk(other, style, depth + 1);
                    }
                    _ => {}
                }
                match map.get("extra") {
                    Some(Value::Array(parts)) => {
                        for part in parts {
                            self.walk(part, style, depth + 1);
                        }
                    }
                    Some(other) => self.walk(other, style, depth + 1),
                    None => {}
                }
            }
            Value::Null => {}
        }
    }

    /// `text` with its `§` codes applied, starting from `base`.
    fn push_formatted(&mut self, text: &str, base: Style) {
        let mut style = base;
        let mut run = String::new();
        let mut chars = text.chars();
        while let Some(ch) = chars.next() {
            if ch != '§' {
                run.push(ch);
                continue;
            }
            // A trailing `§` has no code: it ends the text.
            let Some(code) = chars.next() else { break };
            let next = match code.to_ascii_lowercase() {
                'r' => base,
                'k' => Style {
                    obfuscated: true,
                    ..style
                },
                'l' => Style {
                    bold: true,
                    ..style
                },
                'm' => Style {
                    strikethrough: true,
                    ..style
                },
                'n' => Style {
                    underlined: true,
                    ..style
                },
                'o' => Style {
                    italic: true,
                    ..style
                },
                other => match legacy_color(other) {
                    Some(color) => Style {
                        color: Some(color),
                        ..Style::default()
                    },
                    None => style,
                },
            };
            if next != style {
                self.push(std::mem::take(&mut run), style);
                style = next;
            }
        }
        self.push(run, style);
    }

    /// Adds a run, joined to the last one when they look the same, until the
    /// message is as long as it may be.
    fn push(&mut self, text: String, style: Style) {
        let room = MAX_CHARS.saturating_sub(self.len());
        if text.is_empty() || room == 0 {
            return;
        }
        let text: String = text.chars().take(room).collect();
        if let Some(last) = self.spans.last_mut()
            && style_of(last) == style
        {
            last.text.push_str(&text);
            return;
        }
        self.spans.push(MotdSpan {
            text,
            color: style.color,
            bold: style.bold,
            italic: style.italic,
            underlined: style.underlined,
            strikethrough: style.strikethrough,
            obfuscated: style.obfuscated,
        });
    }
}

const fn style_of(span: &MotdSpan) -> Style {
    Style {
        color: span.color,
        bold: span.bold,
        italic: span.italic,
        underlined: span.underlined,
        strikethrough: span.strikethrough,
        obfuscated: span.obfuscated,
    }
}

/// What a component states about its look, over what it inherits. Anything
/// that is not a component object states nothing.
fn own_style(component: &Value, inherited: Style) -> Style {
    let Value::Object(map) = component else {
        return inherited;
    };
    let flag = |key: &str, from: bool| map.get(key).and_then(Value::as_bool).unwrap_or(from);
    Style {
        color: map
            .get("color")
            .and_then(Value::as_str)
            .and_then(named_or_hex)
            .or(inherited.color),
        bold: flag("bold", inherited.bold),
        italic: flag("italic", inherited.italic),
        underlined: flag("underlined", inherited.underlined),
        strikethrough: flag("strikethrough", inherited.strikethrough),
        obfuscated: flag("obfuscated", inherited.obfuscated),
    }
}

/// The game's sixteen colours, by name and by `§` code.
const COLORS: [(char, &str, u32); 16] = [
    ('0', "black", 0x00_0000),
    ('1', "dark_blue", 0x00_00AA),
    ('2', "dark_green", 0x00_AA00),
    ('3', "dark_aqua", 0x00_AAAA),
    ('4', "dark_red", 0xAA_0000),
    ('5', "dark_purple", 0xAA_00AA),
    ('6', "gold", 0xFF_AA00),
    ('7', "gray", 0xAA_AAAA),
    ('8', "dark_gray", 0x55_5555),
    ('9', "blue", 0x55_55FF),
    ('a', "green", 0x55_FF55),
    ('b', "aqua", 0x55_FFFF),
    ('c', "red", 0xFF_5555),
    ('d', "light_purple", 0xFF_55FF),
    ('e', "yellow", 0xFF_FF55),
    ('f', "white", 0xFF_FFFF),
];

fn legacy_color(code: char) -> Option<u32> {
    COLORS
        .iter()
        .find(|(known, _, _)| *known == code)
        .map(|(_, _, rgb)| *rgb)
}

/// A component's `color`: one of the sixteen names or `#RRGGBB`.
fn named_or_hex(text: &str) -> Option<u32> {
    if let Some(hex) = text.strip_prefix('#') {
        return (hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(hex, 16).ok())
            .flatten();
    }
    COLORS
        .iter()
        .find(|(_, name, _)| *name == text)
        .map(|(_, _, rgb)| *rgb)
}

#[cfg(test)]
mod tests;
