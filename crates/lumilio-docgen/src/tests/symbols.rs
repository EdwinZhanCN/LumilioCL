//! The function a generated row names beside its file.

use super::*;

use crate::symbols::Functions;

const SOURCE: &str = "\
pub fn page() -> u8 {
    // ia[library]: in a function | a | b
    let nested = {
        fn helper() -> u8 {
            // ia[library]: in a nested function | a | b
            1
        }
        helper()
    };
    nested
}

// ia[library]: above a function | a | b
// An ordinary comment.
/// Its documentation.
#[inline]
fn above() {}

struct Card;

impl Card {
    fn menu(&self) {
        // ia[library]: in a method | a | b
    }
}

trait Shelf {
    fn row(&self) {
        // ia[library]: in a trait's method | a | b
    }
}

// ia[library]: at module level | a | b
const LEVEL: u8 = 0;
";

fn symbol(source: &str, action: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let line = lines
        .iter()
        .position(|line| line.contains(action))
        .expect("the source has the action")
        + 1;
    Functions::parse(source).at(line, &lines).map(str::to_owned)
}

#[test]
fn a_row_names_the_function_it_is_in_or_right_above() {
    assert_eq!(symbol(SOURCE, "in a function").as_deref(), Some("page"));
    assert_eq!(
        symbol(SOURCE, "in a nested function").as_deref(),
        Some("helper"),
        "the innermost function"
    );
    assert_eq!(
        symbol(SOURCE, "above a function").as_deref(),
        Some("above"),
        "comments, documentation and attributes may sit between"
    );
    assert_eq!(symbol(SOURCE, "in a method").as_deref(), Some("Card::menu"));
    assert_eq!(
        symbol(SOURCE, "in a trait's method").as_deref(),
        Some("Shelf::row")
    );
    assert_eq!(symbol(SOURCE, "at module level"), None);
}

#[test]
fn a_file_that_does_not_parse_names_only_the_file() {
    let source = "fn broken( {\n    // ia[library]: a | b | c\n}\n";
    assert_eq!(symbol(source, "ia[library]"), None);
}

#[test]
fn the_implementation_column_names_the_file_and_the_function() {
    let mut row = path("library", "搜索");
    let page = &PAGES[0];
    assert!(render(page, &[&row]).contains("| `lumilio-ui/src/a.rs` |"));
    row.symbol = Some("card_menu".into());
    assert!(render(page, &[&row]).contains("| `lumilio-ui/src/a.rs` › `card_menu` |"));
}
