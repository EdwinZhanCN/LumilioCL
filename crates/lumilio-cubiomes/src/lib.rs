//! Safe boundary around the pinned cubiomes source snapshot.
mod ffi;

/// A build/link probe, before the generation API is added.
pub fn probe() -> i32 {
    ffi::probe()
}

#[cfg(test)]
mod tests;
