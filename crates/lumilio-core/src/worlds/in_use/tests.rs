use super::worlds_in_use;

#[test]
fn one_game_has_open_only_the_world_it_touched_last() {
    let touched = [
        ("before", Some(50)),
        ("left", Some(120)),
        ("open", Some(180)),
        ("never", None),
    ];
    assert_eq!(worlds_in_use(touched, &[100]), ["open"]);
    // A lock touched at the very start counts as this game's.
    assert_eq!(worlds_in_use([("w", Some(100))], &[100]), ["w"]);
}

#[test]
fn no_game_or_no_world_touched_since_leaves_every_world_free() {
    let touched = [("a", Some(50)), ("b", None)];
    assert!(worlds_in_use(touched, &[]).is_empty());
    assert!(worlds_in_use(touched, &[100]).is_empty());
}

#[test]
fn with_several_games_every_world_touched_since_the_first_counts() {
    let touched = [
        ("before", Some(50)),
        ("first", Some(110)),
        ("second", Some(220)),
        ("third", Some(230)),
    ];
    let mut open = worlds_in_use(touched, &[200, 100]);
    open.sort_unstable();
    assert_eq!(open, ["first", "second", "third"]);
}
