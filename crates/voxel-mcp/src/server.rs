use crate::params::*;
use parking_lot::Mutex;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use voxel_core::{BrushKind, ColorRgba, IVec3, MaterialKind, Project};
use voxel_vox::{load_path, save_path};

#[derive(Clone)]
pub struct VoxelMcpState {
    pub project: Project,
    pub path: Arc<Mutex<PathBuf>>,
}

impl VoxelMcpState {
    pub fn new(project: Project, path: PathBuf) -> Self {
        Self {
            project,
            path: Arc::new(Mutex::new(path)),
        }
    }

    fn persist(&self) -> Result<(), McpError> {
        let path = self.path.lock().clone();
        save_path(&self.project, &path).map_err(internal)
    }
}

#[derive(Clone)]
pub struct VoxelMcpServer {
    state: VoxelMcpState,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl VoxelMcpServer {
    pub fn new(state: VoxelMcpState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get current model size, voxel count, active color, and brush settings")]
    fn get_scene_info(&self) -> Result<CallToolResult, McpError> {
        let info = self.state.project.info_json();
        let path = self.state.path.lock().display().to_string();
        Ok(CallToolResult::success(vec![Content::text(format!(
            "path: {path}\n{info}"
        ))]))
    }

    #[tool(description = "List occupied voxels of the active model as JSON. Large models may be truncated.")]
    fn list_voxels(&self) -> Result<CallToolResult, McpError> {
        let snap = self.state.project.snapshot();
        let mut list = Vec::new();
        for (x, y, z, c) in snap.model().iter_occupied().take(8_000) {
            list.push(serde_json::json!({"x": x, "y": y, "z": z, "color": c}));
        }
        let truncated = snap.model().voxel_count() > list.len();
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "voxels": list,
                "truncated": truncated,
                "total": snap.model().voxel_count(),
                "active_model": snap.active_model,
            })
            .to_string(),
        )]))
    }

    #[tool(description = "Set or erase a single voxel. color=0 erases; 1..=255 paints with palette index.")]
    fn set_voxel(
        &self,
        Parameters(SetVoxelParams { x, y, z, color }): Parameters<SetVoxelParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        self.state
            .project
            .set_voxel(x, y, z, color)
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("set ({x},{y},{z}) = {color}")))
    }

    #[tool(description = "Erase a single voxel (same as set_voxel with color 0)")]
    fn erase_voxel(
        &self,
        Parameters(Vec3Params { x, y, z }): Parameters<Vec3Params>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        self.state
            .project
            .erase_voxel(x, y, z)
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("erased ({x},{y},{z})")))
    }

    #[tool(description = "Fill an axis-aligned box with a palette color (inclusive bounds)")]
    fn fill_box(
        &self,
        Parameters(p): Parameters<FillBoxParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        let n = self
            .state
            .project
            .fill_box(
                IVec3::new(p.min_x, p.min_y, p.min_z),
                IVec3::new(p.max_x, p.max_y, p.max_z),
                p.color,
            )
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("fill_box changed {n} voxels")))
    }

    #[tool(description = "Fill a sphere centered at (x,y,z) with given radius and palette color")]
    fn fill_sphere(
        &self,
        Parameters(p): Parameters<FillSphereParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        let n = self
            .state
            .project
            .fill_sphere(IVec3::new(p.x, p.y, p.z), p.radius, p.color)
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("fill_sphere changed {n} voxels")))
    }

    #[tool(description = "Draw a 3D Bresenham line between two voxels (inclusive endpoints)")]
    fn fill_line(
        &self,
        Parameters(p): Parameters<FillLineParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        let n = self
            .state
            .project
            .fill_line(
                IVec3::new(p.x0, p.y0, p.z0),
                IVec3::new(p.x1, p.y1, p.z1),
                p.color,
            )
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("fill_line changed {n} voxels")))
    }

    #[tool(description = "Fill a disk on a plane. axis is the plane normal: x|y|z")]
    fn fill_circle(
        &self,
        Parameters(p): Parameters<FillCircleParams>,
    ) -> Result<CallToolResult, McpError> {
        let axis = parse_axis(&p.axis)?;
        self.state.project.checkpoint();
        let n = self
            .state
            .project
            .fill_circle(IVec3::new(p.x, p.y, p.z), p.radius, axis, p.color)
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("fill_circle changed {n} voxels")))
    }

    #[tool(
        description = "Fill an axis-aligned rectangle on a constant-axis plane. Plane coord is a=(x0,y0,z0) on axis; the other two axes span a..=b. Example: axis=z fills x,y between the corners at z=z0."
    )]
    fn fill_plane(
        &self,
        Parameters(p): Parameters<FillPlaneParams>,
    ) -> Result<CallToolResult, McpError> {
        let axis = parse_axis(&p.axis)?;
        self.state.project.checkpoint();
        let n = self
            .state
            .project
            .fill_plane_rect(
                IVec3::new(p.x0, p.y0, p.z0),
                IVec3::new(p.x1, p.y1, p.z1),
                axis,
                p.color,
            )
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("fill_plane changed {n} voxels")))
    }

    #[tool(description = "Flood-fill connected region of same color starting at (x,y,z)")]
    fn flood_fill(
        &self,
        Parameters(p): Parameters<FloodFillParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        let n = self
            .state
            .project
            .flood_fill(IVec3::new(p.x, p.y, p.z), p.color)
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("flood_fill changed {n} voxels")))
    }

    #[tool(description = "Apply the current brush (kind/radius/mirror + active color) at a point")]
    fn apply_brush(
        &self,
        Parameters(p): Parameters<ApplyBrushParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        let n = self
            .state
            .project
            .apply_brush(IVec3::new(p.x, p.y, p.z))
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("apply_brush changed {n} voxels")))
    }

    #[tool(description = "Configure brush: kind=voxel|box|sphere|erase|flood_fill|line|plane|circle, optional mirror axes")]
    fn set_brush(
        &self,
        Parameters(p): Parameters<BrushParams>,
    ) -> Result<CallToolResult, McpError> {
        let kind = parse_brush_kind(&p.kind)?;
        self.state.project.set_brush(kind, p.radius, p.mirror());
        Ok(text(format!(
            "brush kind={} radius={} mirror=({},{},{})",
            p.kind, p.radius, p.mirror_x, p.mirror_y, p.mirror_z
        )))
    }

    #[tool(description = "Set active paint color (palette index 1..=255)")]
    fn set_active_color(
        &self,
        Parameters(ActiveColorParams { index }): Parameters<ActiveColorParams>,
    ) -> Result<CallToolResult, McpError> {
        if index == 0 {
            return Err(McpError::invalid_params("index must be 1..=255", None));
        }
        self.state.project.set_active_color(index);
        Ok(text(format!("active color = {index}")))
    }

    #[tool(description = "Set a palette RGBA color at index 1..=255")]
    fn set_palette_color(
        &self,
        Parameters(p): Parameters<PaletteColorParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        self.state
            .project
            .set_palette_color(p.index, ColorRgba::new(p.r, p.g, p.b, p.a))
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!(
            "palette[{}] = rgba({},{},{},{})",
            p.index, p.r, p.g, p.b, p.a
        )))
    }

    #[tool(description = "Get palette color RGBA at index")]
    fn get_palette_color(
        &self,
        Parameters(ActiveColorParams { index }): Parameters<ActiveColorParams>,
    ) -> Result<CallToolResult, McpError> {
        let c = self.state.project.with_inner(|p| p.palette.get(index));
        Ok(text(format!(
            "palette[{index}] = rgba({},{},{},{})",
            c.r, c.g, c.b, c.a
        )))
    }

    #[tool(description = "Get MagicaVoxel MATL material for a palette index (1..=255)")]
    fn get_material(
        &self,
        Parameters(ActiveColorParams { index }): Parameters<ActiveColorParams>,
    ) -> Result<CallToolResult, McpError> {
        if index == 0 {
            return Err(McpError::invalid_params("index must be 1..=255", None));
        }
        let json = self
            .state
            .project
            .with_inner(|p| p.scene.materials.get(index).to_json(index));
        Ok(text(json.to_string()))
    }

    #[tool(
        description = "Set MATL material on a palette index. type: diffuse|metal|glass|emit|blend|media. Optional weight/rough/spec/ior/att/flux (0..=1 except flux 0..=4)."
    )]
    fn set_material(
        &self,
        Parameters(p): Parameters<SetMaterialParams>,
    ) -> Result<CallToolResult, McpError> {
        if p.index == 0 {
            return Err(McpError::invalid_params("index must be 1..=255", None));
        }
        let kind = parse_material_kind(&p.kind)?;
        let mut mat = self
            .state
            .project
            .with_inner(|proj| proj.scene.materials.get(p.index).clone());
        mat.kind = kind;
        if let Some(v) = p.weight {
            mat.weight = v.clamp(0.0, 1.0);
        }
        if let Some(v) = p.rough {
            mat.rough = v.clamp(0.0, 1.0);
        }
        if let Some(v) = p.spec {
            mat.spec = v.clamp(0.0, 1.0);
        }
        if let Some(v) = p.ior {
            mat.ior = v.clamp(0.0, 2.0);
        }
        if let Some(v) = p.att {
            mat.att = v.clamp(0.0, 1.0);
        }
        if let Some(v) = p.flux {
            mat.flux = v.clamp(0.0, 4.0);
        }
        self.state.project.checkpoint();
        self.state
            .project
            .set_material(p.index, mat)
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!(
            "material[{}] = {}",
            p.index,
            kind.label().to_ascii_lowercase()
        )))
    }

    #[tool(description = "List world LAYR layers: id, name, hidden, color")]
    fn list_layers(&self) -> Result<CallToolResult, McpError> {
        let json = self
            .state
            .project
            .with_inner(|p| p.scene.layers.to_json());
        Ok(text(json.to_string()))
    }

    #[tool(description = "Rename a layer and/or set hidden (hides all objects on that layer)")]
    fn set_layer(
        &self,
        Parameters(p): Parameters<LayerUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        let ok = self.state.project.with_inner_mut(|proj| {
            let mut any = false;
            if let Some(name) = p.name {
                any |= proj.scene.layers.rename(p.layer_id, name);
            }
            if let Some(hidden) = p.hidden {
                any |= proj.scene.layers.set_hidden(p.layer_id, hidden);
            }
            any || proj.scene.layers.get(p.layer_id).is_some()
        });
        if !ok {
            return Err(McpError::invalid_params("unknown layer_id", None));
        }
        self.state.persist()?;
        Ok(text(format!("updated layer {}", p.layer_id)))
    }

    #[tool(description = "Assign a world object (transform node) to a LAYR layer id")]
    fn set_object_layer(
        &self,
        Parameters(p): Parameters<ObjectLayerParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        if !self.state.project.set_object_layer(p.node_id, p.layer_id) {
            return Err(McpError::invalid_params("node is not a transform", None));
        }
        self.state.persist()?;
        Ok(text(format!(
            "node {} → layer {}",
            p.node_id, p.layer_id
        )))
    }

    #[tool(description = "Clear all voxels in the model")]
    fn clear_model(&self) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        self.state.project.clear();
        self.state.persist()?;
        Ok(text("model cleared"))
    }

    #[tool(description = "Resize model (1..=256 per axis). Content is clipped/padded.")]
    fn resize_model(
        &self,
        Parameters(p): Parameters<ResizeParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        self.state
            .project
            .resize(p.size_x, p.size_y, p.size_z)
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!(
            "resized to {}x{}x{}",
            p.size_x, p.size_y, p.size_z
        )))
    }

    #[tool(description = "Mirror entire model on axis x, y, or z")]
    fn mirror_model(
        &self,
        Parameters(MirrorParams { axis }): Parameters<MirrorParams>,
    ) -> Result<CallToolResult, McpError> {
        let ch = axis
            .chars()
            .next()
            .ok_or_else(|| McpError::invalid_params("axis required", None))?;
        self.state.project.checkpoint();
        self.state.project.mirror(ch).map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("mirrored on {axis}")))
    }

    #[tool(description = "Save current project to a .vox path")]
    fn save_vox(
        &self,
        Parameters(PathParams { path }): Parameters<PathParams>,
    ) -> Result<CallToolResult, McpError> {
        let path = PathBuf::from(path);
        save_path(&self.state.project, &path).map_err(internal)?;
        *self.state.path.lock() = path.clone();
        Ok(text(format!("saved {}", path.display())))
    }

    #[tool(description = "Load a MagicaVoxel .vox file into the editor session")]
    fn load_vox(
        &self,
        Parameters(PathParams { path }): Parameters<PathParams>,
    ) -> Result<CallToolResult, McpError> {
        let path = PathBuf::from(path);
        let loaded = load_path(&path).map_err(internal)?;
        let snap = loaded.snapshot();
        self.state.project.checkpoint();
        self.state.project.with_inner_mut(|p| {
            *p = snap;
        });
        *self.state.path.lock() = path.clone();
        Ok(text(format!("loaded {}", path.display())))
    }

    #[tool(
        description = "Reload the bound .vox from disk into this MCP session. The GUI watches the same file and auto-reloads after generate/save — the user does not need to click Reload."
    )]
    fn reload(&self) -> Result<CallToolResult, McpError> {
        let path = self.state.path.lock().clone();
        let loaded = load_path(&path).map_err(internal)?;
        let snap = loaded.snapshot();
        self.state.project.checkpoint();
        self.state.project.with_inner_mut(|p| {
            *p = snap;
        });
        Ok(text(format!("reloaded {}", path.display())))
    }

    #[tool(description = "List world objects: id, name, translation, model_id")]
    fn list_objects(&self) -> Result<CallToolResult, McpError> {
        let info = self.state.project.info_json();
        Ok(text(info["objects"].to_string()))
    }

    #[tool(description = "Select a world object by transform node id (also sets active model)")]
    fn select_object(
        &self,
        Parameters(NodeIdParams { node_id }): Parameters<NodeIdParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.select_node(Some(node_id));
        Ok(text(format!("selected node {node_id}")))
    }

    #[tool(description = "Delete a world object (transform instance). Cannot delete the scene root.")]
    fn delete_object(
        &self,
        Parameters(NodeIdParams { node_id }): Parameters<NodeIdParams>,
    ) -> Result<CallToolResult, McpError> {
        if !self.state.project.delete_object(node_id) {
            return Err(McpError::invalid_params(
                "cannot delete node (missing or scene root)",
                None,
            ));
        }
        self.state.persist()?;
        Ok(text(format!("deleted node {node_id}")))
    }

    #[tool(description = "Undo the last checkpointed mutation")]
    fn undo(&self) -> Result<CallToolResult, McpError> {
        if !self.state.project.undo() {
            return Ok(text("nothing to undo"));
        }
        self.state.persist()?;
        Ok(text("undo"))
    }

    #[tool(description = "Redo the last undone mutation")]
    fn redo(&self) -> Result<CallToolResult, McpError> {
        if !self.state.project.redo() {
            return Ok(text("nothing to redo"));
        }
        self.state.persist()?;
        Ok(text("redo"))
    }

    #[tool(description = "Set active model index for voxel paint tools")]
    fn set_active_model(
        &self,
        Parameters(ModelIdParams { model_id }): Parameters<ModelIdParams>,
    ) -> Result<CallToolResult, McpError> {
        if !self.state.project.set_active_model(model_id as usize) {
            return Err(McpError::invalid_params("model_id out of range", None));
        }
        Ok(text(format!("active model = {model_id}")))
    }

    #[tool(description = "Set world edit mode: model | world")]
    fn set_edit_mode(
        &self,
        Parameters(EditModeParams { mode }): Parameters<EditModeParams>,
    ) -> Result<CallToolResult, McpError> {
        let m = match mode.to_ascii_lowercase().as_str() {
            "model" | "m" => voxel_core::EditMode::Model,
            "world" | "w" => voxel_core::EditMode::World,
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown mode '{other}'"),
                    None,
                ))
            }
        };
        self.state.project.set_edit_mode(m);
        Ok(text(format!("edit_mode = {mode}")))
    }

    #[tool(description = "Translate a world object (transform node) to x,y,z")]
    fn set_object_translation(
        &self,
        Parameters(p): Parameters<ObjectTranslationParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        if !self.state.project.set_object_translation(
            p.node_id,
            IVec3::new(p.x, p.y, p.z),
        ) {
            return Err(McpError::invalid_params("node is not a transform", None));
        }
        self.state.persist()?;
        Ok(text(format!(
            "node {} → ({},{},{})",
            p.node_id, p.x, p.y, p.z
        )))
    }

    #[tool(description = "Add a new empty object (default 32³) at translation")]
    fn add_object(
        &self,
        Parameters(p): Parameters<AddObjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        let size = if p.size == 0 { 32 } else { p.size };
        let id = self
            .state
            .project
            .add_object(&p.name, size, IVec3::new(p.x, p.y, p.z))
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("added object node_id={id}")))
    }

    #[tool(description = "Duplicate the active model as a new world object at translation")]
    fn duplicate_object(
        &self,
        Parameters(p): Parameters<AddObjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.state.project.checkpoint();
        let id = self
            .state
            .project
            .duplicate_active_object(&p.name, IVec3::new(p.x, p.y, p.z))
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("duplicated → node_id={id}")))
    }

    #[tool(
        description = "Export for Godot 4: writes a .glb (glTF 2.0, Y-up, vertex colors, unlit) plus a .tscn wrapper. 1 voxel = 1 Godot unit. MagicaVoxel Z-up is converted to Godot Y-up. scope: world (all objects, default) or model (active model only). Drop the .glb into a Godot project; if the .tscn is in a subfolder, fix its res:// path or instance the .glb directly."
    )]
    fn export_godot(
        &self,
        Parameters(p): Parameters<ExportGodotParams>,
    ) -> Result<CallToolResult, McpError> {
        let scope = voxel_export::ExportScope::parse(&p.scope).map_err(|e| {
            McpError::invalid_params(e.to_string(), None)
        })?;
        let out = voxel_export::export_godot(&self.state.project, &p.path, scope)
            .map_err(internal)?;
        Ok(text(
            serde_json::json!({
                "glb": out.glb_path.display().to_string(),
                "tscn": out.tscn_path.display().to_string(),
                "scope": match scope {
                    voxel_export::ExportScope::World => "world",
                    voxel_export::ExportScope::ActiveModel => "model",
                },
                "meshes": out.meshes,
                "vertices": out.vertices,
                "triangles": out.triangles,
                "note": "Godot 4: instance the .glb (vertex colors). 1 voxel = 1 unit. Z-up → Y-up."
            })
            .to_string(),
        ))
    }
}

