//! Minimal glTF 2.0 binary (.glb) writer with vertex colors + unlit material.

use crate::mesh::CpuMesh;
use crate::{ExportError, Result};
use serde_json::{json, Value};

pub fn write_glb(meshes: &[CpuMesh]) -> Result<Vec<u8>> {
    let meshes: Vec<&CpuMesh> = meshes.iter().filter(|m| !m.is_empty()).collect();
    if meshes.is_empty() {
        return Err(ExportError::Empty);
    }

    let mut bin = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut gltf_meshes = Vec::new();
    let mut child_nodes = Vec::new();

    for (i, mesh) in meshes.iter().enumerate() {
        let pos_acc = push_vec3(&mut bin, &mut buffer_views, &mut accessors, &mesh.positions, true);
        let nrm_acc = push_vec3(&mut bin, &mut buffer_views, &mut accessors, &mesh.normals, false);
        let col_acc = push_vec4(&mut bin, &mut buffer_views, &mut accessors, &mesh.colors);
        let idx_acc = push_indices(&mut bin, &mut buffer_views, &mut accessors, &mesh.indices);

        gltf_meshes.push(json!({
            "name": mesh.name,
            "primitives": [{
                "attributes": {
                    "POSITION": pos_acc,
                    "NORMAL": nrm_acc,
                    "COLOR_0": col_acc
                },
                "indices": idx_acc,
                "material": 0
            }]
        }));

        let node_index = i + 1;
        child_nodes.push(node_index);
    }

    let mut nodes = vec![json!({
        "name": "VoxelWorld",
        "children": child_nodes
    })];
    for (i, mesh) in meshes.iter().enumerate() {
        nodes.push(json!({
            "name": mesh.name,
            "mesh": i,
            "translation": mesh.translation
        }));
    }

    let root = json!({
        "asset": {
            "version": "2.0",
            "generator": "n2d98 Voxel Editor"
        },
        "extensionsUsed": ["KHR_materials_unlit"],
        "scene": 0,
        "scenes": [{ "name": "Scene", "nodes": [0] }],
        "nodes": nodes,
        "meshes": gltf_meshes,
        "materials": [{
            "name": "VoxelVertexColor",
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "metallicFactor": 0.0,
                "roughnessFactor": 1.0
            },
            "extensions": { "KHR_materials_unlit": {} }
        }],
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{ "byteLength": bin.len() }]
    });

    pack_glb(&root, &bin)
}

fn align4(buf: &mut Vec<u8>) -> usize {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
    buf.len()
}

fn push_vec3(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 3]],
    bounds: bool,
) -> usize {
    let byte_offset = align4(bin);
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in data {
        for i in 0..3 {
            min[i] = min[i].min(v[i]);
            max[i] = max[i].max(v[i]);
        }
        for c in v {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let byte_length = data.len() * 12;
    let view = views.len();
    views.push(json!({
        "buffer": 0,
        "byteOffset": byte_offset,
        "byteLength": byte_length,
        "target": 34962
    }));
    let acc = accessors.len();
    let mut obj = json!({
        "bufferView": view,
        "componentType": 5126,
        "count": data.len(),
        "type": "VEC3"
    });
    if bounds {
        obj["min"] = json!(min);
        obj["max"] = json!(max);
    }
    accessors.push(obj);
    acc
}

fn push_vec4(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 4]],
) -> usize {
    let byte_offset = align4(bin);
    for v in data {
        for c in v {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let view = views.len();
    views.push(json!({
        "buffer": 0,
        "byteOffset": byte_offset,
        "byteLength": data.len() * 16,
        "target": 34962
    }));
    let acc = accessors.len();
    accessors.push(json!({
        "bufferView": view,
        "componentType": 5126,
        "count": data.len(),
        "type": "VEC4"
    }));
    acc
}

fn push_indices(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[u32],
) -> usize {
    let byte_offset = align4(bin);
    for i in data {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    let view = views.len();
    views.push(json!({
        "buffer": 0,
        "byteOffset": byte_offset,
        "byteLength": data.len() * 4,
        "target": 34963
    }));
    let acc = accessors.len();
    accessors.push(json!({
        "bufferView": view,
        "componentType": 5125,
        "count": data.len(),
        "type": "SCALAR"
    }));
    acc
}

fn pack_glb(root: &Value, bin: &[u8]) -> Result<Vec<u8>> {
    let mut json_bytes = serde_json::to_vec(root).map_err(|e| ExportError::Msg(e.to_string()))?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let mut bin_bytes = bin.to_vec();
    while bin_bytes.len() % 4 != 0 {
        bin_bytes.push(0);
    }

    let json_len = json_bytes.len() as u32;
    let bin_len = bin_bytes.len() as u32;
    let total = 12 + 8 + json_len + 8 + bin_len;
    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&json_len.to_le_bytes());
    out.extend_from_slice(&0x4E4F_534A_u32.to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&bin_len.to_le_bytes());
    out.extend_from_slice(&0x004E_4942_u32.to_le_bytes());
    out.extend_from_slice(&bin_bytes);
    Ok(out)
}
