#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Camera {
    pub x: f64,
    pub z: f64,
    pub scale: f64,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.,
            z: 0.,
            scale: 4.,
        }
    }
}
impl Camera {
    pub fn pan(&mut self, dx: f64, dz: f64) {
        self.x = (self.x - dx * self.scale).clamp(-29_900_000., 29_900_000.);
        self.z = (self.z - dz * self.scale).clamp(-29_900_000., 29_900_000.);
    }
    pub fn zoom(&mut self, factor: f64, at: [f64; 2], size: [u32; 2]) {
        let old = self.scale;
        self.scale = (self.scale * factor).clamp(0.25, 256.);
        self.x += (at[0] - f64::from(size[0]) / 2.) * (old - self.scale);
        self.z += (at[1] - f64::from(size[1]) / 2.) * (old - self.scale);
        self.pan(0., 0.);
    }
    /// The tile level whose texels are at most twice as coarse as a screen
    /// pixel. Generating at a finer level than that costs 4x per level while
    /// the extra detail is minified away.
    pub fn level(&self) -> u8 {
        (((self.scale * 2.).max(1.).ln() / 4_f64.ln()).floor() as u8).min(4)
    }
    pub fn world(&self, at: [f64; 2], size: [u32; 2]) -> [f64; 2] {
        [
            self.x + (at[0] - f64::from(size[0]) / 2.) * self.scale,
            self.z + (at[1] - f64::from(size[1]) / 2.) * self.scale,
        ]
    }
}
