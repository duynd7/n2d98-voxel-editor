//! MagicaVoxel `LAYR` world-editor layers.

use serde::{Deserialize, Serialize};

/// MagicaVoxel ships 8 layers (ids 0..=7).
pub const DEFAULT_LAYER_COUNT: i32 = 8;

const DEFAULT_LAYER_COLORS: [[u8; 3]; 8] = [
    [220, 224, 230],
    [220, 90, 90],
    [90, 200, 110],
    [80, 140, 230],
    [230, 200, 70],
    [80, 200, 210],
    [200, 110, 210],
    [160, 160, 170],
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    pub id: i32,
    pub name: String,
    pub hidden: bool,
    /// Display swatch (`_color` as `r g b`).
    pub color: [u8; 3],
}

impl Layer {
    pub fn new(id: i32) -> Self {
        let color = if (0..8).contains(&id) {
            DEFAULT_LAYER_COLORS[id as usize]
        } else {
            [180, 180, 180]
        };
        Self {
            id,
            name: format!("{id}"),
            hidden: false,
            color,
        }
    }

    pub fn display_name(&self) -> String {
        if self.name.is_empty() {
            format!("Layer {}", self.id)
        } else {
            self.name.clone()
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "hidden": self.hidden,
            "color": self.color,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerTable {
    pub layers: Vec<Layer>,
}

impl Default for LayerTable {
    fn default() -> Self {
        Self::standard_eight()
    }
}

impl LayerTable {
    pub fn standard_eight() -> Self {
        Self {
            layers: (0..DEFAULT_LAYER_COUNT).map(Layer::new).collect(),
        }
    }

    pub fn from_layers(mut layers: Vec<Layer>) -> Self {
        if layers.is_empty() {
            return Self::standard_eight();
        }
        layers.sort_by_key(|l| l.id);
        Self { layers }
    }

    pub fn get(&self, id: i32) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn get_mut(&mut self, id: i32) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Root transforms use `layer_id == -1` (not hidden by a layer).
    pub fn is_hidden(&self, id: i32) -> bool {
        if id < 0 {
            return false;
        }
        self.get(id).map(|l| l.hidden).unwrap_or(false)
    }

    pub fn set_hidden(&mut self, id: i32, hidden: bool) -> bool {
        if let Some(layer) = self.get_mut(id) {
            layer.hidden = hidden;
            true
        } else {
            false
        }
    }

    pub fn rename(&mut self, id: i32, name: String) -> bool {
        if let Some(layer) = self.get_mut(id) {
            layer.name = name;
            true
        } else {
            false
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!(self.layers.iter().map(Layer::to_json).collect::<Vec<_>>())
    }
}
