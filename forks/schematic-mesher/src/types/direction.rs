//! Direction and axis types for face and rotation handling.

use serde::{Deserialize, Serialize};

/// The six cardinal directions / face directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    /// All six directions in order.
    pub const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    /// Get the offset for this direction.
    pub fn offset(&self) -> (i32, i32, i32) {
        match self {
            Direction::Down => (0, -1, 0),
            Direction::Up => (0, 1, 0),
            Direction::North => (0, 0, -1),
            Direction::South => (0, 0, 1),
            Direction::West => (-1, 0, 0),
            Direction::East => (1, 0, 0),
        }
    }

    /// Get the normal vector for this direction.
    pub fn normal(&self) -> [f32; 3] {
        match self {
            Direction::Down => [0.0, -1.0, 0.0],
            Direction::Up => [0.0, 1.0, 0.0],
            Direction::North => [0.0, 0.0, -1.0],
            Direction::South => [0.0, 0.0, 1.0],
            Direction::West => [-1.0, 0.0, 0.0],
            Direction::East => [1.0, 0.0, 0.0],
        }
    }

    /// Get the opposite direction.
    pub fn opposite(&self) -> Direction {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    /// Get the axis this direction is on.
    pub fn axis(&self) -> Axis {
        match self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
        }
    }

    /// Parse from string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "down" => Some(Direction::Down),
            "up" => Some(Direction::Up),
            "north" => Some(Direction::North),
            "south" => Some(Direction::South),
            "west" => Some(Direction::West),
            "east" => Some(Direction::East),
            _ => None,
        }
    }

    /// Rotate this direction by X rotation (around X axis, in 90-degree increments).
    /// Looking from +X towards origin, positive rotation goes Up -> North -> Down -> South.
    pub fn rotate_x(self, degrees: i32) -> Direction {
        let steps = ((degrees / 90) % 4 + 4) % 4;
        let mut dir = self;
        for _ in 0..steps {
            dir = match dir {
                Direction::Up => Direction::North,
                Direction::North => Direction::Down,
                Direction::Down => Direction::South,
                Direction::South => Direction::Up,
                // X rotation doesn't affect East/West
                Direction::East => Direction::East,
                Direction::West => Direction::West,
            };
        }
        dir
    }

    /// Rotate this direction by Y rotation (around Y axis, in 90-degree increments).
    /// Looking from +Y (above), positive rotation goes North -> East -> South -> West.
    pub fn rotate_y(self, degrees: i32) -> Direction {
        let steps = ((degrees / 90) % 4 + 4) % 4;
        let mut dir = self;
        for _ in 0..steps {
            dir = match dir {
                Direction::North => Direction::East,
                Direction::East => Direction::South,
                Direction::South => Direction::West,
                Direction::West => Direction::North,
                // Y rotation doesn't affect Up/Down
                Direction::Up => Direction::Up,
                Direction::Down => Direction::Down,
            };
        }
        dir
    }

    /// Rotate this direction by a block transform (X then Y rotation).
    pub fn rotate_by_transform(self, x_rot: i32, y_rot: i32) -> Direction {
        // Apply X rotation first, then Y rotation (same order as geometry transform)
        self.rotate_x(x_rot).rotate_y(y_rot)
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Down => write!(f, "down"),
            Direction::Up => write!(f, "up"),
            Direction::North => write!(f, "north"),
            Direction::South => write!(f, "south"),
            Direction::West => write!(f, "west"),
            Direction::East => write!(f, "east"),
        }
    }
}

/// The three axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// Get the unit vector for this axis.
    pub fn unit_vector(&self) -> [f32; 3] {
        match self {
            Axis::X => [1.0, 0.0, 0.0],
            Axis::Y => [0.0, 1.0, 0.0],
            Axis::Z => [0.0, 0.0, 1.0],
        }
    }

    /// Parse from string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }
}

impl std::fmt::Display for Axis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Axis::X => write!(f, "x"),
            Axis::Y => write!(f, "y"),
            Axis::Z => write!(f, "z"),
        }
    }
}
