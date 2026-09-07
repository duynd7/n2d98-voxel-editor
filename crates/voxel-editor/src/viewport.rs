//! OpenGL (glow) mesh renderer for egui PaintCallback.

use crate::camera::OrbitCamera;
use crate::mesh::{MeshData, Vertex};
use glow::HasContext;
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct LineLayer {
    pub points: Vec<[f32; 3]>,
    pub color: [f32; 3],
}

#[derive(Clone, Default)]
pub struct OverlayFrame {
    /// Static-ish guides (bounds + grid). Uploaded with mesh when dirty.
    pub guides: Vec<LineLayer>,
    /// Per-frame cursor (wire cube). Always uploaded.
    pub cursor_lines: Option<LineLayer>,
    /// Translucent ghost cube mesh.
    pub ghost: Option<MeshData>,
    pub ghost_alpha: f32,
}

struct LineBuf {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    count: i32,
    color: [f32; 3],
}

struct ViewportGpu {
    program: glow::Program,
    line_program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    index_count: i32,
    guide_bufs: Vec<LineBuf>,
    cursor_buf: LineBuf,
    ghost_vao: glow::VertexArray,
    ghost_vbo: glow::Buffer,
    ghost_ebo: glow::Buffer,
    ghost_count: i32,
    u_mvp: glow::UniformLocation,
    u_light: glow::UniformLocation,
    u_line_mvp: glow::UniformLocation,
    u_line_color: glow::UniformLocation,
    u_alpha: Option<glow::UniformLocation>,
}

impl ViewportGpu {
    unsafe fn new(gl: &glow::Context) -> Result<Self, String> {
        let program = create_program(gl, SOLID_VS, SOLID_FS)?;
        let line_program = create_program(gl, LINE_VS, LINE_FS)?;

        let make_mesh_vao = |gl: &glow::Context| -> Result<(glow::VertexArray, glow::Buffer, glow::Buffer), String> {
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            let ebo = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
            let stride = std::mem::size_of::<Vertex>() as i32;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 3, glow::FLOAT, false, stride, 24);
            gl.bind_vertex_array(None);
            Ok((vao, vbo, ebo))
        };

        let (vao, vbo, ebo) = make_mesh_vao(gl)?;
        let (ghost_vao, ghost_vbo, ghost_ebo) = make_mesh_vao(gl)?;

