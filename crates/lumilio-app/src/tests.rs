use super::review_size;

#[test]
fn a_review_size_is_read_as_logical_width_by_height() {
    assert_eq!(review_size("1600x1000"), Some((1600., 1000.)));
    assert_eq!(review_size(" 1440 X 900 "), Some((1440., 900.)));
}

#[test]
fn a_review_size_below_the_minimum_or_unreadable_is_ignored() {
    for value in ["640x480", "1600", "wide x tall", "1600x", "infxinf", ""] {
        assert_eq!(review_size(value), None, "{value}");
    }
}
