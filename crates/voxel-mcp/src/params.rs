use rmcp::schemars;
use serde::Deserialize;
use voxel_core::MirrorAxes;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct Vec3Params {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetVoxelParams {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Palette index 1..=255 (0 erases).
    pub color: u8,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FillBoxParams {
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
    pub color: u8,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FillSphereParams {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub radius: u32,
    pub color: u8,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FloodFillParams {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub color: u8,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PaletteColorParams {
    /// Palette index 1..=255
    pub index: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default = "default_alpha")]
    pub a: u8,
}

fn default_alpha() -> u8 {
    255
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ActiveColorParams {
    pub index: u8,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BrushParams {
    /// voxel | box | sphere | erase | flood_fill
    pub kind: String,
    #[serde(default)]
    pub radius: u32,
    #[serde(default)]
    pub mirror_x: bool,
    #[serde(default)]
    pub mirror_y: bool,
    #[serde(default)]
    pub mirror_z: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ResizeParams {
    pub size_x: u32,
    pub size_y: u32,
    pub size_z: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MirrorParams {
    /// x | y | z
    pub axis: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PathParams {
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ApplyBrushParams {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NodeIdParams {
    pub node_id: i32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ModelIdParams {
    pub model_id: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditModeParams {
    /// model | world
    pub mode: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ObjectTranslationParams {
    pub node_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddObjectParams {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(default)]
    pub size: u32,
}

impl BrushParams {
    pub fn mirror(&self) -> MirrorAxes {
        MirrorAxes {
            x: self.mirror_x,
            y: self.mirror_y,
            z: self.mirror_z,
        }
    }
}
