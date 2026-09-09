//! MagicaVoxel `MATL` materials — one slot per palette index (1..=255).

use crate::{ColorRgba, CoreError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// MagicaVoxel 0.99 material types (`_type` in MATL).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MaterialKind {
    #[default]
    Diffuse,
    Metal,
    Glass,
    Emit,
    Blend,
    Media,
}

impl MaterialKind {
    pub fn from_vox(s: &str) -> Self {
        match s.trim().trim_start_matches('_').to_ascii_lowercase().as_str() {
            "metal" => Self::Metal,
            "glass" => Self::Glass,
            "emit" | "emissive" => Self::Emit,
            "blend" => Self::Blend,
            "media" => Self::Media,
            _ => Self::Diffuse,
        }
    }

    pub fn to_vox(self) -> &'static str {
        match self {
            Self::Diffuse => "_diffuse",
            Self::Metal => "_metal",
            Self::Glass => "_glass",
            Self::Emit => "_emit",
            Self::Blend => "_blend",
            Self::Media => "_media",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Diffuse => "Diffuse",
            Self::Metal => "Metal",
            Self::Glass => "Glass",
            Self::Emit => "Emit",
            Self::Blend => "Blend",
            Self::Media => "Media",
        }
    }
}

/// MagicaVoxel default MATL numbers (ogt_vox / MV 0.99).
pub const DEFAULT_WEIGHT: f32 = 1.0;
pub const DEFAULT_ROUGH: f32 = 0.1;
pub const DEFAULT_SPEC: f32 = 0.5;
pub const DEFAULT_IOR: f32 = 0.3;
pub const DEFAULT_ATT: f32 = 0.0;
pub const DEFAULT_FLUX: f32 = 0.0;

/// One palette-index material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub kind: MaterialKind,
    /// `_weight` 0..=1 — metalness / glass mix / emit intensity.
    pub weight: f32,
    pub rough: f32,
    pub spec: f32,
    pub ior: f32,
    pub att: f32,
    /// Emit power (`_flux`), typically 0..=4.
    pub flux: f32,
    /// Unknown MATL keys preserved for round-trip (`_plastic`, `_emit`, `_ldr`, …).
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            kind: MaterialKind::Diffuse,
            weight: DEFAULT_WEIGHT,
            rough: DEFAULT_ROUGH,
            spec: DEFAULT_SPEC,
            ior: DEFAULT_IOR,
            att: DEFAULT_ATT,
            flux: DEFAULT_FLUX,
            extra: BTreeMap::new(),
        }
    }
}

impl Material {
    pub fn is_default(&self) -> bool {
        self.kind == MaterialKind::Diffuse
            && (self.weight - DEFAULT_WEIGHT).abs() < 1e-5
            && (self.rough - DEFAULT_ROUGH).abs() < 1e-5
            && (self.spec - DEFAULT_SPEC).abs() < 1e-5
            && (self.ior - DEFAULT_IOR).abs() < 1e-5
            && self.att.abs() < 1e-5
            && self.flux.abs() < 1e-5
            && self.extra.is_empty()
    }

    pub fn from_dict(props: &HashMap<String, String>) -> Self {
        let mut mat = Material::default();
        if let Some(t) = props.get("_type") {
            mat.kind = MaterialKind::from_vox(t);
        }
        if let Some(v) = props.get("_weight").and_then(|s| s.parse().ok()) {
            mat.weight = v;
        }
        if let Some(v) = props.get("_rough").and_then(|s| s.parse().ok()) {
            mat.rough = v;
        }
        if let Some(v) = props.get("_spec").and_then(|s| s.parse().ok()) {
            mat.spec = v;
        }
        if let Some(v) = props.get("_ior").and_then(|s| s.parse().ok()) {
            mat.ior = v;
        }
        if let Some(v) = props.get("_att").and_then(|s| s.parse().ok()) {
            mat.att = v;
        }
        if let Some(v) = props.get("_flux").and_then(|s| s.parse().ok()) {
            mat.flux = v;
        }
        const KNOWN: &[&str] = &[
            "_type", "_weight", "_rough", "_spec", "_ior", "_att", "_flux",
        ];
        for (k, v) in props {
            if !KNOWN.contains(&k.as_str()) {
                mat.extra.insert(k.clone(), v.clone());
            }
        }
        mat
    }

    /// MATL dict pairs, omitting default-valued optional keys for diffuse.
    pub fn to_dict(&self) -> Vec<(String, String)> {
        let mut pairs = vec![("_type".into(), self.kind.to_vox().to_string())];
        let write_weight = self.kind != MaterialKind::Diffuse
            || (self.weight - DEFAULT_WEIGHT).abs() > 1e-5;
        if write_weight {
            pairs.push(("_weight".into(), fmt_f32(self.weight)));
        }
        let extras = self.kind != MaterialKind::Diffuse;
        if extras || (self.rough - DEFAULT_ROUGH).abs() > 1e-5 {
            pairs.push(("_rough".into(), fmt_f32(self.rough)));
        }
        if extras || (self.spec - DEFAULT_SPEC).abs() > 1e-5 {
            pairs.push(("_spec".into(), fmt_f32(self.spec)));
        }
        if self.kind == MaterialKind::Glass
            || self.kind == MaterialKind::Media
            || (self.ior - DEFAULT_IOR).abs() > 1e-5
        {
            pairs.push(("_ior".into(), fmt_f32(self.ior)));
        }
        if self.kind == MaterialKind::Glass
            || self.kind == MaterialKind::Media
            || self.att.abs() > 1e-5
        {
            pairs.push(("_att".into(), fmt_f32(self.att)));
        }
        if self.kind == MaterialKind::Emit || self.flux.abs() > 1e-5 {
            pairs.push(("_flux".into(), fmt_f32(self.flux)));
        }
        for (k, v) in &self.extra {
            if k != "_type" {
                pairs.push((k.clone(), v.clone()));
            }
        }
        pairs
    }

