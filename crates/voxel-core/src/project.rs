use crate::brush::{self, BrushOptions, MirrorAxes};
use crate::scene::{ModelId, NodeId, Scene, SceneNode, WorldInstance};
use crate::{BrushKind, ColorIndex, ColorRgba, IVec3, Material, Palette, Result, VoxelModel};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

const UNDO_CAP: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EditMode {
    #[default]
    Model,
    World,
}

#[derive(Default)]
struct UndoHistory {
    undo: VecDeque<ProjectInner>,
    redo: VecDeque<ProjectInner>,
}

/// Shared editable document. Safe for GUI + MCP.
/// Undo stacks live here (not in [`ProjectInner`]) so snapshots do not clone history.
#[derive(Clone)]
pub struct Project {
    inner: Arc<Mutex<ProjectInner>>,
    history: Arc<Mutex<UndoHistory>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    fn wrap(inner: ProjectInner) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
            history: Arc::new(Mutex::new(UndoHistory::default())),
        }
    }

    pub fn new(size: u32) -> Result<Self> {
        let scene = Scene::new_empty_world(size)?;
        Ok(Self::wrap(ProjectInner {
            scene,
            palette: Palette::default(),
            active_color: 1,
            brush: BrushOptions::default(),
            active_model: 0,
            selected_node: Some(0),
            edit_mode: EditMode::Model,
        }))
    }

    pub fn from_parts(model: VoxelModel, palette: Palette) -> Self {
        Self::wrap(ProjectInner {
            scene: Scene::from_single_model(model),
            palette,
            active_color: 1,
            brush: BrushOptions::default(),
            active_model: 0,
            selected_node: Some(0),
            edit_mode: EditMode::Model,
        })
    }

    pub fn from_scene(scene: Scene, palette: Palette) -> Self {
        let selected = Some(scene.root);
        Self::wrap(ProjectInner {
            scene,
            palette,
            active_color: 1,
            brush: BrushOptions::default(),
            active_model: 0,
            selected_node: selected,
            edit_mode: EditMode::World,
        })
    }

    /// Push a clone of the current document onto the undo stack and clear redo.
    /// Skips if identical to the last snapshot. Caps undo at [`UNDO_CAP`].
    pub fn checkpoint(&self) {
        let snap = self.inner.lock().clone();
        let mut history = self.history.lock();
        if history.undo.back() == Some(&snap) {
            return;
        }
        if history.undo.len() >= UNDO_CAP {
            history.undo.pop_front();
        }
        history.undo.push_back(snap);
        history.redo.clear();
    }

    pub fn undo(&self) -> bool {
        let mut inner = self.inner.lock();
        let mut history = self.history.lock();
        let Some(prev) = history.undo.pop_back() else {
            return false;
        };
        history.redo.push_back(inner.clone());
        *inner = prev;
        true
    }

    pub fn redo(&self) -> bool {
        let mut inner = self.inner.lock();
        let mut history = self.history.lock();
        let Some(next) = history.redo.pop_back() else {
            return false;
        };
        history.undo.push_back(inner.clone());
        *inner = next;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.history.lock().undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.history.lock().redo.is_empty()
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

    /// Checkpoint, then detach a world-editor transform instance (not the scene root).
    pub fn delete_object(&self, node_id: NodeId) -> bool {
        {
            let inner = self.inner.lock();
            if node_id == inner.scene.root {
                return false;
            }
            if !matches!(
                inner.scene.nodes.get(&node_id),
                Some(SceneNode::Transform(_))
            ) {
                return false;
            }
        }
        self.checkpoint();
        self.with_inner_mut(|p| {
            if !p.scene.remove_object(node_id) {
                return false;
            }
            if p.selected_node == Some(node_id) {
                p.selected_node = p
                    .scene
                    .list_objects()
                    .into_iter()
                    .map(|o| o.id)
                    .next()
                    .or(Some(p.scene.root));
            }
            if let Some(nid) = p.selected_node {
                if let Some(mid) = p.scene.model_id_for_transform(nid) {
                    p.active_model = mid;
                }
            }
            if p.active_model >= p.scene.models.len() {
                p.active_model = 0;
            }
            true
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

    pub fn set_object_layer(&self, node_id: NodeId, layer_id: i32) -> bool {
        self.with_inner_mut(|p| p.scene.set_object_layer(node_id, layer_id))
    }

    pub fn set_layer_hidden(&self, layer_id: i32, hidden: bool) -> bool {
        self.with_inner_mut(|p| p.scene.layers.set_hidden(layer_id, hidden))
    }

    pub fn rename_layer(&self, layer_id: i32, name: String) -> bool {
        self.with_inner_mut(|p| p.scene.layers.rename(layer_id, name))
    }

    pub fn set_material(&self, index: ColorIndex, material: Material) -> Result<()> {
        self.with_inner_mut(|p| p.scene.materials.set(index, material))
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

    pub fn fill_line(&self, a: IVec3, b: IVec3, color: ColorIndex) -> Result<usize> {
        self.with_inner_mut(|p| brush::fill_line(p.model_mut(), a, b, color))
    }

    pub fn fill_circle(
        &self,
        center: IVec3,
        radius: u32,
        axis: char,
        color: ColorIndex,
    ) -> Result<usize> {
        self.with_inner_mut(|p| brush::fill_circle(p.model_mut(), center, radius, axis, color))
    }

    pub fn fill_plane_rect(
        &self,
        a: IVec3,
        b: IVec3,
        axis: char,
        color: ColorIndex,
    ) -> Result<usize> {
        self.with_inner_mut(|p| brush::fill_plane_rect(p.model_mut(), a, b, axis, color))
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
                .map(|o| {
                    serde_json::json!({
                        "id": o.id,
                        "name": o.name,
                        "translation": [o.translation.x, o.translation.y, o.translation.z],
                        "hidden": o.hidden,
                        "model_id": o.model_id,
                        "layer_id": o.layer_id,
                    })
                })
                .collect();
            let active_mat = p.scene.materials.get(p.active_color);
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
                "layers": p.scene.layers.to_json(),
                "material": active_mat.to_json(p.active_color),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_restores_a_voxel() {
        let project = Project::new(8).unwrap();
        project.checkpoint();
        project.set_voxel(1, 2, 3, 7).unwrap();
        assert_eq!(
            project.with_inner(|p| p.model().get(1, 2, 3).unwrap()),
            7
        );
        assert!(project.can_undo());
        assert!(!project.can_redo());
        assert!(project.undo());
        assert_eq!(
            project.with_inner(|p| p.model().get(1, 2, 3).unwrap()),
            0
        );
        assert!(project.can_redo());
        assert!(project.redo());
        assert_eq!(
            project.with_inner(|p| p.model().get(1, 2, 3).unwrap()),
            7
        );
    }
}
