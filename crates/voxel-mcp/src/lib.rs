//! MCP server exposing MagicaVoxel-like voxel editing tools.

mod params;
mod server;

pub use server::{ensure_project_file, VoxelMcpServer, VoxelMcpState};
