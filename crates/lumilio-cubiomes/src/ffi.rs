#![allow(unsafe_code)]

unsafe extern "C" {
    fn lumilio_cubiomes_probe() -> i32;
}

pub(super) fn probe() -> i32 {
    // SAFETY: the C probe owns its generator and accepts no pointers or input.
    unsafe { lumilio_cubiomes_probe() }
}
