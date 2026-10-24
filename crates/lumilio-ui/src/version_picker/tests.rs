use super::*;

fn choice(id: &str, primary: bool) -> Choice {
    Choice {
        id: id.to_owned(),
        primary,
        tag: None,
    }
}

#[test]
fn only_primary_versions_show_until_asked_but_a_search_finds_any() {
    let all = [
        choice("26.4-snapshot-1", false),
        choice("26.3", true),
        choice("26.3-rc-1", false),
        choice("26.2", true),
    ];
    let ids = |shown: Vec<&Choice>| shown.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(visible(&all, "", false)), ["26.3", "26.2"]);
    assert_eq!(visible(&all, "", true).len(), 4);
    assert_eq!(ids(visible(&all, "rc", false)), ["26.3-rc-1"]);
    assert_eq!(ids(visible(&all, " 26.3 ", false)), ["26.3", "26.3-rc-1"]);
    assert!(visible(&all, "1.12", true).is_empty());
}
