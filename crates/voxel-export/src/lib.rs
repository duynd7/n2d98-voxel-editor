//! Engine exporters. Godot first: glTF 2.0 binary (.glb) + optional .tscn wrapper.

mod glb;
mod mesh;

use mesh::build_local_mesh;
use std::path::{Path, PathBuf};
use voxel_core::Project;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no voxels to export")]
    Empty,
    #[error("{0}")]
    Msg(String),
}

pub type Result<T> = std::result::Result<T, ExportError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportScope {
    /// Every visible world instance (separate glTF nodes).
    World,
    /// Active model only, at the origin.
    ActiveModel,
}

impl ExportScope {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "" | "world" | "w" | "scene" => Ok(Self::World),
            "model" | "m" | "active" => Ok(Self::ActiveModel),
            other => Err(ExportError::Msg(format!(
                "unknown export scope '{other}' (use world or model)"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GodotExport {
    pub glb_path: PathBuf,
    pub tscn_path: PathBuf,
    pub meshes: usize,
    pub vertices: usize,
    pub triangles: usize,
}

pub fn export_godot(
    project: &Project,
    path: impl AsRef<Path>,
    scope: ExportScope,
) -> Result<GodotExport> {
    let glb_path = resolve_glb_path(path.as_ref());
    if let Some(parent) = glb_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let meshes = collect_meshes(project, scope);
    let vertices: usize = meshes.iter().map(|m| m.positions.len()).sum();
    let triangles: usize = meshes.iter().map(|m| m.indices.len() / 3).sum();
    let mesh_count = meshes.iter().filter(|m| !m.is_empty()).count();

    let bytes = glb::write_glb(&meshes)?;
    std::fs::write(&glb_path, bytes)?;

    let tscn_path = glb_path.with_extension("tscn");
    let file_name = glb_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("model.glb");
    std::fs::write(&tscn_path, godot_tscn(file_name))?;

    Ok(GodotExport {
        glb_path,
        tscn_path,
        meshes: mesh_count,
        vertices,
        triangles,
    })
}

fn collect_meshes(project: &Project, scope: ExportScope) -> Vec<mesh::CpuMesh> {
    let snap = project.snapshot();
    match scope {
        ExportScope::ActiveModel => {
            vec![build_local_mesh(
                "model",
                snap.model(),
                &snap.palette,
                &snap.scene.materials,
                voxel_core::IVec3::new(0, 0, 0),
                voxel_core::MvRotation::IDENTITY,
            )]
        }
        ExportScope::World => {
            let mut out = Vec::new();
            let mut used: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for inst in snap.scene.collect_instances() {
                if inst.hidden {
                    continue;
                }
                let Some(model) = snap.scene.model(inst.model_id) else {
                    continue;
                };
                let base = if inst.name.is_empty() {
                    format!("object_{}", inst.transform_id)
                } else {
                    inst.name.clone()
                };
                let n = used.entry(base.clone()).or_insert(0);
                *n += 1;
                let name = if *n == 1 {
                    base
                } else {
                    format!("{base}_{n}")
                };
                out.push(build_local_mesh(
                    name,
                    model,
                    &snap.palette,
                    &snap.scene.materials,
                    inst.translation,
                    inst.rotation,
                ));
            }
            out
        }
    }
}

fn resolve_glb_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        return path.join("voxel_model.glb");
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some("glb" | "gltf" | "tscn") => path.with_extension("glb"),
        Some(_) => path.to_path_buf(),
        None => path.with_extension("glb"),
    }
}

fn godot_tscn(glb_file_name: &str) -> String {
    format!(
        "[gd_scene load_steps=2 format=3]\n\
         \n\
         [ext_resource type=\"PackedScene\" path=\"res://{glb_file_name}\" id=\"1_glb\"]\n\
         \n\
         [node name=\"VoxelWorld\" instance=ExtResource(\"1_glb\")]\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::{IVec3, Project};

    #[test]
    fn glb_has_magic_and_voxel_faces() {
        let p = Project::new(8).unwrap();
        p.fill_box(IVec3::new(1, 1, 1), IVec3::new(2, 2, 2), 7)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let out = export_godot(&p, dir.path().join("cube.glb"), ExportScope::World).unwrap();
        let bytes = std::fs::read(&out.glb_path).unwrap();
        assert_eq!(&bytes[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 2);
        assert!(out.triangles > 0);
        assert!(out.tscn_path.exists());
    }
}
