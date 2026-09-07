# Voxel Editor (Rust) + MagicaVoxel-compatible MCP

Cross-platform voxel editor inspired by [MagicaVoxel 0.99.7](https://github.com/ephtracy/ephtracy.github.io/releases/tag/0.99.7), with an **MCP server** so agents can place voxels, paint colors, fill shapes, and save `.vox` files.

> MagicaVoxel is **not open source** (binary-only). This project reimplements editor ops and `.vox` I/O from the public format docs in [ephtracy/voxel-model](https://github.com/ephtracy/voxel-model).

## Crates

| Crate | Role |
|-------|------|
| `voxel-core` | Grid, palette, brushes (box/sphere/flood/mirror) |
| `voxel-vox` | MagicaVoxel `.vox` read/write (SIZE/XYZI/RGBA; skips nTRN/MATL/…) |
| `voxel-mcp` | MCP tool surface |
| `voxel-mcp-server` | stdio MCP binary |
| `voxel-editor` | egui slice-view GUI (macOS / Windows / Linux) |

## Build

```bash
cargo build --release
```

Binaries:

- `target/release/voxel-editor`
- `target/release/voxel-mcp-server`

## Run the editor

```bash
VOXEL_PROJECT=./project.vox cargo run -p voxel_editor --release
# or
./target/release/voxel-editor
```

### 3D viewport controls

| Input | Action |
|-------|--------|
| LMB | Paint (raycast onto voxels / volume) |
| Shift+LMB | Erase hit voxel |
| RMB / Alt+LMB | Orbit |
| MMB / Cmd+LMB | Pan |
| Scroll | Zoom |
| View → Z-slice panel | Optional 2D slice editor |
| View → Reset camera | Frame the volume |

First launch seeds a small demo (two objects) so Model + World modes aren’t empty.

### Model vs World

| Mode | Behavior |
|------|----------|
| **Model** | Edit the active voxel volume (like MagicaVoxel model editor) |
| **World** | Scene graph of objects; click to select; translate / add / duplicate |

World objects are stored as MagicaVoxel `nTRN` / `nGRP` / `nSHP` in `.vox`.

MCP world tools: `list_objects`, `add_object`, `duplicate_object`, `set_object_translation`, `select_object`, `set_active_model`, `set_edit_mode`.

## MCP (Cursor / Claude / etc.)

Add to MCP config (example path — adjust to your checkout):

```json
{
  "mcpServers": {
    "voxel-editor": {
      "command": "/ABS/PATH/TO/Untitled/target/release/voxel-mcp-server",
      "args": ["--project", "/ABS/PATH/TO/Untitled/project.vox", "--size", "32"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

Or from source without a prior release build:

```json
{
  "mcpServers": {
    "voxel-editor": {
      "command": "cargo",
      "args": [
        "run",
        "-q",
        "-p",
        "voxel_mcp_server",
        "--",
        "--project",
        "/ABS/PATH/TO/Untitled/project.vox"
      ]
    }
  }
}
```

### Tools (MVP)

| Tool | Purpose |
|------|---------|
| `get_scene_info` | Size, voxel count, brush |
| `list_voxels` | Occupied voxels (truncated if huge) |
| `set_voxel` / `erase_voxel` | Single cell |
| `fill_box` / `fill_sphere` / `flood_fill` | Shapes |
| `set_brush` / `apply_brush` | MagicaVoxel-like brush + mirror |
| `set_active_color` / `set_palette_color` / `get_palette_color` | Palette |
| `clear_model` / `resize_model` / `mirror_model` | Volume ops |
| `save_vox` / `load_vox` / `reload` | Persistence |

Coordinates follow MagicaVoxel: **X right, Y depth, Z up**. Colors are palette indices **1..=255**.

## Feature roadmap (parity with MV 0.99.7)

- [x] Dense model + 256 palette
- [x] `.vox` import/export (basic)
- [x] MCP place/paint/fill
- [x] Slice GUI
- [x] 3D GPU viewport (orbit + raycast paint)
- [x] World editor scene graph (`nTRN`/`nGRP`/`nSHP`)
- [ ] Materials (`MATL`), layers (`LAYR`)
- [ ] Frame animation
- [ ] Path-trace preview
- [ ] Pattern / face / sculpt brushes
- [ ] Object rotation gizmos / delete object UI

## License

MIT. MagicaVoxel name/assets belong to ephtracy; this is an independent reimplementation.
