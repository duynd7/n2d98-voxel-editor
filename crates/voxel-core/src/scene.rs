//! MagicaVoxel-style scene graph (world editor).
//! Nodes: Transform (nTRN) → Group (nGRP) | Shape (nSHP).

use crate::{IVec3, Result, VoxelModel};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type NodeId = i32;
pub type ModelId = usize;

/// MagicaVoxel packed rotation (single byte) + helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MvRotation(pub u8);

impl MvRotation {
    pub const IDENTITY: Self = Self(0);

    /// Decode to row-major 3×3 with entries in {-1,0,1}.
    pub fn to_matrix(self) -> [[i32; 3]; 3] {
        let r = self.0;
        let mut m = [[0_i32; 3]; 3];
        let index1 = (r & 0b11) as usize;
        let index2 = ((r >> 2) & 0b11) as usize;
        let index3 = !(index1 | index2) & 0b11; // remaining axis 0/1/2
        // MagicaVoxel packs row0/row1 non-zero column indices; row2 is the leftover.
        // If index1|index2 somehow invalid, fall back to identity.
        let i1 = index1.min(2);
        let i2 = if index2 == index1 { (index1 + 1) % 3 } else { index2.min(2) };
        let i3 = if i1 != 0 && i2 != 0 {
            0
        } else if i1 != 1 && i2 != 1 {
            1
        } else {
            2
        };
        let _ = index3;
        let s0 = if (r & (1 << 4)) != 0 { -1 } else { 1 };
        let s1 = if (r & (1 << 5)) != 0 { -1 } else { 1 };
        let s2 = if (r & (1 << 6)) != 0 { -1 } else { 1 };
        m[0][i1] = s0;
        m[1][i2] = s1;
        m[2][i3] = s2;
        // Ensure proper rotation (det = ±1). If zero row, identity.
        if m[0] == [0, 0, 0] || m[1] == [0, 0, 0] || m[2] == [0, 0, 0] {
            return [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
        }
        m
    }

    pub fn from_matrix(m: [[i32; 3]; 3]) -> Self {
        // Find non-zero columns for first two rows
        let col = |row: usize| -> (usize, i32) {
            for c in 0..3 {
                if m[row][c] != 0 {
                    return (c, m[row][c].signum());
                }
            }
            (0, 1)
        };
        let (c0, s0) = col(0);
        let (c1, s1) = col(1);
        let (_, s2) = col(2);
        let mut r = 0u8;
        r |= (c0 as u8) & 0b11;
        r |= ((c1 as u8) & 0b11) << 2;
        if s0 < 0 {
            r |= 1 << 4;
        }
        if s1 < 0 {
            r |= 1 << 5;
        }
        if s2 < 0 {
            r |= 1 << 6;
        }
        Self(r)
    }

    pub fn rotate_vec(self, v: IVec3) -> IVec3 {
        let m = self.to_matrix();
        IVec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformNode {
    pub id: NodeId,
    pub name: String,
    pub hidden: bool,
    pub child: NodeId,
    pub translation: IVec3,
    pub rotation: MvRotation,
    pub layer_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupNode {
    pub id: NodeId,
    pub name: String,
    pub children: Vec<NodeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeNode {
    pub id: NodeId,
    pub model_ids: Vec<ModelId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SceneNode {
    Transform(TransformNode),
    Group(GroupNode),
    Shape(ShapeNode),
}

impl SceneNode {
    pub fn id(&self) -> NodeId {
        match self {
            Self::Transform(n) => n.id,
            Self::Group(n) => n.id,
            Self::Shape(n) => n.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Transform(n) => &n.name,
            Self::Group(n) => &n.name,
            Self::Shape(_) => "",
        }
    }
}

/// Flattened drawable instance in world space.
#[derive(Debug, Clone)]
pub struct WorldInstance {
    pub transform_id: NodeId,
    pub model_id: ModelId,
    pub name: String,
    pub translation: IVec3,
    pub rotation: MvRotation,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub models: Vec<VoxelModel>,
    pub nodes: HashMap<NodeId, SceneNode>,
    pub root: NodeId,
    pub next_id: NodeId,
}

impl Scene {
    /// Single model at origin (backward-compatible default).
    pub fn from_single_model(model: VoxelModel) -> Self {
        let mut nodes = HashMap::new();
        let shape_id = 1;
        let trn_id = 0;
        nodes.insert(
            shape_id,
            SceneNode::Shape(ShapeNode {
                id: shape_id,
                model_ids: vec![0],
            }),
        );
        nodes.insert(
            trn_id,
            SceneNode::Transform(TransformNode {
                id: trn_id,
                name: "object".into(),
                hidden: false,
                child: shape_id,
                translation: IVec3::new(0, 0, 0),
                rotation: MvRotation::IDENTITY,
                layer_id: -1,
            }),
        );
        Self {
            models: vec![model],
            nodes,
            root: trn_id,
            next_id: 2,
        }
    }

    pub fn new_empty_world(default_size: u32) -> Result<Self> {
        let model = VoxelModel::new(default_size, default_size, default_size)?;
        Ok(Self::from_single_model(model))
    }

    pub fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn model(&self, id: ModelId) -> Option<&VoxelModel> {
        self.models.get(id)
    }

    pub fn model_mut(&mut self, id: ModelId) -> Option<&mut VoxelModel> {
        self.models.get_mut(id)
    }

    /// Flatten scene into drawable instances (world editor view).
    pub fn collect_instances(&self) -> Vec<WorldInstance> {
        let mut out = Vec::new();
        self.walk_named(
            self.root,
            IVec3::new(0, 0, 0),
            MvRotation::IDENTITY,
            false,
            0,
            "",
            &mut out,
        );
        out
    }

    fn walk_named(
        &self,
        node_id: NodeId,
        parent_t: IVec3,
        parent_r: MvRotation,
        parent_hidden: bool,
        owner_trn: NodeId,
        owner_name: &str,
        out: &mut Vec<WorldInstance>,
    ) {
        let Some(node) = self.nodes.get(&node_id) else {
            return;
        };
        match node {
            SceneNode::Transform(trn) => {
                let hidden = parent_hidden || trn.hidden;
                let composed_t = {
                    let lt = parent_r.rotate_vec(trn.translation);
                    IVec3::new(parent_t.x + lt.x, parent_t.y + lt.y, parent_t.z + lt.z)
                };
                let composed_r = compose_rotation(parent_r, trn.rotation);
                self.walk_named(
                    trn.child,
                    composed_t,
                    composed_r,
                    hidden,
                    trn.id,
                    &trn.name,
                    out,
                );
            }
            SceneNode::Group(grp) => {
                for &child in &grp.children {
                    self.walk_named(
                        child,
                        parent_t,
                        parent_r,
                        parent_hidden,
                        owner_trn,
                        owner_name,
                        out,
                    );
                }
            }
            SceneNode::Shape(shp) => {
                for &mid in &shp.model_ids {
                    out.push(WorldInstance {
                        transform_id: owner_trn,
                        model_id: mid,
                        name: owner_name.to_string(),
                        translation: parent_t,
                        rotation: parent_r,
                        hidden: parent_hidden,
                    });
                }
            }
        }
    }

    /// Ensure root is Transform → Group so we can add objects.
    pub fn ensure_world_root(&mut self) {
        // If root transform's child is Shape, wrap in Group.
        let Some(SceneNode::Transform(trn)) = self.nodes.get(&self.root).cloned() else {
            return;
        };
        if matches!(self.nodes.get(&trn.child), Some(SceneNode::Group(_))) {
            return;
        }
        let old_child = trn.child;
        let group_id = self.alloc_id();
        // Re-parent old child under a new transform kept? Simpler: make group with one
        // transform that points to old shape — but old root already is that transform.
        // MagicaVoxel structure: root TRN → GRP → [TRN→SHP, TRN→SHP, ...]
        // Convert: create GRP containing current root's child via a new TRN? 
        // Easiest path: create new root TRN→GRP, move old root under GRP.
        let old_root = self.root;
        let new_root = self.alloc_id();
        self.nodes.insert(
            group_id,
            SceneNode::Group(GroupNode {
                id: group_id,
                name: "root".into(),
                children: vec![old_root],
            }),
        );
        self.nodes.insert(
            new_root,
            SceneNode::Transform(TransformNode {
                id: new_root,
                name: "".into(),
                hidden: false,
                child: group_id,
                translation: IVec3::new(0, 0, 0),
                rotation: MvRotation::IDENTITY,
                layer_id: -1,
            }),
        );
        self.root = new_root;
        let _ = old_child;
    }

    /// Add a new object: copy of model or empty, at translation.
    pub fn add_object(
        &mut self,
        name: impl Into<String>,
        model: VoxelModel,
        translation: IVec3,
    ) -> NodeId {
        self.ensure_world_root();
        let model_id = self.models.len();
        self.models.push(model);
        let shape_id = self.alloc_id();
        let trn_id = self.alloc_id();
        self.nodes.insert(
            shape_id,
            SceneNode::Shape(ShapeNode {
                id: shape_id,
                model_ids: vec![model_id],
            }),
        );
        self.nodes.insert(
            trn_id,
            SceneNode::Transform(TransformNode {
                id: trn_id,
                name: name.into(),
                hidden: false,
                child: shape_id,
                translation,
                rotation: MvRotation::IDENTITY,
                layer_id: 0,
            }),
        );
        // Attach under root group
        if let Some(SceneNode::Transform(root_trn)) = self.nodes.get(&self.root).cloned() {
            if let Some(SceneNode::Group(grp)) = self.nodes.get_mut(&root_trn.child) {
                grp.children.push(trn_id);
            }
        }
        trn_id
    }

    pub fn set_translation(&mut self, trn_id: NodeId, t: IVec3) -> bool {
        if let Some(SceneNode::Transform(trn)) = self.nodes.get_mut(&trn_id) {
            trn.translation = t;
            true
        } else {
            false
        }
    }

    pub fn set_hidden(&mut self, trn_id: NodeId, hidden: bool) -> bool {
        if let Some(SceneNode::Transform(trn)) = self.nodes.get_mut(&trn_id) {
            trn.hidden = hidden;
            true
        } else {
            false
        }
    }

    pub fn rename(&mut self, trn_id: NodeId, name: String) -> bool {
        if let Some(SceneNode::Transform(trn)) = self.nodes.get_mut(&trn_id) {
            trn.name = name;
            true
        } else {
            false
        }
    }

    /// Model id referenced by a transform (first shape model).
    pub fn model_id_for_transform(&self, trn_id: NodeId) -> Option<ModelId> {
        let SceneNode::Transform(trn) = self.nodes.get(&trn_id)? else {
            return None;
        };
        self.find_shape_model(trn.child)
    }

    fn find_shape_model(&self, node_id: NodeId) -> Option<ModelId> {
        match self.nodes.get(&node_id)? {
            SceneNode::Shape(s) => s.model_ids.first().copied(),
            SceneNode::Group(g) => g.children.iter().find_map(|&c| self.find_shape_model(c)),
            SceneNode::Transform(t) => self.find_shape_model(t.child),
        }
    }

    pub fn list_objects(&self) -> Vec<(NodeId, String, IVec3, bool, ModelId)> {
        self.collect_instances()
            .into_iter()
            .filter(|i| !i.hidden)
            .map(|i| {
                (
                    i.transform_id,
                    if i.name.is_empty() {
                        format!("object_{}", i.transform_id)
                    } else {
                        i.name.clone()
                    },
                    i.translation,
                    i.hidden,
                    i.model_id,
                )
            })
            .collect()
    }
}

fn compose_rotation(a: MvRotation, b: MvRotation) -> MvRotation {
    let ma = a.to_matrix();
    let mb = b.to_matrix();
    let mut m = [[0_i32; 3]; 3];
    for r in 0..3 {
        for c in 0..3 {
            m[r][c] = ma[r][0] * mb[0][c] + ma[r][1] * mb[1][c] + ma[r][2] * mb[2][c];
        }
    }
    MvRotation::from_matrix(m)
}

/// Transform local voxel coords → world (integer, MagicaVoxel style).
pub fn local_to_world(local: IVec3, translation: IVec3, rotation: MvRotation) -> IVec3 {
    let r = rotation.rotate_vec(local);
    IVec3::new(r.x + translation.x, r.y + translation.y, r.z + translation.z)
}

/// Inverse: world → local (for raycast hit mapping).
pub fn world_to_local(world: IVec3, translation: IVec3, rotation: MvRotation) -> IVec3 {
    let p = IVec3::new(
        world.x - translation.x,
        world.y - translation.y,
        world.z - translation.z,
    );
    // Inverse of orthonormal rotation = transpose
    let m = rotation.to_matrix();
    let mt = [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]];
    IVec3::new(
        mt[0][0] * p.x + mt[0][1] * p.y + mt[0][2] * p.z,
        mt[1][0] * p.x + mt[1][1] * p.y + mt[1][2] * p.z,
        mt[2][0] * p.x + mt[2][1] * p.y + mt[2][2] * p.z,
    )
}
