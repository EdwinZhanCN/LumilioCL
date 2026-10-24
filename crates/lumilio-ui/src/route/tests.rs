use super::Route;

#[test]
fn navigation_order_and_labels_are_stable() {
    assert_eq!(
        Route::ORDER,
        [
            Route::Home,
            Route::Library,
            Route::Discover,
            Route::Activity,
            Route::Accounts,
            Route::Settings
        ]
    );
    assert_eq!(
        Route::ORDER.map(Route::label),
        ["首页", "游戏库", "发现", "动态", "账户", "设置"]
    );
}

#[test]
fn index_round_trip_reaches_only_arch_routes() {
    for (index, route) in Route::ORDER.into_iter().enumerate() {
        assert_eq!(Route::from_index(index), Some(route));
        assert_eq!(route.index(), index);
    }
    assert_eq!(Route::from_index(6), None);
}

#[test]
fn shortcuts_require_the_secondary_modifier() {
    assert_eq!(Route::from_shortcut("1", true), Some(Route::Home));
    assert_eq!(Route::from_shortcut("4", true), Some(Route::Activity));
    assert_eq!(Route::from_shortcut("5", true), Some(Route::Accounts));
    assert_eq!(Route::from_shortcut(",", true), Some(Route::Settings));
    assert_eq!(Route::from_shortcut("2", false), None);
    assert_eq!(Route::from_shortcut("x", true), None);
}
