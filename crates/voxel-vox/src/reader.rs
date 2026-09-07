use crate::dict::{dict_get_i32, dict_get_ivec3, read_dict};
use crate::{default_palette_from_bytes, Result, VoxError};
use std::collections::HashMap;
use voxel_core::{
    GroupNode, IVec3, MvRotation, NodeId, Palette, Scene, SceneNode, ShapeNode, TransformNode,
    VoxelModel,
};

pub fn read_vox_scene(data: &[u8]) -> Result<(Scene, Palette)> {
    if data.len() < 8 {
        return Err(VoxError::BadMagic);
    }
    if &data[0..4] != b"VOX " {
        return Err(VoxError::BadMagic);
    }
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
    if version != 150 && version != 200 {
        return Err(VoxError::BadVersion(version));
    }

    let mut offset = 8;
    let main = read_chunk_header(data, &mut offset)?;
    if main.id != *b"MAIN" {
        return Err(VoxError::BadChunk("MAIN"));
    }
    let children_end = offset + main.children_size as usize;
    if children_end > data.len() {
        return Err(VoxError::BadChunk("MAIN"));
    }

    let mut sizes: Vec<(u32, u32, u32)> = Vec::new();
    let mut xyzi: Vec<Vec<(u8, u8, u8, u8)>> = Vec::new();
    let mut palette = Palette::magicavoxel_default();
    let mut nodes: HashMap<NodeId, SceneNode> = HashMap::new();
    let mut max_id = -1_i32;

    while offset < children_end {
        let chunk = read_chunk_header(data, &mut offset)?;
        let content = &data[offset..offset + chunk.content_size as usize];
        let after_children = offset + chunk.content_size as usize + chunk.children_size as usize;

        match &chunk.id {
            b"SIZE" => {
                if content.len() < 12 {
                    return Err(VoxError::BadChunk("SIZE"));
                }
                sizes.push((
                    u32::from_le_bytes(content[0..4].try_into().unwrap()),
                    u32::from_le_bytes(content[4..8].try_into().unwrap()),
                    u32::from_le_bytes(content[8..12].try_into().unwrap()),
                ));
            }
            b"XYZI" => {
                if content.len() < 4 {
                    return Err(VoxError::BadChunk("XYZI"));
                }
                let n = u32::from_le_bytes(content[0..4].try_into().unwrap()) as usize;
                if content.len() < 4 + n * 4 {
                    return Err(VoxError::BadChunk("XYZI"));
                }
                let mut voxels = Vec::with_capacity(n);
                for i in 0..n {
                    let o = 4 + i * 4;
                    voxels.push((content[o], content[o + 1], content[o + 2], content[o + 3]));
                }
                xyzi.push(voxels);
            }
            b"RGBA" => palette = default_palette_from_bytes(content)?,
            b"nTRN" => {
                let node = parse_ntrn(content)?;
                max_id = max_id.max(node.id);
                nodes.insert(node.id, SceneNode::Transform(node));
            }
            b"nGRP" => {
                let node = parse_ngrp(content)?;
                max_id = max_id.max(node.id);
                nodes.insert(node.id, SceneNode::Group(node));
            }
            b"nSHP" => {
                let node = parse_nshp(content)?;
                max_id = max_id.max(node.id);
                nodes.insert(node.id, SceneNode::Shape(node));
            }
            b"PACK" | b"MATL" | b"MATT" | b"LAYR" | b"rOBJ" | b"rCAM" | b"NOTE" | b"IMAP"
            | b"META" => {}
            _ => {}
        }
        offset = after_children;
    }

    if sizes.is_empty() || xyzi.is_empty() {
        return Err(VoxError::Msg("no SIZE/XYZI model in file".into()));
    }

    let count = sizes.len().min(xyzi.len());
    let mut models = Vec::with_capacity(count);
    for i in 0..count {
        let (sx, sy, sz) = sizes[i];
        models.push(VoxelModel::from_sparse(sx, sy, sz, xyzi[i].iter().copied())?);
    }

    let scene = if nodes.is_empty() {
        // Legacy file: one transform per model under a root group
        build_default_scene(models)
    } else {
        let root = find_root(&nodes).unwrap_or(0);
        Scene {
            models,
            nodes,
            root,
            next_id: max_id + 1,
        }
    };

    Ok((scene, palette))
}

fn build_default_scene(models: Vec<VoxelModel>) -> Scene {
    if models.len() == 1 {
        return Scene::from_single_model(models.into_iter().next().unwrap());
    }
    let mut scene = Scene {
        models,
        nodes: HashMap::new(),
        root: 0,
        next_id: 1,
    };
    let group_id = scene.alloc_id();
    let mut children = Vec::new();
    for mid in 0..scene.models.len() {
        let shape_id = scene.alloc_id();
        let trn_id = scene.alloc_id();
        scene.nodes.insert(
            shape_id,
            SceneNode::Shape(ShapeNode {
                id: shape_id,
                model_ids: vec![mid],
            }),
        );
        scene.nodes.insert(
            trn_id,
            SceneNode::Transform(TransformNode {
                id: trn_id,
                name: format!("model_{mid}"),
                hidden: false,
                child: shape_id,
                translation: IVec3::new((mid as i32) * 40, 0, 0),
                rotation: MvRotation::IDENTITY,
                layer_id: 0,
            }),
        );
        children.push(trn_id);
    }
    scene.nodes.insert(
        group_id,
        SceneNode::Group(GroupNode {
            id: group_id,
            name: "root".into(),
            children,
        }),
    );
    let root = 0;
    scene.nodes.insert(
        root,
        SceneNode::Transform(TransformNode {
            id: root,
            name: "".into(),
            hidden: false,
            child: group_id,
            translation: IVec3::new(0, 0, 0),
            rotation: MvRotation::IDENTITY,
            layer_id: -1,
        }),
    );
    scene.root = root;
    scene
}

