//! Xaero's Minimap waypoints: file naming, a line-preserving file model, share
//! strings, and the overlay that draws them. The format has no public
//! specification; what is known comes from XaeroTools and is checked against
//! the pinned sample in `tests/data/xaero/` (plan W17).
pub(crate) mod edit;
pub(crate) mod naming;
pub(crate) mod overlay;
pub(crate) mod waypoints;
pub(crate) mod world_map;

#[cfg(test)]
mod tests;
