use anyhow::Result;
use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use voxel_mcp::{
    VoxelMcpServer, VoxelMcpState,
    ensure_project_file,
};

#[derive(Parser, Debug)]
#[command(name = "voxel-mcp-server", about = "MCP server for voxel editing (.vox)")]
struct Args {
    /// Path to the MagicaVoxel-compatible .vox project file
    #[arg(long, env = "VOXEL_PROJECT", default_value = "project.vox")]
    project: PathBuf,

    /// Default cube size when creating a new project
    #[arg(long, default_value_t = 32)]
    size: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let args = Args::parse();
    let project = ensure_project_file(&args.project, args.size)?;
    tracing::info!("voxel MCP bound to {}", args.project.display());

    let state = VoxelMcpState::new(project, args.project);
    let service = VoxelMcpServer::new(state).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