fn find_root(nodes: &HashMap<NodeId, SceneNode>) -> Option<NodeId> {
    let mut referenced = std::collections::HashSet::new();
    for node in nodes.values() {
        match node {
            SceneNode::Transform(t) => {
                referenced.insert(t.child);
            }
            SceneNode::Group(g) => {
                for &c in &g.children {
                    referenced.insert(c);
                }
            }
            SceneNode::Shape(_) => {}
        }
    }
    nodes
        .keys()
        .copied()
        .find(|id| !referenced.contains(id))
}

fn parse_ntrn(content: &[u8]) -> Result<TransformNode> {
    let mut o = 0usize;
    if content.len() < 4 {
        return Err(VoxError::BadChunk("nTRN"));
    }
    let id = i32::from_le_bytes(content[o..o + 4].try_into().unwrap());
    o += 4;
    let attrs = read_dict(content, &mut o)?;
    let name = attrs.get("_name").cloned().unwrap_or_default();
    let hidden = attrs.get("_hidden").map(|v| v != "0").unwrap_or(false);
    if o + 16 > content.len() {
        return Err(VoxError::BadChunk("nTRN"));
    }
    let child = i32::from_le_bytes(content[o..o + 4].try_into().unwrap());
    o += 4;
    let _reserved = i32::from_le_bytes(content[o..o + 4].try_into().unwrap());
    o += 4;
    let layer_id = i32::from_le_bytes(content[o..o + 4].try_into().unwrap());
    o += 4;
    let n_frames = i32::from_le_bytes(content[o..o + 4].try_into().unwrap());
    o += 4;
    let mut translation = IVec3::new(0, 0, 0);
    let mut rotation = MvRotation::IDENTITY;
    for _ in 0..n_frames.max(0) {
        let frame = read_dict(content, &mut o)?;
        if let Some((x, y, z)) = dict_get_ivec3(&frame, "_t") {
            translation = IVec3::new(x, y, z);
        }
        if let Some(r) = dict_get_i32(&frame, "_r") {
            rotation = MvRotation(r as u8);
        }
    }
    Ok(TransformNode {
        id,
        name,
        hidden,
        child,
        translation,
        rotation,
        layer_id,
    })
}

fn parse_ngrp(content: &[u8]) -> Result<GroupNode> {
    let mut o = 0usize;
    if content.len() < 4 {
        return Err(VoxError::BadChunk("nGRP"));
    }
    let id = i32::from_le_bytes(content[o..o + 4].try_into().unwrap());
    o += 4;
    let attrs = read_dict(content, &mut o)?;
    let name = attrs.get("_name").cloned().unwrap_or_default();
    if o + 4 > content.len() {
        return Err(VoxError::BadChunk("nGRP"));
    }
    let n = i32::from_le_bytes(content[o..o + 4].try_into().unwrap()) as usize;
    o += 4;
    let mut children = Vec::with_capacity(n);
    for _ in 0..n {
        if o + 4 > content.len() {
            return Err(VoxError::BadChunk("nGRP"));
        }
        children.push(i32::from_le_bytes(content[o..o + 4].try_into().unwrap()));
        o += 4;
    }
    Ok(GroupNode { id, name, children })
}

fn parse_nshp(content: &[u8]) -> Result<ShapeNode> {
    let mut o = 0usize;
    if content.len() < 4 {
        return Err(VoxError::BadChunk("nSHP"));
    }
    let id = i32::from_le_bytes(content[o..o + 4].try_into().unwrap());
    o += 4;
    let _attrs = read_dict(content, &mut o)?;
    if o + 4 > content.len() {
        return Err(VoxError::BadChunk("nSHP"));
    }
    let n = i32::from_le_bytes(content[o..o + 4].try_into().unwrap()) as usize;
    o += 4;
    let mut model_ids = Vec::with_capacity(n);
    for _ in 0..n {
        if o + 4 > content.len() {
            return Err(VoxError::BadChunk("nSHP"));
        }
        let mid = i32::from_le_bytes(content[o..o + 4].try_into().unwrap()) as usize;
        o += 4;
        let _model_attrs = read_dict(content, &mut o)?;
        model_ids.push(mid);
    }
    Ok(ShapeNode { id, model_ids })
}

struct ChunkHeader {
    id: [u8; 4],
    content_size: u32,
    children_size: u32,
}

fn read_chunk_header(data: &[u8], offset: &mut usize) -> Result<ChunkHeader> {
    if *offset + 12 > data.len() {
        return Err(VoxError::BadChunk("header"));
    }
    let id = data[*offset..*offset + 4].try_into().unwrap();
    let content_size = u32::from_le_bytes(data[*offset + 4..*offset + 8].try_into().unwrap());
    let children_size = u32::from_le_bytes(data[*offset + 8..*offset + 12].try_into().unwrap());
    *offset += 12;
    if *offset + content_size as usize > data.len() {
        return Err(VoxError::BadChunk("content"));
    }
    Ok(ChunkHeader {
        id,
        content_size,
        children_size,
    })
}
