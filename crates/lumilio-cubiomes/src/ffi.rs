#![allow(unsafe_code)]

unsafe extern "C" {
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
    ) -> i32;
    fn lumilio_cubiomes_colors(out: *mut u8);
}

pub(super) fn generate(
    version: super::Version,
    seed: i64,
    dimension: super::Dimension,
    range: super::Range,
) -> Result<Vec<i32>, super::Error> {
    let mut out = vec![0; range.width as usize * range.height as usize];
    // SAFETY: only validated bounded ranges reach this private function. The
    // bridge allocates cubiomes' scratch cache and copies exactly out.len() ints.
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
        )
    };
    if result == 0 {
        Ok(out)
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
