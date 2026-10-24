use super::*;

#[test]
fn only_errors_stay_by_default() {
    assert!(!Toast::info("a").sticky);
    assert!(!Toast::success("a").sticky);
    assert!(Toast::error("a").sticky);
    assert!(Toast::info("a").sticky().sticky);
    assert_eq!(
        Toast::error("没装上").technical("io").technical.as_deref(),
        Some("io")
    );
}
