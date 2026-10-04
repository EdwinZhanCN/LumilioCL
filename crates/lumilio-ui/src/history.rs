//! Browser-style location history for the navigation bar (design language
//! §6). Window-free, so its rules are unit-tested on their own.

/// The way back and the way forward from the current location. The current
/// location itself lives with its owner and is handed in on every move.
pub struct History<T> {
    back: Vec<T>,
    forward: Vec<T>,
    limit: usize,
}

impl<T> History<T> {
    pub const fn new(limit: usize) -> Self {
        Self {
            back: Vec::new(),
            forward: Vec::new(),
            limit,
        }
    }

    /// Leaves `here` for a new location: `here` becomes the way back and the
    /// way forward is forgotten. The oldest entry goes past the limit.
    pub fn visit(&mut self, here: T) {
        self.back.push(here);
        if self.back.len() > self.limit {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    /// Where back leads, if anywhere; `here` becomes the way forward.
    pub fn back(&mut self, here: T) -> Option<T> {
        let to = self.back.pop()?;
        self.forward.push(here);
        Some(to)
    }

    /// Where forward leads, if anywhere; `here` becomes the way back.
    pub fn forward(&mut self, here: T) -> Option<T> {
        let to = self.forward.pop()?;
        self.back.push(here);
        Some(to)
    }

    pub fn can_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.back.iter().chain(&self.forward)
    }

    /// Forgets every entry `gone` matches, e.g. a deleted instance, so
    /// neither direction can lead to it.
    pub fn forget(&mut self, gone: impl Fn(&T) -> bool) {
        self.back.retain(|entry| !gone(entry));
        self.forward.retain(|entry| !gone(entry));
    }
}

#[cfg(test)]
mod tests {
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
}
