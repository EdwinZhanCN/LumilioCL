use super::History;

#[test]
fn back_and_forward_walk_the_visited_locations_like_a_browser() {
    let mut history = History::new(10);
    assert!(!history.can_back() && !history.can_forward());
    history.visit("home");
    history.visit("library");
    // Now at "instance".
    assert_eq!(history.back("instance"), Some("library"));
    assert_eq!(history.back("library"), Some("home"));
    assert_eq!(history.back("home"), None, "nothing before the first");
    assert!(history.can_forward());
    assert_eq!(history.forward("home"), Some("library"));
    assert_eq!(history.forward("library"), Some("instance"));
    assert_eq!(history.forward("instance"), None);
}

#[test]
fn a_new_visit_forgets_the_way_forward() {
    let mut history = History::new(10);
    history.visit("home");
    assert_eq!(history.back("library"), Some("home"));
    assert!(history.can_forward());
    history.visit("home");
    assert!(!history.can_forward());
    assert_eq!(history.back("discover"), Some("home"));
}

#[test]
fn history_is_bounded_and_drops_the_oldest() {
    let mut history = History::new(2);
    for at in ["a", "b", "c"] {
        history.visit(at);
    }
    assert_eq!(history.back("d"), Some("c"));
    assert_eq!(history.back("c"), Some("b"));
    assert_eq!(history.back("b"), None, "\"a\" was dropped");
}

#[test]
fn forgotten_locations_are_unreachable_in_both_directions() {
    let mut history = History::new(10);
    history.visit("library");
    history.visit("gone");
    // At "discover"; walk back to "library" so "gone" is on both sides.
    assert_eq!(history.back("discover"), Some("gone"));
    assert_eq!(history.back("gone"), Some("library"));
    history.forget(|entry| *entry == "gone");
    assert!(!history.can_back());
    assert_eq!(history.forward("library"), Some("discover"));
    assert_eq!(history.forward("discover"), None, "\"gone\" was skipped");
    assert_eq!(history.back("discover"), Some("library"));
}

#[test]
fn collapsing_leaves_no_step_that_goes_nowhere() {
    let mut history = History::new(10);
    for at in ["home", "library", "game", "library", "game"] {
        history.visit(at);
    }
    // At "discover". Without the games, back would be library twice.
    history.forget(|entry| *entry == "game");
    history.collapse(&"discover", |a, b| a == b);
    assert_eq!(history.back("discover"), Some("library"));
    assert_eq!(
        history.back("library"),
        Some("home"),
        "one library, not two"
    );
    assert_eq!(history.back("home"), None);

    let mut history = History::new(10);
    for at in ["library", "game", "library"] {
        history.visit(at);
    }
    // At "discover"; walk back to the first "library".
    assert_eq!(history.back("discover"), Some("library"));
    assert_eq!(history.back("library"), Some("game"));
    assert_eq!(history.back("game"), Some("library"));
    history.forget(|entry| *entry == "game");
    history.collapse(&"library", |a, b| a == b);
    assert_eq!(
        history.forward("library"),
        Some("discover"),
        "no step from library to library"
    );
}