        let make_line = |gl: &glow::Context| -> Result<LineBuf, String> {
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 12, 0);
            gl.bind_vertex_array(None);
            Ok(LineBuf {
                vao,
                vbo,
                count: 0,
                color: [1.0, 1.0, 1.0],
            })
        };

        let u_mvp = gl
            .get_uniform_location(program, "u_mvp")
            .ok_or_else(|| "u_mvp missing".to_string())?;
        let u_light = gl
            .get_uniform_location(program, "u_light")
            .ok_or_else(|| "u_light missing".to_string())?;
        let u_alpha = gl.get_uniform_location(program, "u_alpha");
        let u_line_mvp = gl
            .get_uniform_location(line_program, "u_mvp")
            .ok_or_else(|| "line u_mvp missing".to_string())?;
        let u_line_color = gl
            .get_uniform_location(line_program, "u_color")
            .ok_or_else(|| "line u_color missing".to_string())?;

        Ok(Self {
            program,
            line_program,
            vao,
            vbo,
            ebo,
            index_count: 0,
            guide_bufs: Vec::new(),
            cursor_buf: make_line(gl)?,
            ghost_vao,
            ghost_vbo,
            ghost_ebo,
            ghost_count: 0,
            u_mvp,
            u_light,
            u_line_mvp,
            u_line_color,
            u_alpha,
        })
    }

    unsafe fn upload_mesh(&mut self, gl: &glow::Context, mesh: &MeshData) {
        upload_indexed(gl, self.vao, self.vbo, self.ebo, mesh);
        self.index_count = mesh.indices.len() as i32;
    }

    unsafe fn upload_guides(&mut self, gl: &glow::Context, layers: &[LineLayer]) {
        // Recreate buffers to match layer count
        while self.guide_bufs.len() < layers.len() {
            let vao = gl.create_vertex_array().unwrap();
            let vbo = gl.create_buffer().unwrap();
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 12, 0);
            gl.bind_vertex_array(None);
            self.guide_bufs.push(LineBuf {
                vao,
                vbo,
                count: 0,
                color: [1.0; 3],
            });
        }
        for (buf, layer) in self.guide_bufs.iter_mut().zip(layers.iter()) {
            upload_lines(gl, buf, &layer.points, layer.color);
        }
        for buf in self.guide_bufs.iter_mut().skip(layers.len()) {
            buf.count = 0;
        }
    }

    unsafe fn upload_cursor(&mut self, gl: &glow::Context, layer: Option<&LineLayer>) {
        if let Some(layer) = layer {
            upload_lines(gl, &mut self.cursor_buf, &layer.points, layer.color);
        } else {
            self.cursor_buf.count = 0;
        }
    }

    unsafe fn upload_ghost(&mut self, gl: &glow::Context, mesh: Option<&MeshData>) {
        if let Some(mesh) = mesh {
            upload_indexed(gl, self.ghost_vao, self.ghost_vbo, self.ghost_ebo, mesh);
            self.ghost_count = mesh.indices.len() as i32;
        } else {
            self.ghost_count = 0;
        }
    }

    unsafe fn paint(
        &self,
        gl: &glow::Context,
        camera: &OrbitCamera,
        info: &egui::PaintCallbackInfo,
        ghost_alpha: f32,
    ) {
        let vp = info.viewport;
        let aspect = (vp.width() / vp.height().max(1.0)).max(0.01);
        let mvp = camera.view_proj(aspect);

        let depth_was = gl.is_enabled(glow::DEPTH_TEST);
        let cull_was = gl.is_enabled(glow::CULL_FACE);
        let blend_was = gl.is_enabled(glow::BLEND);

        gl.enable(glow::DEPTH_TEST);
        gl.depth_func(glow::LESS);
        gl.clear_depth_f32(1.0);
        gl.enable(glow::CULL_FACE);
        gl.cull_face(glow::BACK);
        gl.disable(glow::BLEND);

        gl.clear_color(0.09, 0.10, 0.12, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

        // Solids
        if self.index_count > 0 {
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(Some(&self.u_mvp), false, &mvp.to_cols_array());
            gl.uniform_3_f32(Some(&self.u_light), 0.35, -0.55, 0.75);
            if let Some(u) = &self.u_alpha {
                gl.uniform_1_f32(Some(u), 1.0);
            }
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_elements(glow::TRIANGLES, self.index_count, glow::UNSIGNED_INT, 0);
            gl.bind_vertex_array(None);
        }

        // Guides (grid / bounds / axes) — depth on
        gl.use_program(Some(self.line_program));
        gl.uniform_matrix_4_f32_slice(Some(&self.u_line_mvp), false, &mvp.to_cols_array());
        for buf in &self.guide_bufs {
            if buf.count <= 0 {
                continue;
            }
            gl.uniform_3_f32(Some(&self.u_line_color), buf.color[0], buf.color[1], buf.color[2]);
            gl.bind_vertex_array(Some(buf.vao));
            gl.draw_arrays(glow::LINES, 0, buf.count);
        }

        // Ghost cube (translucent)
        if self.ghost_count > 0 {
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.depth_mask(false);
            gl.disable(glow::CULL_FACE);
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(Some(&self.u_mvp), false, &mvp.to_cols_array());
            gl.uniform_3_f32(Some(&self.u_light), 0.2, -0.4, 0.9);
            if let Some(u) = &self.u_alpha {
                gl.uniform_1_f32(Some(u), ghost_alpha.clamp(0.05, 1.0));
            }
            gl.bind_vertex_array(Some(self.ghost_vao));
            gl.draw_elements(glow::TRIANGLES, self.ghost_count, glow::UNSIGNED_INT, 0);
            gl.bind_vertex_array(None);
            gl.depth_mask(true);
            gl.enable(glow::CULL_FACE);
            gl.disable(glow::BLEND);
        }

        // Cursor wire on top
        if self.cursor_buf.count > 0 {
            gl.disable(glow::DEPTH_TEST);
            gl.use_program(Some(self.line_program));
            gl.uniform_matrix_4_f32_slice(Some(&self.u_line_mvp), false, &mvp.to_cols_array());
            let c = self.cursor_buf.color;
            gl.uniform_3_f32(Some(&self.u_line_color), c[0], c[1], c[2]);
            gl.bind_vertex_array(Some(self.cursor_buf.vao));
            gl.draw_arrays(glow::LINES, 0, self.cursor_buf.count);
            gl.bind_vertex_array(None);
            gl.enable(glow::DEPTH_TEST);
        }

        gl.use_program(None);
        if !depth_was {
            gl.disable(glow::DEPTH_TEST);
        }
        if !cull_was {
            gl.disable(glow::CULL_FACE);
        }
        if blend_was {
            gl.enable(glow::BLEND);
        } else {
            gl.disable(glow::BLEND);
        }
    }

    unsafe fn destroy(&self, gl: &glow::Context) {
        gl.delete_program(self.program);
        gl.delete_program(self.line_program);
        gl.delete_vertex_array(self.vao);
        gl.delete_buffer(self.vbo);
        gl.delete_buffer(self.ebo);
        gl.delete_vertex_array(self.ghost_vao);
        gl.delete_buffer(self.ghost_vbo);
        gl.delete_buffer(self.ghost_ebo);
        gl.delete_vertex_array(self.cursor_buf.vao);
        gl.delete_buffer(self.cursor_buf.vbo);
        for buf in &self.guide_bufs {
            gl.delete_vertex_array(buf.vao);
            gl.delete_buffer(buf.vbo);
        }
    }
}

unsafe fn upload_indexed(
    gl: &glow::Context,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    mesh: &MeshData,
) {
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    gl.buffer_data_u8_slice(
        glow::ARRAY_BUFFER,
        Vertex::as_bytes(&mesh.vertices),
        glow::DYNAMIC_DRAW,
    );
    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
    let index_bytes = std::slice::from_raw_parts(
        mesh.indices.as_ptr() as *const u8,
        mesh.indices.len() * 4,
    );
    gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, index_bytes, glow::DYNAMIC_DRAW);
    gl.bind_vertex_array(None);
}

