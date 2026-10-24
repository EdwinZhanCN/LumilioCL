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

    /// Merges steps that lead to the same place, as forgetting entries can
    /// leave behind: neither direction then takes a step that changes nothing,
    /// counting `here`, where the person is now.
    pub fn collapse(&mut self, here: &T, same: impl Fn(&T, &T) -> bool) {
        self.back.dedup_by(|a, b| same(a, b));
        self.forward.dedup_by(|a, b| same(a, b));
        while self.back.last().is_some_and(|last| same(last, here)) {
            self.back.pop();
        }
        while self.forward.last().is_some_and(|next| same(next, here)) {
            self.forward.pop();
        }
    }
}

#[cfg(test)]
mod tests;
