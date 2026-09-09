use crate::dict::write_dict;
use crate::Result;
use voxel_core::{Layer, Material, Palette, Scene, SceneNode, VoxelModel};

/// Write MagicaVoxel-compatible .vox with models + scene graph.
pub fn write_vox_scene(scene: &Scene, palette: &Palette) -> Result<Vec<u8>> {
    let mut children = Vec::new();

    for model in &scene.models {
        children.extend_from_slice(&encode_model(model));
    }
    children.extend_from_slice(&encode_rgba(palette));

    // Scene graph chunks
    let mut ordered: Vec<_> = scene.nodes.keys().copied().collect();
    ordered.sort();
    for id in ordered {
        if let Some(node) = scene.nodes.get(&id) {
            children.extend_from_slice(&encode_node(node));
        }
    }

    for (id, mat) in scene.materials.iter_non_default() {
        children.extend_from_slice(&encode_matl(id, mat));
    }
    for layer in &scene.layers.layers {
        children.extend_from_slice(&encode_layr(layer));
    }

    let main = encode_chunk(b"MAIN", &[], &children);
    let mut out = Vec::with_capacity(8 + main.len());
    out.extend_from_slice(b"VOX ");
    out.extend_from_slice(&150u32.to_le_bytes());
    out.extend_from_slice(&main);
    Ok(out)
}

fn encode_model(model: &VoxelModel) -> Vec<u8> {
    let voxels: Vec<(u8, u8, u8, u8)> = model.iter_occupied().collect();
    let (sx, sy, sz) = model.size();
    let mut size_content = Vec::with_capacity(12);
    size_content.extend_from_slice(&sx.to_le_bytes());
    size_content.extend_from_slice(&sy.to_le_bytes());
    size_content.extend_from_slice(&sz.to_le_bytes());
    let mut xyzi_content = Vec::with_capacity(4 + voxels.len() * 4);
    xyzi_content.extend_from_slice(&(voxels.len() as u32).to_le_bytes());
    for (x, y, z, c) in &voxels {
        xyzi_content.extend_from_slice(&[*x, *y, *z, *c]);
    }
    let mut out = encode_chunk(b"SIZE", &size_content, &[]);
    out.extend_from_slice(&encode_chunk(b"XYZI", &xyzi_content, &[]));
    out
}

fn encode_rgba(palette: &Palette) -> Vec<u8> {
    let mut b = Vec::with_capacity(256 * 4);
    for i in 1..=255u8 {
        let c = palette.get(i);
        b.extend_from_slice(&[c.r, c.g, c.b, c.a]);
    }
    b.extend_from_slice(&[0, 0, 0, 0]);
    encode_chunk(b"RGBA", &b, &[])
}

fn encode_matl(id: u8, mat: &Material) -> Vec<u8> {
    let mut content = Vec::new();
    content.extend_from_slice(&(id as i32).to_le_bytes());
    let owned = mat.to_dict();
    let refs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    write_dict(&mut content, &refs);
    encode_chunk(b"MATL", &content, &[])
}

fn encode_layr(layer: &Layer) -> Vec<u8> {
    let mut content = Vec::new();
    content.extend_from_slice(&layer.id.to_le_bytes());
    let hidden = if layer.hidden { "1" } else { "0" };
    let color = format!("{} {} {}", layer.color[0], layer.color[1], layer.color[2]);
    let mut attrs: Vec<(&str, &str)> = Vec::new();
    if !layer.name.is_empty() {
        attrs.push(("_name", layer.name.as_str()));
    }
    attrs.push(("_hidden", hidden));
    attrs.push(("_color", color.as_str()));
    write_dict(&mut content, &attrs);
    content.extend_from_slice(&(-1_i32).to_le_bytes());
    encode_chunk(b"LAYR", &content, &[])
}

fn encode_node(node: &SceneNode) -> Vec<u8> {
    match node {
        SceneNode::Transform(t) => {
            let mut content = Vec::new();
            content.extend_from_slice(&t.id.to_le_bytes());
            let mut attrs: Vec<(&str, &str)> = Vec::new();
            let name;
            if !t.name.is_empty() {
                name = t.name.as_str();
                attrs.push(("_name", name));
            }
            let hidden;
            if t.hidden {
                hidden = "1";
                attrs.push(("_hidden", hidden));
            }
            write_dict(&mut content, &attrs);
            content.extend_from_slice(&t.child.to_le_bytes());
            content.extend_from_slice(&(-1_i32).to_le_bytes()); // reserved
            content.extend_from_slice(&t.layer_id.to_le_bytes());
            content.extend_from_slice(&1_i32.to_le_bytes()); // num frames
            let tstr = format!("{} {} {}", t.translation.x, t.translation.y, t.translation.z);
            let mut frame_owned: Vec<(String, String)> =
                vec![("_t".into(), tstr)];
            if t.rotation.0 != 0 {
                frame_owned.push(("_r".into(), format!("{}", t.rotation.0)));
            }
            let frame_refs: Vec<(&str, &str)> = frame_owned
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            write_dict(&mut content, &frame_refs);
            encode_chunk(b"nTRN", &content, &[])
        }
        SceneNode::Group(g) => {
            let mut content = Vec::new();
            content.extend_from_slice(&g.id.to_le_bytes());
            let attrs: Vec<(&str, &str)> = if g.name.is_empty() {
                vec![]
            } else {
                vec![("_name", g.name.as_str())]
            };
            write_dict(&mut content, &attrs);
            content.extend_from_slice(&(g.children.len() as i32).to_le_bytes());
            for &c in &g.children {
                content.extend_from_slice(&c.to_le_bytes());
            }
            encode_chunk(b"nGRP", &content, &[])
        }
        SceneNode::Shape(s) => {
            let mut content = Vec::new();
            content.extend_from_slice(&s.id.to_le_bytes());
            write_dict(&mut content, &[]);
            content.extend_from_slice(&(s.model_ids.len() as i32).to_le_bytes());
            for &mid in &s.model_ids {
                content.extend_from_slice(&(mid as i32).to_le_bytes());
                write_dict(&mut content, &[]);
            }
            encode_chunk(b"nSHP", &content, &[])
        }
    }
}

fn encode_chunk(id: &[u8; 4], content: &[u8], children: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + content.len() + children.len());
    out.extend_from_slice(id);
    out.extend_from_slice(&(content.len() as u32).to_le_bytes());
    out.extend_from_slice(&(children.len() as u32).to_le_bytes());
    out.extend_from_slice(content);
    out.extend_from_slice(children);
    out
}
