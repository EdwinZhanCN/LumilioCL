//! 替换方块… (plan schematic-studio, T13, T24): a table of rows, 从 → 到.
//! Each row's source is one of the file's blocks with a chip per property
//! (a value, or 任意; 不限属性 drops them all); its target is chosen from
//! the file's game version's blocks, recent ones first, and keeps every
//! property it shares with the block it replaces. The scope is all,
//! one region, the current slice or the current selection. What would
//! change is counted on the background executor whenever anything changes,
//! row by row, and the commit key says the total. A table can be kept as a
//! named 方案 in launcher data and applied to another schematic; both are
//! on the footer's leading side, as they act on the whole table.
//!
//! Every choice in the table is a dropdown: a row repeats, and segment keys
//! (their LED band, their width that follows the labels) would make each
//! row taller and the columns ragged. The scope's options include region
//! names, which are the file's data, so it is a dropdown too.

mod render;

mod schemes;

mod form;

mod replace_dialog;

use replace_dialog::{BLOCK_MENU, CHIP_WIDTH, Row, Within};
pub(crate) use replace_dialog::{BlockChoice, Opening, Preview, ReplaceDialog};
