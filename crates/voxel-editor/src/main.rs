//! Cross-platform voxel editor — polished Model/World UI + draw UX.

mod camera;
mod mesh;
mod raycast;
mod theme;
mod viewport;

use camera::OrbitCamera;
use eframe::egui;
use glam::Vec2;
use std::path::PathBuf;
use viewport::{LineLayer, OverlayFrame, Viewport3D};
use voxel_core::{local_to_world, ColorIndex, EditMode, IVec3, Project};
use voxel_vox::{load_path, save_path};

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_title("Voxel Editor"),
        depth_buffer: 24,
        ..Default::default()
    };
    eframe::run_native(
        "Voxel Editor",
        native_options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(EditorApp::new(cc)))
        }),
    )
}

#[derive(Clone, Copy)]
struct HoverCell {
    /// Cell shown by the ghost (ray reach: solid face or far empty).
    cell: IVec3,
    erase: bool,
    from_solid: bool,
}

struct EditorApp {
    project: Project,
    path: PathBuf,
    slice_z: i32,
    status: String,
    dirty: bool,
    camera: OrbitCamera,
    viewport3d: Viewport3D,
    show_slice: bool,
    show_grid: bool,
    orbiting: bool,
    tx: i32,
    ty: i32,
    tz: i32,
    new_obj_name: String,
    hover: Option<HoverCell>,
    last_paint_cell: Option<IVec3>,
    theme_ready: bool,
    prev_show_grid: bool,
}

