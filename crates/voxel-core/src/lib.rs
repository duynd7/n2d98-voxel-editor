//! Core voxel data structures and editing ops.
//! Feature set is intentionally aligned with MagicaVoxel model + world editing
//! — MagicaVoxel itself is closed-source; we reimplement from public .vox docs.

mod brush;
mod model;
mod palette;
mod project;
mod scene;

pub use brush::{BrushKind, BrushOptions, MirrorAxes};
pub use model::{IVec3, VoxelModel};
pub use palette::{ColorRgba, Palette};
pub use project::{EditMode, Project, ProjectInner};
pub use scene::{
    local_to_world, world_to_local, GroupNode, ModelId, MvRotation, NodeId, Scene, SceneNode,
    ShapeNode, TransformNode, WorldInstance,
};

pub type ColorIndex = u8;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("coordinates out of bounds: ({x}, {y}, {z}) for size {sx}x{sy}x{sz}")]
    OutOfBounds {
        x: i32,
        y: i32,
        z: i32,
        sx: u32,
        sy: u32,
        sz: u32,
    },
    #[error("invalid color index {0} (0 = empty, 1..=255 valid)")]
    InvalidColor(u8),
    #[error("invalid size {0}x{1}x{2} (each axis must be 1..=256)")]
    InvalidSize(u32, u32, u32),
}

pub type Result<T> = std::result::Result<T, CoreError>;