unsafe fn upload_lines(
    gl: &glow::Context,
    buf: &mut LineBuf,
    points: &[[f32; 3]],
    color: [f32; 3],
) {
    gl.bind_vertex_array(Some(buf.vao));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(buf.vbo));
    let bytes =
        std::slice::from_raw_parts(points.as_ptr() as *const u8, std::mem::size_of_val(points));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW);
    buf.count = points.len() as i32;
    buf.color = color;
    gl.bind_vertex_array(None);
}

pub struct Viewport3D {
    gpu: Arc<Mutex<Option<ViewportGpu>>>,
    mesh_dirty: bool,
}

impl Viewport3D {
    pub fn new() -> Self {
        Self {
            gpu: Arc::new(Mutex::new(None)),
            mesh_dirty: true,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.mesh_dirty = true;
    }

    pub fn paint_callback(
        &mut self,
        rect: egui::Rect,
        camera: OrbitCamera,
        mesh: &MeshData,
        overlay: &OverlayFrame,
        gl: Option<&Arc<glow::Context>>,
    ) -> egui::PaintCallback {
        if let Some(gl) = gl {
            let mut slot = self.gpu.lock();
            unsafe {
                if slot.is_none() {
                    match ViewportGpu::new(gl) {
                        Ok(gpu) => *slot = Some(gpu),
                        Err(e) => eprintln!("viewport gpu init failed: {e}"),
                    }
                }
                if let Some(gpu) = slot.as_mut() {
                    if self.mesh_dirty {
                        gpu.upload_mesh(gl, mesh);
                        gpu.upload_guides(gl, &overlay.guides);
                        self.mesh_dirty = false;
                    }
                    gpu.upload_cursor(gl, overlay.cursor_lines.as_ref());
                    gpu.upload_ghost(gl, overlay.ghost.as_ref());
                }
            }
        }

        let gpu = self.gpu.clone();
        let ghost_alpha = overlay.ghost_alpha;
        egui::PaintCallback {
            rect,
            callback: Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
                let gl = painter.gl();
                let guard = gpu.lock();
                if let Some(gpu) = guard.as_ref() {
                    unsafe {
                        gpu.paint(gl, &camera, &info, ghost_alpha);
                    }
                }
            })),
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        let mut slot = self.gpu.lock();
        if let Some(gpu) = slot.take() {
            unsafe {
                gpu.destroy(gl);
            }
        }
    }
}

unsafe fn create_program(
    gl: &glow::Context,
    vs_src: &str,
    fs_src: &str,
) -> Result<glow::Program, String> {
    let program = gl.create_program()?;
    let vs = compile_shader(gl, glow::VERTEX_SHADER, vs_src)?;
    let fs = compile_shader(gl, glow::FRAGMENT_SHADER, fs_src)?;
    gl.attach_shader(program, vs);
    gl.attach_shader(program, fs);
    gl.bind_attrib_location(program, 0, "a_pos");
    gl.bind_attrib_location(program, 1, "a_normal");
    gl.bind_attrib_location(program, 2, "a_color");
    gl.link_program(program);
    if !gl.get_program_link_status(program) {
        return Err(gl.get_program_info_log(program));
    }
    gl.detach_shader(program, vs);
    gl.detach_shader(program, fs);
    gl.delete_shader(vs);
    gl.delete_shader(fs);
    Ok(program)
}

unsafe fn compile_shader(
    gl: &glow::Context,
    kind: u32,
    src: &str,
) -> Result<glow::Shader, String> {
    let shader = gl.create_shader(kind)?;
    gl.shader_source(shader, src);
    gl.compile_shader(shader);
    if !gl.get_shader_compile_status(shader) {
        return Err(gl.get_shader_info_log(shader));
    }
    Ok(shader)
}

const SOLID_VS: &str = r#"#version 150
in vec3 a_pos;
in vec3 a_normal;
in vec3 a_color;
uniform mat4 u_mvp;
out vec3 v_normal;
out vec3 v_color;
void main() {
    v_normal = a_normal;
    v_color = a_color;
    gl_Position = u_mvp * vec4(a_pos, 1.0);
}
"#;

const SOLID_FS: &str = r#"#version 150
in vec3 v_normal;
in vec3 v_color;
uniform vec3 u_light;
uniform float u_alpha;
out vec4 out_color;
void main() {
    float ndl = max(dot(normalize(v_normal), normalize(u_light)), 0.0);
    float ambient = 0.28;
    float shade = ambient + (1.0 - ambient) * ndl;
    float a = u_alpha;
    if (a <= 0.0) a = 1.0;
    out_color = vec4(v_color * shade, a);
}
"#;

const LINE_VS: &str = r#"#version 150
in vec3 a_pos;
uniform mat4 u_mvp;
void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
}
"#;

const LINE_FS: &str = r#"#version 150
uniform vec3 u_color;
out vec4 out_color;
void main() {
    out_color = vec4(u_color, 1.0);
}
"#;