impl EditorApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let path = PathBuf::from(
            std::env::var("VOXEL_PROJECT").unwrap_or_else(|_| "project.vox".into()),
        );
        let (project, status) = if path.exists() {
            match load_path(&path) {
                Ok(p) => (p, format!("Loaded {}", path.display())),
                Err(e) => (
                    Project::new(32).expect("default project"),
                    format!("Failed to load {}: {e}", path.display()),
                ),
            }
        } else {
            let p = Project::new(32).expect("default project");
            let _ = p.fill_box(IVec3::new(12, 12, 0), IVec3::new(19, 19, 7), 1);
            let _ = p.fill_sphere(IVec3::new(16, 16, 12), 4, 8);
            let _ = p.duplicate_active_object("prop_b", IVec3::new(40, 0, 0));
            let _ = p.set_active_model(0);
            let _ = save_path(&p, &path);
            (p, format!("Created {}", path.display()))
        };

        let (sx, sy, sz) = project.with_inner(|p| p.model().size());
        let (tx, ty, tz) = project.with_inner(|p| {
            p.selected_node
                .and_then(|id| {
                    p.scene.nodes.get(&id).and_then(|n| match n {
                        voxel_core::SceneNode::Transform(t) => {
                            Some((t.translation.x, t.translation.y, t.translation.z))
                        }
                        _ => None,
                    })
                })
                .unwrap_or((0, 0, 0))
        });

        Self {
            project,
            path,
            slice_z: (sz as i32) / 2,
            status,
            dirty: false,
            camera: OrbitCamera::for_volume(sx.max(64), sy.max(64), sz.max(32)),
            viewport3d: Viewport3D::new(),
            show_slice: false,
            show_grid: true,
            orbiting: false,
            tx,
            ty,
            tz,
            new_obj_name: "object".into(),
            hover: None,
            last_paint_cell: None,
            theme_ready: false,
            prev_show_grid: true,
        }
    }

    fn save(&mut self) {
        match save_path(&self.project, &self.path) {
            Ok(()) => {
                self.dirty = false;
                self.status = format!("Saved {}", self.path.display());
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("MagicaVoxel", &["vox"])
            .pick_file()
        {
            match load_path(&path) {
                Ok(p) => {
                    self.project = p;
                    self.path = path;
                    self.frame_camera();
                    self.sync_translation_fields();
                    self.viewport3d.mark_dirty();
                    self.dirty = false;
                    self.hover = None;
                    self.status = format!("Opened {}", self.path.display());
                }
                Err(e) => self.status = format!("Open failed: {e}"),
            }
        }
    }

    fn reload(&mut self) {
        match load_path(&self.path) {
            Ok(p) => {
                self.project = p;
                self.frame_camera();
                self.sync_translation_fields();
                self.viewport3d.mark_dirty();
                self.dirty = false;
                self.hover = None;
                self.status = format!("Reloaded {}", self.path.display());
            }
            Err(e) => self.status = format!("Reload failed: {e}"),
        }
    }

    fn frame_camera(&mut self) {
        let instances = self.project.instances();
        let snap = self.project.snapshot();
        let mut max_e = 32u32;
        for inst in &instances {
            if let Some(m) = snap.scene.model(inst.model_id) {
                let (sx, sy, sz) = m.size();
                max_e = max_e.max(sx).max(sy).max(sz);
                max_e = max_e
                    .max(inst.translation.x.unsigned_abs())
                    .max(inst.translation.y.unsigned_abs())
                    .max(inst.translation.z.unsigned_abs());
            }
        }
        self.camera = OrbitCamera::for_volume(max_e + 16, max_e + 16, max_e / 2 + 16);
        let (_, _, sz) = snap.model().size();
        self.slice_z = (sz as i32) / 2;
    }

    fn sync_translation_fields(&mut self) {
        let t = self.project.with_inner(|p| {
            p.selected_node.and_then(|id| {
                p.scene.nodes.get(&id).and_then(|n| match n {
                    voxel_core::SceneNode::Transform(t) => Some(t.translation),
                    _ => None,
                })
            })
        });
        if let Some(t) = t {
            self.tx = t.x;
            self.ty = t.y;
            self.tz = t.z;
        }
    }

    fn pick(
        &self,
        rect: egui::Rect,
        pos: egui::Pos2,
    ) -> Option<(raycast::Hit, IVec3, IVec3)> {
        let local = Vec2::new(pos.x - rect.min.x, pos.y - rect.min.y);
        let size = Vec2::new(rect.width(), rect.height());
        let (origin, dir) = self.camera.screen_ray(local, size);
        let max_dist = self.camera.distance * 4.0;
        let mode = self.project.with_inner(|p| p.edit_mode);

        let hit = if mode == EditMode::World {
            let snap = self.project.snapshot();
            let instances = snap.scene.collect_instances();
            raycast::raycast_world(&instances, &snap.scene.models, origin, dir, max_dist)?
        } else {
            self.project
                .with_inner(|p| raycast::raycast(p.model(), origin, dir, max_dist))?
        };

        // World-space cells for the surface the ray reaches (hit) and the paint neighbor (place).
        let (place_w, hit_w) = if mode == EditMode::World {
            let snap = self.project.snapshot();
            let instances = snap.scene.collect_instances();
            let inst = instances.iter().find(|i| {
                Some(i.transform_id) == hit.transform_id && Some(i.model_id) == hit.model_id
            });
            if let Some(inst) = inst {
                (
                    local_to_world(
                        IVec3::new(hit.place_x, hit.place_y, hit.place_z),
                        inst.translation,
                        inst.rotation,
                    ),
                    local_to_world(
                        IVec3::new(hit.x, hit.y, hit.z),
                        inst.translation,
                        inst.rotation,
                    ),
                )
            } else {
                (
                    IVec3::new(hit.place_x, hit.place_y, hit.place_z),
                    IVec3::new(hit.x, hit.y, hit.z),
                )
            }
        } else {
            (
                IVec3::new(hit.place_x, hit.place_y, hit.place_z),
                IVec3::new(hit.x, hit.y, hit.z),
            )
        };

        Some((hit, place_w, hit_w))
    }

    fn update_hover(&mut self, rect: egui::Rect, pos: Option<egui::Pos2>, erase: bool) {
        let Some(pos) = pos else {
            self.hover = None;
            return;
        };
        if let Some((hit, place_w, hit_w)) = self.pick(rect, pos) {
            // Ghost tracks where the ray reaches:
            // - solid: the face voxel (farther than the empty place neighbor)
            // - empty volume: far cell inside the bounds
            // Erase highlights the solid; paint preview uses place (adjacent) when from solid.
            let cell = if erase {
                hit_w
            } else if hit.from_solid {
                place_w
            } else {
                hit_w // far empty cell
            };
            self.hover = Some(HoverCell {
                cell,
                erase,
                from_solid: hit.from_solid,
            });
        } else {
            self.hover = None;
        }
    }

    fn paint_at_pointer(&mut self, rect: egui::Rect, pos: egui::Pos2, erase: bool) {
        let Some((hit, place_w, hit_w)) = self.pick(rect, pos) else {
            return;
        };

        let cell = if erase { hit_w } else { place_w };
        // Snap: only apply once per cell while dragging
        if self.last_paint_cell == Some(cell) {
            return;
        }
        self.last_paint_cell = Some(cell);

        let mode = self.project.with_inner(|p| p.edit_mode);
        let active: ColorIndex = self.project.with_inner(|p| p.active_color);

        if mode == EditMode::World {
            if let Some(tid) = hit.transform_id {
                self.project.select_node(Some(tid));
                self.sync_translation_fields();
            }
            if let Some(mid) = hit.model_id {
                self.project.set_active_model(mid);
            }
        }

        if erase {
            if self.project.erase_voxel(hit.x, hit.y, hit.z).is_ok() {
                self.dirty = true;
                self.viewport3d.mark_dirty();
                self.status = format!("erase  {}, {}, {}", hit.x, hit.y, hit.z);
            }
        } else if self
            .project
            .set_voxel(hit.place_x, hit.place_y, hit.place_z, active)
            .is_ok()
        {
            self.dirty = true;
            self.viewport3d.mark_dirty();
            self.status = format!(
                "paint  {}, {}, {}  ·  color {active}",
                hit.place_x, hit.place_y, hit.place_z
            );
        }
    }

    fn build_draw_mesh(&self) -> mesh::MeshData {
        let snap = self.project.snapshot();
        match snap.edit_mode {
            EditMode::Model => mesh::build_mesh(snap.model(), &snap.palette),
            EditMode::World => {
                let instances = snap.scene.collect_instances();
                let refs: Vec<_> = instances
                    .iter()
                    .filter(|i| !i.hidden)
                    .filter_map(|i| {
                        snap.scene
                            .model(i.model_id)
                            .map(|m| (m, i.translation, i.rotation))
                    })
                    .collect();
                mesh::build_world_mesh(&refs, &snap.palette)
            }
        }
    }

    fn build_overlay(&self) -> OverlayFrame {
        let snap = self.project.snapshot();
        let mut guides = Vec::new();

        let (sx, sy, sz) = match snap.edit_mode {
            EditMode::Model => snap.model().size(),
            EditMode::World => {
                // Cover active model or union of instances on ground
                let mut sx = 32u32;
                let mut sy = 32u32;
                let mut sz = 32u32;
                for inst in snap.scene.collect_instances() {
                    if let Some(m) = snap.scene.model(inst.model_id) {
                        let (a, b, c) = m.size();
                        sx = sx.max(a + inst.translation.x.max(0) as u32);
                        sy = sy.max(b + inst.translation.y.max(0) as u32);
                        sz = sz.max(c + inst.translation.z.max(0) as u32);
                    }
                }
                (sx, sy, sz)
            }
        };

        // Bounds
        guides.push(LineLayer {
            points: mesh::build_bounds_lines(sx, sy, sz),
            color: [0.45, 0.50, 0.55],
        });

        if self.show_grid {
            let (minor, major) = mesh::build_ground_grid(sx, sy, 0.0, 8);
            guides.push(LineLayer {
                points: minor,
                color: [0.18, 0.20, 0.24],
            });
            guides.push(LineLayer {
                points: major,
                color: [0.28, 0.32, 0.38],
            });
        }

        // Axes
        for (a, b, col) in mesh::build_axis_gizmo(8.0) {
            guides.push(LineLayer {
                points: vec![a, b],
                color: col,
            });
        }

        let (cursor_lines, ghost, ghost_alpha) = if let Some(h) = self.hover {
            let cell = h.cell;
            let wire_color = if h.erase {
                [0.95, 0.35, 0.35]
            } else {
                [0.25, 0.95, 0.85]
            };
            let fill = if h.erase {
                [0.9, 0.25, 0.25]
            } else {
                let c = snap.palette.get(snap.active_color);
                [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                ]
            };
            (
                Some(LineLayer {
                    points: mesh::build_wire_cube(cell.x, cell.y, cell.z, 0.02),
                    color: wire_color,
                }),
                Some(mesh::build_ghost_cube_mesh(cell.x, cell.y, cell.z, fill, 0.04)),
                if h.erase { 0.28 } else { 0.38 },
            )
        } else {
            (None, None, 0.0)
        };

        OverlayFrame {
            guides,
            cursor_lines,
            ghost,
            ghost_alpha,
        }
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.theme_ready {
            theme::apply(ctx);
            self.theme_ready = true;
        }

        // ── Top bar ──────────────────────────────────────────────
        egui::TopBottomPanel::top("top")
            .exact_height(44.0)
            .frame(egui::Frame::new().fill(theme::PANEL_BG).inner_margin(egui::Margin::symmetric(12, 8)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        egui::RichText::new("VOXEL")
                            .strong()
                            .color(theme::ACCENT)
                            .size(15.0),
                    );
                    ui.add_space(12.0);

                    let file_btn = |ui: &mut egui::Ui, label: &str| {
                        ui.add(
                            egui::Button::new(label)
                                .fill(egui::Color32::from_rgb(40, 44, 52))
                                .min_size(egui::vec2(64.0, 26.0)),
                        )
                    };
                    if file_btn(ui, "Open").clicked() {
                        self.open_dialog();
                    }
                    if file_btn(ui, "Save").clicked() {
                        self.save();
                    }
                    if file_btn(ui, "Reload").clicked() {
                        self.reload();
                    }

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    let mut mode = self.project.with_inner(|p| p.edit_mode);
                    mode_toggle(ui, &mut mode);
                    if mode != self.project.with_inner(|p| p.edit_mode) {
                        self.project.set_edit_mode(mode);
                        self.viewport3d.mark_dirty();
                        self.hover = None;
                        self.status = match mode {
                            EditMode::Model => "Model editor".into(),
                            EditMode::World => "World editor".into(),
                        };
                    }

                    ui.add_space(12.0);
                    ui.checkbox(&mut self.show_grid, "Grid");
                    if ui
                        .add(egui::Checkbox::new(&mut self.show_slice, "Slice"))
                        .changed()
                    {
                        // no-op
                    }
                    if ui
                        .add(
                            egui::Button::new("Frame")
                                .fill(egui::Color32::from_rgb(40, 44, 52)),
                        )
                        .clicked()
                    {
                        self.frame_camera();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let name = self
                            .path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("project.vox");
                        ui.label(
                            egui::RichText::new(format!(
                                "{}{}",
                                name,
                                if self.dirty { "  ·  unsaved" } else { "" }
                            ))
                            .color(theme::MUTED)
                            .size(12.5),
                        );
                    });
                });
            });

        // ── Status ───────────────────────────────────────────────
        egui::TopBottomPanel::bottom("status")
            .exact_height(28.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(18, 20, 24))
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&self.status).size(12.5));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(
                                "LMB paint  ·  Shift erase  ·  RMB orbit  ·  MMB pan  ·  scroll zoom",
                            )
                            .color(theme::MUTED)
                            .size(11.5),
                        );
                    });
                });
            });

        // ── Left tool panel ──────────────────────────────────────
        egui::SidePanel::left("tools")
            .default_width(248.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL_BG)
                    .inner_margin(egui::Margin::same(12))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(40, 44, 52))),
            )
            .show(ctx, |ui| {
                let mode = self.project.with_inner(|p| p.edit_mode);
                section_label(ui, "PALETTE");
                let active = self.project.with_inner(|p| p.active_color);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("#{active}"))
                            .color(theme::ACCENT)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "model {}",
                            self.project.with_inner(|p| p.active_model)
                        ))
                        .color(theme::MUTED)
                        .size(12.0),
                    );
                });
                ui.add_space(6.0);

                let colors: Vec<(u8, [f32; 4])> = self.project.with_inner(|p| {
                    (1u8..=255)
                        .map(|i| (i, p.palette.get(i).to_egui_rgba()))
                        .collect()
                });
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                            for (i, rgba) in colors {
                                let color = egui::Color32::from_rgba_unmultiplied(
                                    (rgba[0] * 255.0) as u8,
                                    (rgba[1] * 255.0) as u8,
                                    (rgba[2] * 255.0) as u8,
                                    255,
                                );
                                let (rect, response) = ui
                                    .allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::click());
                                ui.painter().rect_filled(rect, 3.0, color);
                                if i == active {
                                    ui.painter().rect_stroke(
                                        rect,
                                        3.0,
                                        egui::Stroke::new(1.5_f32, theme::ACCENT),
                                        egui::StrokeKind::Outside,
                                    );
                                }
                                if response.clicked() {
                                    self.project.set_active_color(i);
                                }
                                response.on_hover_text(format!("Palette {i}"));
                            }
                        });
                    });

                ui.add_space(10.0);
                if ui
                    .add(
                        egui::Button::new("Clear model")
                            .fill(egui::Color32::from_rgb(60, 36, 36))
                            .min_size(egui::vec2(ui.available_width(), 28.0)),
                    )
                    .clicked()
                {
                    self.project.clear();
                    self.dirty = true;
                    self.viewport3d.mark_dirty();
                }

                if mode == EditMode::World {
                    ui.add_space(14.0);
                    section_label(ui, "OBJECTS");
                    let objects = self.project.with_inner(|p| p.scene.list_objects());
                    let selected = self.project.with_inner(|p| p.selected_node);
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for (id, name, t, hidden, mid) in &objects {
                                let selected_here = selected == Some(*id);
                                let label = format!("{name}  ·  m{mid}");
                                let resp = ui.selectable_label(selected_here, &label);
                                if resp.clicked() {
                                    self.project.select_node(Some(*id));
                                    self.sync_translation_fields();
                                    self.status = format!("selected {name}");
                                }
                                resp.on_hover_text(format!(
                                    "id {id}  t({},{},{}){}",
                                    t.x,
                                    t.y,
                                    t.z,
                                    if *hidden { " hidden" } else { "" }
                                ));
                            }
                        });

                    ui.add_space(8.0);
                    section_label(ui, "TRANSFORM");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.tx).prefix("X "));
                        ui.add(egui::DragValue::new(&mut self.ty).prefix("Y "));
                        ui.add(egui::DragValue::new(&mut self.tz).prefix("Z "));
                    });
                    if ui
                        .add(
                            egui::Button::new("Apply translation")
                                .fill(theme::ACCENT_DIM)
                                .min_size(egui::vec2(ui.available_width(), 28.0)),
                        )
                        .clicked()
                    {
                        if let Some(id) = self.project.with_inner(|p| p.selected_node) {
                            self.project
                                .set_object_translation(id, IVec3::new(self.tx, self.ty, self.tz));
                            self.dirty = true;
                            self.viewport3d.mark_dirty();
                        }
                    }

                    ui.add_space(8.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_obj_name)
                            .hint_text("object name")
                            .desired_width(ui.available_width()),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Add 32³").clicked() {
                            let name = self.new_obj_name.clone();
                            if let Ok(id) = self.project.add_object(
                                &name,
                                32,
                                IVec3::new(self.tx + 40, self.ty, self.tz),
                            ) {
                                self.sync_translation_fields();
                                self.dirty = true;
                                self.viewport3d.mark_dirty();
                                self.status = format!("added #{id}");
                            }
                        }
                        if ui.button("Duplicate").clicked() {
                            let name = format!("{}_copy", self.new_obj_name);
                            if let Ok(id) = self.project.duplicate_active_object(
                                &name,
                                IVec3::new(self.tx + 40, self.ty, self.tz),
                            ) {
                                self.sync_translation_fields();
                                self.dirty = true;
                                self.viewport3d.mark_dirty();
                                self.status = format!("duplicated #{id}");
                            }
                        }
                    });
                }
            });

        if self.show_slice {
            egui::SidePanel::right("slice")
                .default_width(280.0)
                .frame(
                    egui::Frame::new()
                        .fill(theme::PANEL_BG)
                        .inner_margin(egui::Margin::same(10)),
                )
                .show(ctx, |ui| {
                    section_label(ui, "Z SLICE");
                    let (sx, sy, sz) = self.project.with_inner(|p| p.model().size());
                    ui.add(egui::Slider::new(&mut self.slice_z, 0..=(sz as i32 - 1)).text("Z"));
                    let cell = 8.0_f32;
                    let active: ColorIndex = self.project.with_inner(|p| p.active_color);
                    let desired = egui::vec2(sx as f32 * cell, sy as f32 * cell);
                    let (rect, response) =
                        ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
                    {
                        let snap = self.project.snapshot();
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(16, 18, 21));
                        for y in 0..sy as i32 {
                            for x in 0..sx as i32 {
                                let c = snap.model().get_or_empty(x, y, self.slice_z);
                                if c == 0 {
                                    continue;
                                }
                                let rgba = snap.palette.get(c).to_egui_rgba();
                                let color = egui::Color32::from_rgba_unmultiplied(
                                    (rgba[0] * 255.0) as u8,
                                    (rgba[1] * 255.0) as u8,
                                    (rgba[2] * 255.0) as u8,
                                    255,
                                );
                                let min = rect.min
                                    + egui::vec2(
                                        x as f32 * cell,
                                        (sy as i32 - 1 - y) as f32 * cell,
                                    );
                                painter.rect_filled(
                                    egui::Rect::from_min_size(
                                        min,
                                        egui::vec2(cell - 1.0, cell - 1.0),
                                    ),
                                    1.0,
                                    color,
                                );
                            }
                        }
                    }
                    if let Some(pos) = response.interact_pointer_pos() {
                        if response.dragged() || response.clicked() {
                            let local = pos - rect.min;
                            let x = (local.x / cell).floor() as i32;
                            let y = (sy as i32 - 1) - (local.y / cell).floor() as i32;
                            if x >= 0 && y >= 0 && x < sx as i32 && y < sy as i32 {
                                let color = if ui.input(|i| i.modifiers.shift) {
                                    0
                                } else {
                                    active
                                };
                                if self.project.set_voxel(x, y, self.slice_z, color).is_ok() {
                                    self.dirty = true;
                                    self.viewport3d.mark_dirty();
                                }
                            }
                        }
                    }
                });
        }

        // ── Viewport ─────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(14, 15, 17)))
            .show(ctx, |ui| {
                let snap = self.project.snapshot();
                let (sx, sy, sz) = snap.model().size();
                let count = snap.model().voxel_count();

                let available = ui.available_size();
                let (rect, response) =
                    ui.allocate_exact_size(available, egui::Sense::click_and_drag());

                let modifiers = ui.input(|i| i.modifiers);
                let erase = modifiers.shift;

                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll.abs() > 0.0 {
                        self.camera.zoom(scroll / 40.0);
                    }
                    self.update_hover(rect, response.hover_pos(), erase);
                    ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                } else {
                    self.hover = None;
                }

                let secondary = response.dragged_by(egui::PointerButton::Secondary)
                    || (response.dragged_by(egui::PointerButton::Primary) && modifiers.alt);
                let middle = response.dragged_by(egui::PointerButton::Middle)
                    || (response.dragged_by(egui::PointerButton::Primary) && modifiers.command);

                if secondary {
                    self.orbiting = true;
                    let delta = response.drag_delta();
                    self.camera.orbit(Vec2::new(delta.x, -delta.y));
                } else if middle {
                    self.orbiting = true;
                    let delta = response.drag_delta();
                    self.camera.pan(Vec2::new(delta.x, delta.y), rect.height());
                } else if response.dragged_by(egui::PointerButton::Primary) && !modifiers.alt {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.paint_at_pointer(rect, pos, erase);
                    }
                }

                if response.clicked_by(egui::PointerButton::Primary)
                    && !modifiers.alt
                    && !self.orbiting
                {
                    self.last_paint_cell = None;
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.paint_at_pointer(rect, pos, erase);
                    }
                }
                if response.drag_started() {
                    self.last_paint_cell = None;
                }
                if response.drag_stopped() {
                    self.orbiting = false;
                    self.last_paint_cell = None;
                }

                // Toggle grid → rebuild guides
                if self.prev_show_grid != self.show_grid {
                    self.prev_show_grid = self.show_grid;
                    self.viewport3d.mark_dirty();
                }

                let mesh_data = self.build_draw_mesh();
                let overlay = self.build_overlay();
                let callback = self.viewport3d.paint_callback(
                    rect,
                    self.camera.clone(),
                    &mesh_data,
                    &overlay,
                    frame.gl(),
                );
                ui.painter().add(callback);

                // HUD overlays
                let painter = ui.painter_at(rect);
                let hud = format!(
                    "{:?}  ·  {sx}×{sy}×{sz}  ·  {count} voxels  ·  models {}",
                    snap.edit_mode,
                    snap.scene.models.len()
                );
                painter.text(
                    rect.left_top() + egui::vec2(14.0, 12.0),
                    egui::Align2::LEFT_TOP,
                    hud,
                    egui::FontId::proportional(12.5),
                    egui::Color32::from_rgba_unmultiplied(200, 210, 220, 210),
                );

                if let Some(h) = self.hover {
                    let tag = if h.erase {
                        "ERASE"
                    } else if h.from_solid {
                        "PLACE"
                    } else {
                        "PLACE · far"
                    };
                    let col = if h.erase {
                        theme::DANGER
                    } else {
                        theme::ACCENT
                    };
                    let text = format!(
                        "{tag}  →  {}, {}, {}   (snap)",
                        h.cell.x, h.cell.y, h.cell.z
                    );
                    // badge background
                    let galley = painter.layout_no_wrap(
                        text.clone(),
                        egui::FontId::proportional(13.0),
                        col,
                    );
                    let pos = rect.left_bottom() + egui::vec2(14.0, -36.0);
                    let bg = egui::Rect::from_min_size(
                        pos - egui::vec2(8.0, 4.0),
                        galley.size() + egui::vec2(16.0, 10.0),
                    );
                    painter.rect_filled(bg, 6.0, egui::Color32::from_rgba_unmultiplied(12, 14, 18, 200));
                    painter.galley(pos, galley, col);
                }
            });
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let Some(gl) = gl {
            self.viewport3d.destroy(gl);
        }
    }
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .color(theme::MUTED)
            .strong(),
    );
    ui.add_space(4.0);
}

fn mode_toggle(ui: &mut egui::Ui, mode: &mut EditMode) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let model = *mode == EditMode::Model;
        let world = *mode == EditMode::World;
        if ui
            .add(
                egui::Button::new("Model")
                    .fill(if model {
                        theme::ACCENT_DIM
                    } else {
                        egui::Color32::from_rgb(40, 44, 52)
                    })
                    .selected(model),
            )
            .clicked()
        {
            *mode = EditMode::Model;
        }
        if ui
            .add(
                egui::Button::new("World")
                    .fill(if world {
                        theme::ACCENT_DIM
                    } else {
                        egui::Color32::from_rgb(40, 44, 52)
                    })
                    .selected(world),
            )
            .clicked()
        {
            *mode = EditMode::World;
        }
    });
}
