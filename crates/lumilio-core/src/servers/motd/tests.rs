use super::*;
use serde_json::json;

fn span(text: &str, color: Option<u32>) -> MotdSpan {
    MotdSpan {
        text: text.to_owned(),
        color,
        ..MotdSpan::default()
    }
}

#[test]
fn legacy_codes_colour_the_text_and_a_colour_clears_the_formats() {
    let motd = Motd::from_legacy("§lBold §aGreen§r plain");
    assert_eq!(
        motd.spans,
        vec![
            MotdSpan {
                bold: true,
                ..span("Bold ", None)
            },
            span("Green", Some(0x55_FF55)),
            span(" plain", None),
        ]
    );
}

#[test]
fn legacy_codes_ignore_case_and_skip_unknown_codes_with_their_letter() {
    let motd = Motd::from_legacy("§Cred§zstill§");
    assert_eq!(motd.spans, vec![span("redstill", Some(0xFF_5555))]);
}

#[test]
fn a_component_inherits_its_parents_look_and_overrides_what_it_states() {
    let motd = Motd::from_component(&json!({
        "text": "A",
        "color": "gold",
        "bold": true,
        "extra": [
            {"text": "B", "bold": false},
            {"text": "C", "color": "#123456"},
            "D"
        ]
    }));
    let gold = Some(0xFF_AA00);
    assert_eq!(
        motd.spans,
        vec![
            MotdSpan {
                bold: true,
                ..span("A", gold)
            },
            span("B", gold),
            MotdSpan {
                bold: true,
                ..span("C", Some(0x12_3456))
            },
            MotdSpan {
                bold: true,
                ..span("D", gold)
            },
        ]
    );
}

#[test]
fn the_first_element_of_an_array_is_the_parent_of_the_rest() {
    let motd = Motd::from_component(
        &json!([{"text": "x", "italic": true}, "y", {"text": "z", "italic": false}]),
    );
    assert_eq!(
        motd.spans,
        vec![
            MotdSpan {
                italic: true,
                ..span("xy", None)
            },
            span("z", None),
        ]
    );
}

#[test]
fn section_codes_inside_a_component_start_from_its_look() {
    let motd = Motd::from_component(&json!({"text": "§eHi§r there", "color": "aqua"}));
    assert_eq!(
        motd.spans,
        vec![span("Hi", Some(0xFF_FF55)), span(" there", Some(0x55_FFFF))]
    );
}

#[test]
fn an_unknown_colour_is_inherited_instead() {
    let motd = Motd::from_component(&json!({
        "text": "",
        "color": "red",
        "extra": [{"text": "a", "color": "#12345"}, {"text": "b", "color": "mauve"}]
    }));
    assert_eq!(motd.spans, vec![span("ab", Some(0xFF_5555))]);
}

#[test]
fn lines_split_runs_at_line_breaks_and_plain_keeps_the_words() {
    let motd = Motd::from_legacy("  §aTop\n§bBottom  ");
    let lines = motd.lines();
    assert_eq!(
        lines,
        vec![
            vec![span("  ", None), span("Top", Some(0x55_FF55))],
            vec![span("Bottom  ", Some(0x55_FFFF))],
        ]
    );
    assert_eq!(motd.plain(), "Top\nBottom");
    assert_eq!(Motd::from_legacy("a\n\nb").lines()[1], Vec::new());
}

#[test]
fn a_hostile_message_is_capped_in_length_and_depth() {
    let long = "x".repeat(MAX_CHARS * 2);
    assert_eq!(Motd::from_legacy(&long).plain().len(), MAX_CHARS);
    let mut deep = json!("bottom");
    for _ in 0..(MAX_DEPTH + 8) {
        deep = json!({"text": "", "extra": [deep]});
    }
    assert_eq!(Motd::from_component(&deep).plain(), "");
}
