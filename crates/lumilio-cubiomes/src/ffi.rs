#![allow(unsafe_code)]

unsafe extern "C" {
    #[cfg(test)]
    fn lumilio_cubiomes_sample(mc: i32, seed: u64, dim: i32, x: i32, y: i32, z: i32) -> i32;
    #[cfg(test)]
    fn lumilio_cubiomes_placement(
        mc: i32,
        seed: u64,
        kind: i32,
        rx: i32,
        rz: i32,
        out: *mut i32,
    ) -> i32;
    #[cfg(test)]
    fn lumilio_cubiomes_biome_rule(mc: i32, rule: i32, biome: i32) -> i32;
    fn lumilio_cubiomes_probe() -> i32;
    fn lumilio_cubiomes_generate(
        mc: i32,
        seed: u64,
        dim: i32,
        scale: i32,
        x: i32,
        z: i32,
        width: i32,
        height: i32,
        out: *mut i32,
        count: usize,
        cancelled: Option<unsafe extern "C" fn(*mut std::ffi::c_void) -> i32>,
        user: *mut std::ffi::c_void,
    ) -> i32;
    fn lumilio_cubiomes_colors(out: *mut u8);
    fn lumilio_cubiomes_structure_available(mc: i32, dim: i32, kind: i32) -> i32;
    fn lumilio_cubiomes_region_blocks(mc: i32, dim: i32, kind: i32) -> i32;
    fn lumilio_cubiomes_structures(
        mc: i32,
        seed: u64,
        dim: i32,
        kind: i32,
        x0: i32,
        z0: i32,
        x1: i32,
        z1: i32,
        out: *mut i32,
        cap: usize,
        count: *mut usize,
        cancelled: Option<unsafe extern "C" fn(*mut std::ffi::c_void) -> i32>,
        user: *mut std::ffi::c_void,
    ) -> i32;
    fn lumilio_cubiomes_strongholds(
        mc: i32,
        seed: u64,
        out: *mut i32,
        cap: usize,
        count: *mut usize,
        cancelled: Option<unsafe extern "C" fn(*mut std::ffi::c_void) -> i32>,
        user: *mut std::ffi::c_void,
    ) -> i32;
    fn lumilio_cubiomes_spawn(mc: i32, seed: u64, dim: i32, out: *mut i32) -> i32;
    fn lumilio_cubiomes_slime(seed: u64, cx: i32, cz: i32, width: i32, height: i32, out: *mut u8);
}

#[cfg(test)]
pub(super) fn sample(
    version: super::Version,
    seed: i64,
    dimension: super::Dimension,
    at: [i32; 3],
) -> i32 {
    // SAFETY: version/dimension are closed Rust identities. The C function
    // also bounds all coordinates before creating or sampling a generator.
    unsafe {
        lumilio_cubiomes_sample(
            version.mc,
            seed as u64,
            dimension as i32,
            at[0],
            at[1],
            at[2],
        )
    }
}

#[cfg(test)]
pub(super) fn placement(
    version: super::Version,
    seed: i64,
    kind: super::Structure,
    region: [i32; 2],
) -> Result<[i32; 2], super::Error> {
    let mut out = [0; 2];
    // SAFETY: fixed two-element output; closed kind/version and bounded region.
    let result = unsafe {
        lumilio_cubiomes_placement(
            version.mc,
            seed as u64,
            kind as i32,
            region[0],
            region[1],
            out.as_mut_ptr(),
        )
    };
    if result == 0 {
        Ok(out)
    } else {
        Err(super::Error::Generation)
    }
}

#[cfg(test)]
pub(super) fn biome_rule(version: super::Version, rule: i32, biome: i32) -> bool {
    // SAFETY: bridge validates the rule before indexing, no pointers cross FFI.
    unsafe { lumilio_cubiomes_biome_rule(version.mc, rule, biome) != 0 }
}

/// Trampoline for the bridge's cancellation hook.
///
/// # Safety
/// `user` must point to a live `&mut dyn FnMut() -> bool` for the whole call.
unsafe extern "C" fn poll(user: *mut std::ffi::c_void) -> i32 {
    // SAFETY: `generate` passes a pointer to its own `&mut dyn FnMut` and the
    // bridge only calls this synchronously while `generate` is on the stack.
    let cancelled = unsafe { &mut *user.cast::<&mut dyn FnMut() -> bool>() };
    i32::from(cancelled())
}

