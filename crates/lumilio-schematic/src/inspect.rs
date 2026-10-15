//! What one block holds besides its state (plan schematic-studio T32): its
//! block entity's given name, the items in its slots and the text on it,
//! read for the studio's inspector, and the rest of its data as SNBT for
//! 技术详情. Item stacks are read in both shapes the game has saved (`Count`
//! before 1.20.5, `count` after), text as JSON strings (before 1.21.5) or as
//! NBT.

use nucleation::block_position::BlockPosition;
use nucleation::nbt::{NbtMap, NbtValue};

use crate::UniversalSchematic;

/// One stack in a block entity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    /// The slot, where the block entity numbers them.
    pub slot: Option<i32>,
    pub id: String,
    pub count: u32,
}

/// What a block entity holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contents {
    /// Its type, `minecraft:chest`.
    pub id: String,
    /// The name it was given (an anvil's rename).
    pub name: Option<String>,
    /// Its items in slot order; `None` for one that holds no items at all.
    pub items: Option<Vec<Stack>>,
    /// How its slots are laid out, where it has items.
    pub grid: Option<Grid>,
    /// Its words, line by line: a sign's front then its back, a command
    /// block's command. Empty lines are left out.
    pub text: Vec<String>,
    /// Every key it stores but its id and position, one a line in SNBT,
    /// sorted; empty when there is nothing else.
    pub data: String,
}

/// A block entity's slots as its screen in the game lays them out: a chest
/// is three rows of nine, a hopper one row of five.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    pub columns: u32,
    pub slots: u32,
}

/// Block entities whose slots are not rows of nine, by name: columns, slots.
/// The rest are rows of nine, as many as their highest slot needs.
const GRIDS: [(&str, u32, u32); 16] = [
    ("chest", 9, 27),
    ("trapped_chest", 9, 27),
    ("barrel", 9, 27),
    ("shulker_box", 9, 27),
    ("hopper", 5, 5),
    ("dispenser", 3, 9),
    ("dropper", 3, 9),
    ("crafter", 3, 9),
    ("furnace", 3, 3),
    ("blast_furnace", 3, 3),
    ("smoker", 3, 3),
    ("brewing_stand", 5, 5),
    ("campfire", 4, 4),
    ("chiseled_bookshelf", 3, 6),
    ("shelf", 3, 3),
    ("decorated_pot", 1, 1),
];

/// Keys that hold one item rather than a list: a jukebox's record, a
/// lectern's book, a decorated pot's or brushable block's item.
const SINGLE_ITEMS: [&str; 3] = ["RecordItem", "Book", "item"];

/// How the slots of block entity `id` holding `items` are laid out. A slot
/// past the table's (a modded container) adds rows rather than vanish.
#[must_use]
pub fn grid(id: &str, items: &[Stack]) -> Grid {
    let name = id.rsplit(':').next().unwrap_or(id).to_ascii_lowercase();
    let needed = items
        .iter()
        .map(|stack| stack.slot.map_or(1, |slot| slot.max(0) as u32 + 1))
        .max()
        .unwrap_or(0)
        .max(u32::try_from(items.len()).unwrap_or(u32::MAX));
    match GRIDS.iter().find(|(known, _, _)| *known == name) {
        Some(&(_, columns, slots)) => Grid {
            columns,
            slots: slots.max(needed.div_ceil(columns) * columns),
        },
        None if needed <= 1 && items.iter().all(|stack| stack.slot.is_none()) => Grid {
            columns: 1,
            slots: 1,
        },
        None => Grid {
            columns: 9,
            slots: needed.div_ceil(9).max(1) * 9,
        },
    }
}
/// Keys every block entity has, said elsewhere.
const PLACE_KEYS: [&str; 7] = ["id", "Id", "x", "y", "z", "Pos", "keepPacked"];

/// The block entity at `position`, or `None` where there is none.
pub fn block_entity(schematic: &UniversalSchematic, position: [i32; 3]) -> Option<Contents> {
    let [x, y, z] = position;
    let entity = schematic.get_block_entity(BlockPosition { x, y, z })?;
    let nbt = entity.nbt.as_ref();
    let items = items(nbt);
    Some(Contents {
        id: entity.id.clone(),
        name: nbt
            .get("CustomName")
            .map(text)
            .filter(|name| !name.is_empty()),
        grid: items.as_deref().map(|items| grid(&entity.id, items)),
        items,
        text: lines(nbt),
        data: data(nbt),
    })
}

fn items(nbt: &NbtMap) -> Option<Vec<Stack>> {
    let mut stacks: Vec<Stack> = match nbt.get("Items") {
        Some(NbtValue::List(list)) => list.iter().filter_map(stack).collect(),
        _ => Vec::new(),
    };
    let singles: Vec<Stack> = SINGLE_ITEMS
        .iter()
        .filter_map(|key| nbt.get(key))
        .filter_map(stack)
        .collect();
    if !nbt.inner().contains_key("Items") && singles.is_empty() {
        return None;
    }
    stacks.extend(singles);
    stacks.sort_by_key(|stack| stack.slot.unwrap_or(i32::MAX));
    Some(stacks)
}

