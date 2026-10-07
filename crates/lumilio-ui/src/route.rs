//! The global routes: the pages the navigation bar reaches directly.

/// The destinations that are always available in the global shell.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Route {
    Home,
    Library,
    Discover,
    Activity,
    Accounts,
    Settings,
}

impl Route {
    /// Stable ordering for the compact navigation capsule.
    pub const ORDER: [Self; 6] = [
        Self::Home,
        Self::Library,
        Self::Discover,
        Self::Activity,
        Self::Accounts,
        Self::Settings,
    ];

    /// Returns the short label shown in expanded contexts and tooltips.
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => crate::tr!("route-home"),
            Self::Library => crate::tr!("route-library"),
            Self::Discover => crate::tr!("route-discover"),
            Self::Activity => crate::tr!("route-activity"),
            Self::Accounts => crate::tr!("route-accounts"),
            Self::Settings => crate::tr!("route-settings"),
        }
    }

    /// Returns the stable keyboard hint associated with the route.
    pub const fn shortcut(self) -> &'static str {
        #[cfg(target_os = "macos")]
        match self {
            Self::Home => "⌘1",
            Self::Library => "⌘2",
            Self::Discover => "⌘3",
            Self::Activity => "⌘4",
            Self::Accounts => "⌘5",
            Self::Settings => "⌘,",
        }

        #[cfg(not(target_os = "macos"))]
        match self {
            Self::Home => "Ctrl+1",
            Self::Library => "Ctrl+2",
            Self::Discover => "Ctrl+3",
            Self::Activity => "Ctrl+4",
            Self::Accounts => "Ctrl+5",
            Self::Settings => "Ctrl+,",
        }
    }

    /// Converts a compact navigation index to a route.
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Home),
            1 => Some(Self::Library),
            2 => Some(Self::Discover),
            3 => Some(Self::Activity),
            4 => Some(Self::Accounts),
            5 => Some(Self::Settings),
            _ => None,
        }
    }

    /// Resolves the context-aware global shortcut without changing shell state.
    pub fn from_shortcut(key: &str, secondary_modifier: bool) -> Option<Self> {
        if !secondary_modifier {
            return None;
        }

        match key {
            "1" => Some(Self::Home),
            "2" => Some(Self::Library),
            "3" => Some(Self::Discover),
            "4" => Some(Self::Activity),
            "5" => Some(Self::Accounts),
            "," => Some(Self::Settings),
            _ => None,
        }
    }

    /// Returns the stable zero-based order index.
    pub const fn index(self) -> usize {
        match self {
            Self::Home => 0,
            Self::Library => 1,
            Self::Discover => 2,
            Self::Activity => 3,
            Self::Accounts => 4,
            Self::Settings => 5,
        }
    }
}

#[cfg(test)]
mod tests {
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
}
