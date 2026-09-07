use crate::params::*;
use parking_lot::Mutex;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use voxel_core::{BrushKind, ColorRgba, IVec3, Project};
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
        let n = self
            .state
            .project
            .fill_sphere(IVec3::new(p.x, p.y, p.z), p.radius, p.color)
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("fill_sphere changed {n} voxels")))
    }

    #[tool(description = "Flood-fill connected region of same color starting at (x,y,z)")]
    fn flood_fill(
        &self,
        Parameters(p): Parameters<FloodFillParams>,
    ) -> Result<CallToolResult, McpError> {
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
        let n = self
            .state
            .project
            .apply_brush(IVec3::new(p.x, p.y, p.z))
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("apply_brush changed {n} voxels")))
    }

    #[tool(description = "Configure brush: kind=voxel|box|sphere|erase|flood_fill, optional mirror axes")]
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

    #[tool(description = "Clear all voxels in the model")]
    fn clear_model(&self) -> Result<CallToolResult, McpError> {
        self.state.project.clear();
        self.state.persist()?;
        Ok(text("model cleared"))
    }

    #[tool(description = "Resize model (1..=256 per axis). Content is clipped/padded.")]
    fn resize_model(
        &self,
        Parameters(p): Parameters<ResizeParams>,
    ) -> Result<CallToolResult, McpError> {
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
        self.state.project.with_inner_mut(|p| {
            *p = snap;
        });
        *self.state.path.lock() = path.clone();
        Ok(text(format!("loaded {}", path.display())))
    }

    #[tool(description = "Reload the currently bound project file from disk")]
    fn reload(&self) -> Result<CallToolResult, McpError> {
        let path = self.state.path.lock().clone();
        let loaded = load_path(&path).map_err(internal)?;
        let snap = loaded.snapshot();
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
        let id = self
            .state
            .project
            .duplicate_active_object(&p.name, IVec3::new(p.x, p.y, p.z))
            .map_err(core_err)?;
        self.state.persist()?;
        Ok(text(format!("duplicated → node_id={id}")))
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
                 Use set_edit_mode/list_objects/add_object/set_object_translation for the world editor. \
                 Voxel paint tools (set_voxel, fill_*) operate on the active model."
                    .into(),
            ),
        }
    }
}

fn parse_brush_kind(s: &str) -> Result<BrushKind, McpError> {
    match s.to_ascii_lowercase().as_str() {
        "voxel" | "v" => Ok(BrushKind::Voxel),
        "box" | "b" | "cube" => Ok(BrushKind::Box),
        "sphere" | "s" => Ok(BrushKind::Sphere),
        "erase" | "e" => Ok(BrushKind::Erase),
        "flood_fill" | "flood" | "f" => Ok(BrushKind::FloodFill),
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