#[tool_handler]
impl ServerHandler for VoxelMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "voxel-mcp-server".into(),
                title: Some("Voxel Editor MCP".into()),
                version: env!("CARGO_PKG_VERSION").into(),
                website_url: None,
                icons: None,
            },
            instructions: Some(
                "Voxel editor MCP (MagicaVoxel-compatible .vox) with Model + World modes. \
                 Coordinates: X right, Y depth, Z up. Colors are palette indices 1..=255. \
                 Use set_edit_mode/list_objects/add_object/delete_object/set_object_translation for the world editor. \
                 Use get_material/set_material for MATL (per palette index). \
                 Use list_layers/set_layer/set_object_layer for LAYR. \
                 Voxel paint tools (set_voxel, fill_*) operate on the active model and persist the .vox; \
                 the GUI auto-reloads that file. After generating, call export_godot to write a Godot 4 .glb. \
                 Mutating tools checkpoint once for undo/redo. Use delete_object to remove a world instance."
                    .into(),
            ),
        }
    }
}

fn parse_axis(s: &str) -> Result<char, McpError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "x" => Ok('x'),
        "y" => Ok('y'),
        "z" => Ok('z'),
        other => Err(McpError::invalid_params(
            format!("unknown axis '{other}' (expected x|y|z)"),
            None,
        )),
    }
}

