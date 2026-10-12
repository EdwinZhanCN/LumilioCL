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
    #[cfg(test)]
    fn lumilio_cubiomes_decoration_raw(
        mc: i32,
        dim: i32,
        kind: i32,
        seed: u64,
        cx: i32,
        cz: i32,
        out: *mut i32,
        cap: i32,
    ) -> i32;
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
    fn lumilio_cubiomes_variant(mc: i32, seed: u64, kind: i32, x: i32, z: i32) -> i32;
    fn lumilio_cubiomes_decoration_available(mc: i32, dim: i32, kind: i32) -> i32;
    #[cfg(test)]
    fn lumilio_cubiomes_trig(mc: i32, cos: i32, value: f32) -> f32;
    #[cfg(test)]
    fn lumilio_cubiomes_ravine_check(
        mc: i32,
        seed: u64,
        sx: i32,
        sz: i32,
        count: *mut i64,
        checksum: *mut u64,
    ) -> i32;
    fn lumilio_cubiomes_ravines(
        mc: i32,
        seed: u64,
        x0: i32,
        z0: i32,
        x1: i32,
        z1: i32,
        out: *mut i32,
        cap: usize,
        used: *mut usize,
        cancelled: Option<unsafe extern "C" fn(*mut std::ffi::c_void) -> i32>,
        user: *mut std::ffi::c_void,
    ) -> i32;
    #[cfg(test)]
    fn lumilio_cubiomes_vein_noise(mc: i32, seed: u64, noise: i32, x: i32, y: i32, z: i32) -> f64;
    #[cfg(test)]
    fn lumilio_cubiomes_vein_column(
        mc: i32,
        seed: u64,
        x: i32,
        z: i32,
        y0: i32,
        y1: i32,
        out: *mut u8,
    ) -> i32;
    fn lumilio_cubiomes_vein_zones(
        mc: i32,
        seed: u64,
        vein: i32,
        x0: i32,
        z0: i32,
        cell: i32,
        w: i32,
        h: i32,
        out: *mut f32,
        cancelled: Option<unsafe extern "C" fn(*mut std::ffi::c_void) -> i32>,
        user: *mut std::ffi::c_void,
    ) -> i32;
    fn lumilio_cubiomes_decorations(
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

/// The raw candidate sequence of decoration `kind` (the fork's
/// `DecorationType`) in a chunk, or `None` when the release lacks it.
#[cfg(test)]
pub(super) fn decoration_raw(
    version: super::Version,
    dimension: super::Dimension,
    kind: i32,
    seed: i64,
    chunk: [i32; 2],
) -> Option<Vec<[i32; 3]>> {
    let mut out = [0; 256 * 3];
    // SAFETY: `out` holds the 256 triples the cap allows; the bridge rejects
    // larger caps and writes at most `cap` triples.
    let n = unsafe {
        lumilio_cubiomes_decoration_raw(
            version.mc,
            dimension as i32,
            kind,
            seed as u64,
            chunk[0],
            chunk[1],
            out.as_mut_ptr(),
            256,
        )
    };
    let n = usize::try_from(n).ok()?;
    Some(out[..n * 3].as_chunks::<3>().0.to_vec())
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

/// The release's `Mth.sin` (or `Mth.cos`) of a float.
#[cfg(test)]
pub(super) fn trig(version: super::Version, cos: bool, value: f32) -> f32 {
    // SAFETY: plain values in and out.
    unsafe { lumilio_cubiomes_trig(version.mc, i32::from(cos), value) }
}

/// A canyon start's carved blocks in every chunk it reaches: (count,
/// checksum), or `None` when the chunk starts none.
#[cfg(test)]
pub(super) fn ravine_check(
    version: super::Version,
    seed: i64,
    chunk: [i32; 2],
) -> Option<(i64, u64)> {
    let (mut count, mut checksum) = (0, 0);
    // SAFETY: two outputs that outlive the synchronous call.
    let found = unsafe {
        lumilio_cubiomes_ravine_check(
            version.mc,
            seed as u64,
            chunk[0],
            chunk[1],
            &mut count,
            &mut checksum,
        )
    };
    (found == 1).then_some((count, checksum))
}

/// Ravines whose start chunk lies in the block area, packed as the bridge
/// writes them; `cap` bounds the ints.
pub(super) fn ravines(
    version: super::Version,
    seed: i64,
    area: [i32; 4],
    cap: usize,
    cancelled: &mut dyn FnMut() -> bool,
) -> Result<Vec<i32>, super::Error> {
    let mut out = vec![0; cap];
    let mut used = 0;
    let mut hook: &mut dyn FnMut() -> bool = cancelled;
    // SAFETY: `out` holds `cap` ints and the bridge stops before exceeding
    // them (code 3); `used` and `hook` outlive the synchronous call.
    let result = unsafe {
        lumilio_cubiomes_ravines(
            version.mc,
            seed as u64,
            area[0],
            area[1],
            area[2],
            area[3],
            out.as_mut_ptr(),
            cap,
            &mut used,
            Some(poll),
            (&raw mut hook).cast(),
        )
    };
    match result {
        0 => {
            out.truncate(used);
            Ok(out)
        }
        2 => Err(super::Error::Cancelled),
        3 => Err(super::Error::InvalidRange),
        _ => Err(super::Error::Generation),
    }
}

/// One raw vein noise (the fork's `VN_*`) at a block.
#[cfg(test)]
pub(super) fn vein_noise(version: super::Version, seed: i64, noise: i32, at: [i32; 3]) -> f64 {
    // SAFETY: plain values in and out; the bridge bounds `noise`.
    unsafe { lumilio_cubiomes_vein_noise(version.mc, seed as u64, noise, at[0], at[1], at[2]) }
}

/// The vein rule's block codes (`VB_*`) for a column from `y0` to `y1`.
#[cfg(test)]
pub(super) fn vein_column(
    version: super::Version,
    seed: i64,
    column: [i32; 2],
    y0: i32,
    y1: i32,
) -> Vec<u8> {
    let mut out = vec![0_u8; (y1 - y0 + 1).max(0) as usize];
    // SAFETY: `out` holds one byte per height the bridge writes.
    unsafe {
        lumilio_cubiomes_vein_column(
            version.mc,
            seed as u64,
            column[0],
            column[1],
            y0,
            y1,
            out.as_mut_ptr(),
        )
    };
    out
}

/// Zone strengths of `vein` on a `w` × `h` grid of `cell`-block squares.
pub(super) fn vein_zones(
    version: super::Version,
    seed: i64,
    vein: i32,
    origin: [i32; 2],
    cell: i32,
    size: [i32; 2],
    cancelled: &mut dyn FnMut() -> bool,
) -> Result<Vec<f32>, super::Error> {
    let mut out = vec![0_f32; size[0] as usize * size[1] as usize];
    let mut hook: &mut dyn FnMut() -> bool = cancelled;
    // SAFETY: `out` holds the w × h cells the bridge writes; `hook` outlives
    // the synchronous call.
    let result = unsafe {
        lumilio_cubiomes_vein_zones(
            version.mc,
            seed as u64,
            vein,
            origin[0],
            origin[1],
            cell,
            size[0],
            size[1],
            out.as_mut_ptr(),
            Some(poll),
            (&raw mut hook).cast(),
        )
    };
    match result {
        0 => Ok(out),
        2 => Err(super::Error::Cancelled),
        _ => Err(super::Error::Generation),
    }
}

pub(super) fn decoration_available(mc: i32, dim: i32, kind: i32) -> bool {
    // SAFETY: plain integers in, a plain integer out.
    unsafe { lumilio_cubiomes_decoration_available(mc, dim, kind) != 0 }
}

/// Biome-filtered candidates of decoration `kind` in `[x0, x1) x [z0, z1)`,
/// as (x, y, z, flags). `cap` bounds the number of candidates.
pub(super) fn decorations(
    version: super::Version,
    seed: i64,
    dimension: super::Dimension,
    kind: i32,
    area: [i32; 4],
    cap: usize,
    cancelled: &mut dyn FnMut() -> bool,
) -> Result<Vec<[i32; 4]>, super::Error> {
    let mut out = vec![0; cap * 4];
    let mut count = 0;
    let mut hook: &mut dyn FnMut() -> bool = cancelled;
    // SAFETY: `out` has room for `cap` quads, which the bridge never exceeds
    // (it stops with code 3); `count` and `hook` outlive the synchronous call.
    let result = unsafe {
        lumilio_cubiomes_decorations(
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
        0 => Ok(out[..count * 4].as_chunks::<4>().0.to_vec()),
        2 => Err(super::Error::Cancelled),
        3 => Err(super::Error::InvalidRange),
        _ => Err(super::Error::Generation),
    }
}

/// The kind's start variant at a structure position, or -1 for none.
pub(super) fn variant(mc: i32, seed: i64, kind: i32, at: [i32; 2]) -> i32 {
    // SAFETY: plain integers in, a plain integer out; the bridge bounds `kind`
    // and owns the End City piece buffer it allocates.
    unsafe { lumilio_cubiomes_variant(mc, seed as u64, kind, at[0], at[1]) }
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