fn stack(value: &NbtValue) -> Option<Stack> {
    let NbtValue::Compound(item) = value else {
        return None;
    };
    let id = item.get("id")?.as_string()?.clone();
    let count = item
        .get("count")
        .or_else(|| item.get("Count"))
        .and_then(number)
        .unwrap_or(1);
    Some(Stack {
        slot: item
            .get("Slot")
            .and_then(number)
            .and_then(|slot| i32::try_from(slot).ok()),
        id,
        count: u32::try_from(count).unwrap_or(0),
    })
}

fn number(value: &NbtValue) -> Option<i64> {
    match value {
        NbtValue::Byte(v) => Some(i64::from(*v)),
        NbtValue::Short(v) => Some(i64::from(*v)),
        NbtValue::Int(v) => Some(i64::from(*v)),
        NbtValue::Long(v) => Some(*v),
        _ => None,
    }
}

fn lines(nbt: &NbtMap) -> Vec<String> {
    let mut lines = Vec::new();
    for side in ["front_text", "back_text"] {
        if let Some(NbtValue::Compound(side)) = nbt.get(side)
            && let Some(NbtValue::List(messages)) = side.get("messages")
        {
            lines.extend(messages.iter().map(text));
        }
    }
    // Signs before 1.20.
    lines.extend(
        ["Text1", "Text2", "Text3", "Text4"]
            .iter()
            .filter_map(|key| nbt.get(key))
            .map(text),
    );
    if let Some(NbtValue::String(command)) = nbt.get("Command") {
        lines.push(command.clone());
    }
    lines.retain(|line| !line.trim().is_empty());
    lines
}

/// A text component's plain text: a JSON string as signs saved it before
/// 1.21.5, else NBT; a string that is not JSON is the text itself.
fn text(value: &NbtValue) -> String {
    match value {
        NbtValue::String(raw) => serde_json::from_str::<serde_json::Value>(raw)
            .map_or_else(|_| raw.clone(), |json| json_text(&json)),
        NbtValue::Compound(part) => {
            let own = part.get("text").map(text).unwrap_or_default();
            let rest = match part.get("extra") {
                Some(NbtValue::List(extra)) => extra.iter().map(text).collect(),
                _ => String::new(),
            };
            own + &rest
        }
        NbtValue::List(parts) => parts.iter().map(text).collect(),
        other => snbt(other),
    }
}

fn json_text(json: &serde_json::Value) -> String {
    use serde_json::Value;
    match json {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts.iter().map(json_text).collect(),
        Value::Object(part) => {
            let own = part.get("text").map(json_text).unwrap_or_default();
            let rest: String = match part.get("extra") {
                Some(Value::Array(extra)) => extra.iter().map(json_text).collect(),
                _ => String::new(),
            };
            own + &rest
        }
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn data(nbt: &NbtMap) -> String {
    let mut keys: Vec<&String> = nbt
        .inner()
        .keys()
        .filter(|key| !PLACE_KEYS.contains(&key.as_str()))
        .collect();
    keys.sort();
    keys.iter()
        .map(|key| format!("{}: {}", snbt_key(key), snbt(&nbt.inner()[*key])))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `value` in SNBT, compounds with their keys sorted.
fn snbt(value: &NbtValue) -> String {
    let list = |items: Vec<String>| items.join(", ");
    match value {
        NbtValue::Byte(v) => format!("{v}b"),
        NbtValue::Short(v) => format!("{v}s"),
        NbtValue::Int(v) => v.to_string(),
        NbtValue::Long(v) => format!("{v}L"),
        NbtValue::Float(v) => format!("{v}f"),
        NbtValue::Double(v) => format!("{v}d"),
        NbtValue::String(v) => quoted(v),
        NbtValue::ByteArray(v) => {
            format!("[B; {}]", list(v.iter().map(|b| format!("{b}b")).collect()))
        }
        NbtValue::IntArray(v) => format!("[I; {}]", list(v.iter().map(i32::to_string).collect())),
        NbtValue::LongArray(v) => {
            format!("[L; {}]", list(v.iter().map(|l| format!("{l}L")).collect()))
        }
        NbtValue::List(v) => format!("[{}]", list(v.iter().map(snbt).collect())),
        NbtValue::Compound(map) => {
            let mut keys: Vec<&String> = map.inner().keys().collect();
            keys.sort();
            let pairs = keys
                .iter()
                .map(|key| format!("{}: {}", snbt_key(key), snbt(&map.inner()[*key])))
                .collect();
            format!("{{{}}}", list(pairs))
        }
    }
}

fn snbt_key(key: &str) -> String {
    let bare = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+'));
    if bare { key.to_owned() } else { quoted(key) }
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}
