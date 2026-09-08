//! MagicaVoxel `.vox` reader/writer with scene graph (nTRN/nGRP/nSHP).

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use voxel_core::{ColorRgba, Palette, Project, Scene, VoxelModel};

mod dict;
mod reader;
mod writer;

pub use reader::read_vox_scene;
pub use writer::write_vox_scene;

#[derive(Debug, thiserror::Error)]
pub enum VoxError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid VOX magic")]
    BadMagic,
    #[error("unsupported VOX version {0}")]
    BadVersion(u32),
    #[error("malformed chunk `{0}`")]
    BadChunk(&'static str),
    #[error("core error: {0}")]
    Core(#[from] voxel_core::CoreError),
    #[error("{0}")]
    Msg(String),
}

pub type Result<T> = std::result::Result<T, VoxError>;

pub fn load_path(path: impl AsRef<Path>) -> Result<Project> {
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let (scene, palette) = read_vox_scene(&buf)?;
    Ok(Project::from_scene(scene, palette))
}

pub fn save_path(project: &Project, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let snap = project.snapshot();
    let bytes = write_vox_scene(&snap.scene, &snap.palette)?;
    let tmp = sibling_temp(path);
    std::fs::write(&tmp, &bytes)?;
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn sibling_temp(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| "project.vox".into());
    name.push(".tmp");
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

/// Legacy single-model API.
pub fn read_vox(data: &[u8]) -> Result<(VoxelModel, Palette)> {
    let (scene, palette) = read_vox_scene(data)?;
    let model = scene
        .models
        .into_iter()
        .next()
        .ok_or_else(|| VoxError::Msg("empty scene".into()))?;
    Ok((model, palette))
}

pub fn write_vox(model: &VoxelModel, palette: &Palette) -> Result<Vec<u8>> {
    write_vox_scene(&Scene::from_single_model(model.clone()), palette)
}

pub fn default_palette_from_bytes(rgba_chunk: &[u8]) -> Result<Palette> {
    if rgba_chunk.len() < 256 * 4 {
        return Err(VoxError::BadChunk("RGBA"));
    }
    let mut palette = Palette::magicavoxel_default();
    for i in 0..255 {
        let o = i * 4;
        let color = ColorRgba::new(
            rgba_chunk[o],
            rgba_chunk[o + 1],
            rgba_chunk[o + 2],
            rgba_chunk[o + 3],
        );
        let _ = palette.set((i + 1) as u8, color);
    }
    Ok(palette)
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::{IVec3, Scene, VoxelModel};

    #[test]
    fn roundtrip_basic_vox() {
        let mut model = VoxelModel::new(8, 8, 8).unwrap();
        model.set(1, 2, 3, 7).unwrap();
        model.set(4, 5, 6, 12).unwrap();
        let mut palette = Palette::magicavoxel_default();
        palette.set(7, ColorRgba::rgb(10, 20, 30)).unwrap();

        let bytes = write_vox(&model, &palette).unwrap();
        let (m2, p2) = read_vox(&bytes).unwrap();
        assert_eq!(m2.get(1, 2, 3).unwrap(), 7);
        assert_eq!(m2.get(4, 5, 6).unwrap(), 12);
        assert_eq!(p2.get(7), ColorRgba::rgb(10, 20, 30));
        assert_eq!(m2.voxel_count(), 2);
    }

    #[test]
    fn roundtrip_scene_two_models() {
        let mut scene = Scene::from_single_model(VoxelModel::new(8, 8, 8).unwrap());
        scene.ensure_world_root();
        let mut m2 = VoxelModel::new(4, 4, 4).unwrap();
        m2.set(1, 1, 1, 3).unwrap();
        let id = scene.add_object("b", m2, IVec3::new(20, 0, 0));
        assert!(id > 0);
        let palette = Palette::magicavoxel_default();
        let bytes = write_vox_scene(&scene, &palette).unwrap();
        let (s2, _) = read_vox_scene(&bytes).unwrap();
        assert_eq!(s2.models.len(), 2);
        assert_eq!(s2.model(1).unwrap().get(1, 1, 1).unwrap(), 3);
        let objs = s2.list_objects();
        assert!(objs.len() >= 1);
    }
}
