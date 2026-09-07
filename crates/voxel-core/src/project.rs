use crate::brush::{self, BrushOptions, MirrorAxes};
use crate::scene::{ModelId, NodeId, Scene, WorldInstance};
use crate::{BrushKind, ColorIndex, ColorRgba, IVec3, Palette, Result, VoxelModel};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EditMode {
    #[default]
    Model,
    World,
}

/// Shared editable document. Safe for GUI + MCP.
#[derive(Clone)]
pub struct Project {
    inner: Arc<Mutex<ProjectInner>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInner {
    pub scene: Scene,
    pub palette: Palette,
    pub active_color: ColorIndex,
    pub brush: BrushOptions,
    pub active_model: ModelId,
    pub selected_node: Option<NodeId>,
    pub edit_mode: EditMode,
}

impl ProjectInner {
    /// Active model (model editor target).
    pub fn model(&self) -> &VoxelModel {
        self.scene
            .model(self.active_model)
            .expect("active model missing")
    }

    pub fn model_mut(&mut self) -> &mut VoxelModel {
        let id = self.active_model;
        self.scene
            .model_mut(id)
            .expect("active model missing")
    }
}

impl Project {
    pub fn new(size: u32) -> Result<Self> {
        let scene = Scene::new_empty_world(size)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(ProjectInner {
                scene,
                palette: Palette::default(),
                active_color: 1,
                brush: BrushOptions::default(),
                active_model: 0,
                selected_node: Some(0),
                edit_mode: EditMode::Model,
            })),
        })
    }

    pub fn from_parts(model: VoxelModel, palette: Palette) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ProjectInner {
                scene: Scene::from_single_model(model),
                palette,
                active_color: 1,
                brush: BrushOptions::default(),
                active_model: 0,
                selected_node: Some(0),
                edit_mode: EditMode::Model,
            })),
        }
    }

    pub fn from_scene(scene: Scene, palette: Palette) -> Self {
        let selected = Some(scene.root);
        Self {
            inner: Arc::new(Mutex::new(ProjectInner {
                scene,
                palette,
                active_color: 1,
                brush: BrushOptions::default(),
                active_model: 0,
                selected_node: selected,
                edit_mode: EditMode::World,
            })),
        }
    }

    pub fn with_inner<R>(&self, f: impl FnOnce(&ProjectInner) -> R) -> R {
        f(&self.inner.lock())
    }

    pub fn with_inner_mut<R>(&self, f: impl FnOnce(&mut ProjectInner) -> R) -> R {
        f(&mut self.inner.lock())
    }

    pub fn snapshot(&self) -> ProjectInner {
        self.inner.lock().clone()
    }

    pub fn set_edit_mode(&self, mode: EditMode) {
        self.with_inner_mut(|p| p.edit_mode = mode);
    }

    pub fn set_active_model(&self, id: ModelId) -> bool {
        self.with_inner_mut(|p| {
            if id < p.scene.models.len() {
                p.active_model = id;
                true
            } else {
                false
            }
        })
    }

    pub fn select_node(&self, id: Option<NodeId>) {
        self.with_inner_mut(|p| {
            p.selected_node = id;
            if let Some(nid) = id {
                if let Some(mid) = p.scene.model_id_for_transform(nid) {
                    p.active_model = mid;
                }
            }
        });
    }

    pub fn add_object(&self, name: &str, size: u32, translation: IVec3) -> Result<NodeId> {
        self.with_inner_mut(|p| {
            let model = VoxelModel::new(size, size, size)?;
            let id = p.scene.add_object(name, model, translation);
            if let Some(mid) = p.scene.model_id_for_transform(id) {
                p.active_model = mid;
            }
            p.selected_node = Some(id);
            p.edit_mode = EditMode::World;
            Ok(id)
        })
    }

    pub fn duplicate_active_object(&self, name: &str, translation: IVec3) -> Result<NodeId> {
        self.with_inner_mut(|p| {
            let model = p.model().clone();
            let id = p.scene.add_object(name, model, translation);
            if let Some(mid) = p.scene.model_id_for_transform(id) {
                p.active_model = mid;
            }
            p.selected_node = Some(id);
            Ok(id)
        })
    }

    pub fn set_object_translation(&self, node_id: NodeId, t: IVec3) -> bool {
        self.with_inner_mut(|p| p.scene.set_translation(node_id, t))
    }

    pub fn instances(&self) -> Vec<WorldInstance> {
        self.with_inner(|p| p.scene.collect_instances())
    }

    pub fn set_voxel(&self, x: i32, y: i32, z: i32, color: ColorIndex) -> Result<()> {
        self.with_inner_mut(|p| p.model_mut().set(x, y, z, color))
    }

    pub fn erase_voxel(&self, x: i32, y: i32, z: i32) -> Result<()> {
        self.set_voxel(x, y, z, 0)
    }

    pub fn fill_box(&self, min: IVec3, max: IVec3, color: ColorIndex) -> Result<usize> {
        self.with_inner_mut(|p| brush::fill_box_region(p.model_mut(), min, max, color))
    }

    pub fn fill_sphere(&self, center: IVec3, radius: u32, color: ColorIndex) -> Result<usize> {
        self.with_inner_mut(|p| brush::fill_sphere(p.model_mut(), center, radius, color))
    }

    pub fn flood_fill(&self, start: IVec3, color: ColorIndex) -> Result<usize> {
        self.with_inner_mut(|p| brush::flood_fill(p.model_mut(), start, color))
    }

    pub fn apply_brush(&self, center: IVec3) -> Result<usize> {
        self.with_inner_mut(|p| {
            let mut opts = p.brush;
            opts.color = p.active_color;
            brush::apply_brush(p.model_mut(), center, opts)
        })
    }

    pub fn set_palette_color(&self, index: u8, color: ColorRgba) -> Result<()> {
        self.with_inner_mut(|p| p.palette.set(index, color))
    }

    pub fn set_active_color(&self, index: ColorIndex) {
        self.with_inner_mut(|p| {
            if index != 0 {
                p.active_color = index;
                p.brush.color = index;
            }
        });
    }

    pub fn set_brush(&self, kind: BrushKind, radius: u32, mirror: MirrorAxes) {
        self.with_inner_mut(|p| {
            p.brush.kind = kind;
            p.brush.radius = radius;
            p.brush.mirror = mirror;
            p.brush.color = p.active_color;
        });
    }

    pub fn clear(&self) {
        self.with_inner_mut(|p| p.model_mut().clear());
    }

    pub fn resize(&self, sx: u32, sy: u32, sz: u32) -> Result<()> {
        self.with_inner_mut(|p| p.model_mut().resize(sx, sy, sz))
    }

    pub fn mirror(&self, axis: char) -> Result<()> {
        self.with_inner_mut(|p| brush::mirror_model(p.model_mut(), axis))
    }

    pub fn info_json(&self) -> serde_json::Value {
        self.with_inner(|p| {
            let (sx, sy, sz) = p.model().size();
            let objects: Vec<_> = p
                .scene
                .list_objects()
                .into_iter()
                .map(|(id, name, t, hidden, mid)| {
                    serde_json::json!({
                        "id": id,
                        "name": name,
                        "translation": [t.x, t.y, t.z],
                        "hidden": hidden,
                        "model_id": mid,
                    })
                })
                .collect();
            serde_json::json!({
                "edit_mode": p.edit_mode,
                "active_model": p.active_model,
                "selected_node": p.selected_node,
                "model_count": p.scene.models.len(),
                "size": [sx, sy, sz],
                "voxel_count": p.model().voxel_count(),
                "active_color": p.active_color,
                "brush": p.brush,
                "objects": objects,
            })
        })
    }
}
