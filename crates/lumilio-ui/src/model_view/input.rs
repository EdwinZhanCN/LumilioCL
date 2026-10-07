use lumilio_schematic_render::View;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Orbital,
    Explore,
}

#[derive(Default)]
pub(super) struct FlightInput {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    up: bool,
    pub(super) shift: bool,
}

impl FlightInput {
    pub(super) fn key(&mut self, key: &str, pressed: bool) -> bool {
        let held = match key {
            "w" | "W" => &mut self.forward,
            "s" | "S" => &mut self.backward,
            "a" | "A" => &mut self.left,
            "d" | "D" => &mut self.right,
            "space" | " " => &mut self.up,
            _ => return false,
        };
        *held = pressed;
        true
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn advance(&self, view: View, elapsed: Duration) -> View {
        let axis = |positive: bool, negative: bool| {
            f32::from(u8::from(positive)) - f32::from(u8::from(negative))
        };
        // Minecraft creative flight's base horizontal speed: about 10.9 blocks/s.
        view.moved(
            axis(self.right, self.left),
            axis(self.up, self.shift),
            axis(self.forward, self.backward),
            elapsed.as_secs_f32() * 10.9,
        )
    }
}