pub(super) fn generate(
    version: super::Version,
    seed: i64,
    dimension: super::Dimension,
    range: super::Range,
    cancelled: &mut dyn FnMut() -> bool,
) -> Result<Vec<i32>, super::Error> {
    let mut out = vec![0; range.width as usize * range.height as usize];
    let mut hook: &mut dyn FnMut() -> bool = cancelled;
    // SAFETY: only validated bounded ranges reach this private function. The
    // bridge allocates cubiomes' scratch cache and copies exactly out.len() ints;
    // `hook` outlives the call and is only used from this thread.
    let result = unsafe {
        lumilio_cubiomes_generate(
            version.mc,
            seed as u64,
            dimension as i32,
            range.scale,
            range.x,
            range.z,
            range.width,
            range.height,
            out.as_mut_ptr(),
            out.len(),
            Some(poll),
            (&raw mut hook).cast(),
        )
    };
    if result == 0 {
        Ok(out)
    } else if result == 2 {
        Err(super::Error::Cancelled)
    } else {
        Err(super::Error::Generation)
    }
}

pub(super) fn colors() -> [[u8; 3]; 256] {
    let mut out = [[0; 3]; 256];
    // SAFETY: out contains exactly 256 contiguous RGB triplets, as required
    // by initBiomeColors; the bridge neither retains nor reads the pointer.
    unsafe { lumilio_cubiomes_colors(out.as_mut_ptr().cast()) };
    out
}

pub(super) fn probe() -> i32 {
    // SAFETY: the C probe owns its generator and accepts no pointers or input.
    unsafe { lumilio_cubiomes_probe() }
}

pub(super) fn structure_available(mc: i32, dim: i32, kind: i32) -> bool {
    // SAFETY: plain integers in, a plain integer out.
    unsafe { lumilio_cubiomes_structure_available(mc, dim, kind) != 0 }
}

/// The region grid's block edge for a kind, 0 when unavailable. Sizes the
/// output buffer the query needs.
pub(super) fn region_blocks(mc: i32, dim: i32, kind: i32) -> i32 {
    // SAFETY: plain integers in, a plain integer out.
    unsafe { lumilio_cubiomes_region_blocks(mc, dim, kind) }
}

/// Viable positions of one structure kind in `[x0, x1) x [z0, z1)`.
/// `cap` must be at least the number of regions the area touches.
pub(super) fn structures(
    version: super::Version,
    seed: i64,
    dimension: super::Dimension,
    kind: i32,
    area: [i32; 4],
    cap: usize,
    cancelled: &mut dyn FnMut() -> bool,
) -> Result<Vec<[i32; 2]>, super::Error> {
    let mut out = vec![0; cap * 2];
    let mut count = 0;
    let mut hook: &mut dyn FnMut() -> bool = cancelled;
    // SAFETY: `out` has room for `cap` pairs, which the bridge never exceeds
    // (it stops with code 3); `count` and `hook` outlive the synchronous call.
    let result = unsafe {
        lumilio_cubiomes_structures(
            version.mc,
            seed as u64,
            dimension as i32,
            kind,
            area[0],
            area[1],
            area[2],
            area[3],
            out.as_mut_ptr(),
            cap,
            &mut count,
            Some(poll),
            (&raw mut hook).cast(),
        )
    };
    match result {
        0 => Ok(out[..count * 2].as_chunks::<2>().0.to_vec()),
        2 => Err(super::Error::Cancelled),
        _ => Err(super::Error::Generation),
    }
}

pub(super) fn strongholds(
    version: super::Version,
    seed: i64,
    limit: usize,
    cancelled: &mut dyn FnMut() -> bool,
) -> Result<Vec<[i32; 2]>, super::Error> {
    let mut out = vec![0; limit * 2];
    let mut count = 0;
    let mut hook: &mut dyn FnMut() -> bool = cancelled;
    // SAFETY: `out` has room for `limit` pairs and the bridge stops at `limit`;
    // `count` and `hook` outlive the synchronous call.
    let result = unsafe {
        lumilio_cubiomes_strongholds(
            version.mc,
            seed as u64,
            out.as_mut_ptr(),
            limit,
            &mut count,
            Some(poll),
            (&raw mut hook).cast(),
        )
    };
    match result {
        0 => Ok(out[..count * 2].as_chunks::<2>().0.to_vec()),
        2 => Err(super::Error::Cancelled),
        _ => Err(super::Error::Generation),
    }
}

pub(super) fn spawn(version: super::Version, seed: i64) -> [i32; 2] {
    let mut out = [0; 2];
    // SAFETY: `out` is the two ints the bridge writes.
    unsafe {
        lumilio_cubiomes_spawn(
            version.mc,
            seed as u64,
            super::Dimension::Overworld as i32,
            out.as_mut_ptr(),
        )
    };
    out
}

pub(super) fn slime(seed: i64, cx: i32, cz: i32, width: i32, height: i32) -> Vec<bool> {
    let mut out = vec![0_u8; width as usize * height as usize];
    // SAFETY: only validated sizes reach here; `out` has width * height bytes.
    unsafe { lumilio_cubiomes_slime(seed as u64, cx, cz, width, height, out.as_mut_ptr()) };
    out.into_iter().map(|value| value != 0).collect()
}
