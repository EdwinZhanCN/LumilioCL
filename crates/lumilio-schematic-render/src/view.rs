/// Orbit camera around the middle of the schematic. The distance is chosen so the whole build fits
/// at `zoom == 1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub zoom: f32,
    pub target: Option<[f32; 3]>,
    /// An explicit eye position selects first-person perspective instead of orbit fitting.
    pub position: Option<[f32; 3]>,
    /// Linear RGBA clear colour, supplied by the host's theme.
    pub background: Option<[f32; 4]>,
}

impl View {
    pub const MIN_ZOOM: f32 = 0.3;
    pub const MAX_ZOOM: f32 = 8.0;
    /// Looking straight up or down flips the camera's up vector.
    pub const MAX_PITCH: f32 = 85.0;

    pub const fn new() -> Self {
        Self {
            yaw_deg: 40.0,
            pitch_deg: 30.0,
            zoom: 1.0,
            target: None,
            position: None,
            background: None,
        }
    }

    /// Dragging right turns the build to the right, dragging down tilts the camera over it.
    pub fn orbit(self, dx_deg: f32, dy_deg: f32) -> Self {
        Self {
            yaw_deg: (self.yaw_deg - dx_deg).rem_euclid(360.0),
            pitch_deg: (self.pitch_deg + dy_deg).clamp(-Self::MAX_PITCH, Self::MAX_PITCH),
            ..self
        }
    }

    pub fn zoomed(self, factor: f32) -> Self {
        Self {
            zoom: (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM),
            ..self
        }
    }

    /// First-person mouse look: yaw and pitch use the renderer's orbit convention.
    pub fn look(self, dx_deg: f32, dy_deg: f32) -> Self {
        self.orbit(dx_deg, dy_deg)
    }

    /// Creative-style flight: WASD follows horizontal heading, Space/Shift changes height.
    /// Normalize combined input so diagonals never move faster. Distance is in blocks.
    pub fn moved(self, right: f32, up: f32, forward: f32, distance: f32) -> Self {
        let Some(mut position) = self.position else {
            return self;
        };
        let length = (right * right + up * up + forward * forward).sqrt().max(1.);
        let yaw = self.yaw_deg.to_radians();
        position[0] += (right * yaw.cos() - forward * yaw.sin()) * distance / length;
        position[1] += up * distance / length;
        position[2] += (-right * yaw.sin() - forward * yaw.cos()) * distance / length;
        Self {
            position: Some(position),
            ..self
        }
    }
}

impl Default for View {
    fn default() -> Self {
        Self::new()
    }
}