    pub fn to_json(&self, index: u8) -> serde_json::Value {
        serde_json::json!({
            "index": index,
            "type": self.kind.label().to_ascii_lowercase(),
            "weight": self.weight,
            "rough": self.rough,
            "spec": self.spec,
            "ior": self.ior,
            "att": self.att,
            "flux": self.flux,
            "extra": self.extra,
        })
    }

    /// Viewport / export RGB with a cheap emit boost (unlit preview).
    pub fn viewport_rgb(&self, albedo: ColorRgba) -> [f32; 3] {
        let mut r = albedo.r as f32 / 255.0;
        let mut g = albedo.g as f32 / 255.0;
        let mut b = albedo.b as f32 / 255.0;
        if self.kind == MaterialKind::Emit {
            let boost = 1.0 + self.weight.clamp(0.0, 1.0) * (1.0 + self.flux.max(0.0) * 0.35);
            r = (r * boost).min(1.0);
            g = (g * boost).min(1.0);
            b = (b * boost).min(1.0);
        }
        [r, g, b]
    }

    pub fn viewport_rgba(&self, albedo: ColorRgba) -> [f32; 4] {
        let [r, g, b] = self.viewport_rgb(albedo);
        let a = if self.is_transparent() {
            (1.0 - self.weight.clamp(0.0, 1.0) * 0.65) * (albedo.a as f32 / 255.0)
        } else {
            albedo.a as f32 / 255.0
        };
        [r, g, b, a]
    }

    /// Glass / media — viewport draws these after opaque with alpha blending.
    pub fn is_transparent(&self) -> bool {
        matches!(self.kind, MaterialKind::Glass | MaterialKind::Media)
    }

    /// Unlit add factor for the viewport (`albedo + albedo * emit`).
    pub fn emit_amount(&self) -> f32 {
        if self.kind == MaterialKind::Emit {
            self.weight.clamp(0.0, 1.0) * (1.0 + self.flux.max(0.0) * 0.35)
        } else {
            0.0
        }
    }

    /// Metalness for viewport Blinn specular (Metal / Blend).
    pub fn metal_amount(&self) -> f32 {
        match self.kind {
            MaterialKind::Metal | MaterialKind::Blend => self.weight.clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

fn fmt_f32(v: f32) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// 256-slot table; index 0 is unused (empty voxel).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialTable {
    slots: Vec<Material>,
}

impl Default for MaterialTable {
    fn default() -> Self {
        Self {
            slots: vec![Material::default(); 256],
        }
    }
}

impl MaterialTable {
    pub fn get(&self, index: u8) -> &Material {
        &self.slots[index as usize]
    }

    pub fn get_mut(&mut self, index: u8) -> &mut Material {
        &mut self.slots[index as usize]
    }

    pub fn set(&mut self, index: u8, material: Material) -> Result<()> {
        if index == 0 {
            return Err(CoreError::InvalidColor(0));
        }
        self.slots[index as usize] = material;
        Ok(())
    }

    /// Insert from a MATL/MATT id (1..=255). Ids 0 and 256+ are ignored.
    pub fn insert_id(&mut self, id: i32, material: Material) {
        if (1..=255).contains(&id) {
            self.slots[id as usize] = material;
        }
    }

    pub fn iter_non_default(&self) -> impl Iterator<Item = (u8, &Material)> {
        (1u8..=255).filter_map(|i| {
            let m = &self.slots[i as usize];
            if m.is_default() {
                None
            } else {
                Some((i, m))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorRgba;

    #[test]
    fn preview_helpers_do_not_change_dict_roundtrip() {
        let mut glass = Material::default();
        glass.kind = MaterialKind::Glass;
        glass.weight = 0.8;
        let dict: std::collections::HashMap<_, _> = glass.to_dict().into_iter().collect();
        let back = Material::from_dict(&dict);
        assert_eq!(back.kind, MaterialKind::Glass);
        assert!((back.weight - 0.8).abs() < 1e-5);
        assert!(glass.is_transparent());
        let a = ColorRgba::rgb(10, 20, 30);
        assert!((glass.viewport_rgba(a)[3] - (1.0 - 0.8 * 0.65)).abs() < 1e-5);

        let mut emit = Material::default();
        emit.kind = MaterialKind::Emit;
        emit.weight = 1.0;
        emit.flux = 2.0;
        assert!(!emit.is_transparent());
        assert!((emit.emit_amount() - (1.0 + 2.0 * 0.35)).abs() < 1e-5);
        let boosted = emit.viewport_rgb(a);
        assert!(boosted[0] > a.r as f32 / 255.0);

        let mut metal = Material::default();
        metal.kind = MaterialKind::Metal;
        metal.weight = 0.5;
        assert!((metal.metal_amount() - 0.5).abs() < 1e-5);
        assert!(!metal.is_transparent());
    }
}