fn parse_material_kind(s: &str) -> Result<MaterialKind, McpError> {
    match s.trim().trim_start_matches('_').to_ascii_lowercase().as_str() {
        "diffuse" | "d" => Ok(MaterialKind::Diffuse),
        "metal" | "m" => Ok(MaterialKind::Metal),
        "glass" | "g" => Ok(MaterialKind::Glass),
        "emit" | "emissive" | "e" => Ok(MaterialKind::Emit),
        "blend" => Ok(MaterialKind::Blend),
        "media" => Ok(MaterialKind::Media),
        other => Err(McpError::invalid_params(
            format!("unknown material type '{other}'"),
            None,
        )),
    }
}

fn parse_brush_kind(s: &str) -> Result<BrushKind, McpError> {
    match s.to_ascii_lowercase().as_str() {
        "voxel" | "v" => Ok(BrushKind::Voxel),
        "box" | "b" | "cube" => Ok(BrushKind::Box),
        "sphere" | "s" => Ok(BrushKind::Sphere),
        "erase" | "e" => Ok(BrushKind::Erase),
        "flood_fill" | "flood" | "f" => Ok(BrushKind::FloodFill),
        "line" | "l" => Ok(BrushKind::Line),
        "plane" | "p" => Ok(BrushKind::Plane),
        "circle" | "c" => Ok(BrushKind::Circle),
        other => Err(McpError::invalid_params(
            format!("unknown brush kind '{other}'"),
            None,
        )),
    }
}

fn text(s: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![Content::text(s.into())])
}

fn core_err(e: voxel_core::CoreError) -> McpError {
    McpError::invalid_params(e.to_string(), None)
}

fn internal(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

pub fn ensure_project_file(path: &Path, default_size: u32) -> anyhow::Result<Project> {
    if path.exists() {
        Ok(load_path(path)?)
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let project = Project::new(default_size)?;
        save_path(&project, path)?;
        Ok(project)
    }
}
