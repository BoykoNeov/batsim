//! The `sim-server` binary.
//!
//! ```text
//! cargo run -p sim-server -- [--bind ADDR] [--chem-dir DIR]
//! ```
//!
//! Defaults: `127.0.0.1:8080` and `chemistries` relative to the working directory, so
//! `cargo run -p sim-server` from the workspace root finds the shipped chemistries
//! with no arguments.
//!
//! Argument parsing is by hand. A `clap` dependency would buy nothing over two flags
//! and would be the second dependency added to this crate for the convenience of its
//! own command line rather than for the job it does.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use sim_server::{serve, AppState, API_VERSION};
use tokio::net::TcpListener;

struct Args {
    bind: String,
    chem_dir: PathBuf,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".into(),
            chem_dir: PathBuf::from("chemistries"),
        }
    }
}

const USAGE: &str = "\
usage: sim-server [--bind ADDR] [--chem-dir DIR]

  --bind ADDR      address to listen on (default 127.0.0.1:8080)
  --chem-dir DIR   directory holding <chemistry_id>.toml (default ./chemistries)
";

fn parse_args() -> Result<Args> {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        match flag.as_str() {
            "--bind" => {
                args.bind = argv.next().context("--bind needs an address")?;
            }
            "--chem-dir" => {
                args.chem_dir = argv.next().context("--chem-dir needs a directory")?.into();
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("unknown argument {other:?}\n\n{USAGE}"),
        }
    }
    Ok(args)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sim_server=info,tower_http=info".into()),
        )
        .init();

    let args = parse_args()?;

    if !args.chem_dir.is_dir() {
        // A warning rather than an error: a server whose scenarios all inline their
        // chemistry never touches this directory, and refusing to start would make
        // that perfectly good configuration impossible.
        tracing::warn!(
            chem_dir = %args.chem_dir.display(),
            "chemistry directory does not exist; scenarios naming a chemistry id will fail \
             (scenarios that inline `chemistry_toml` are unaffected)"
        );
    }

    let listener = TcpListener::bind(&args.bind)
        .await
        .with_context(|| format!("binding {}", args.bind))?;
    let addr: SocketAddr = listener.local_addr().context("reading the bound address")?;

    tracing::info!(
        %addr,
        api_version = API_VERSION,
        snapshot_version = sim_core::SNAPSHOT_VERSION,
        chem_dir = %args.chem_dir.display(),
        "sim-server listening"
    );

    serve(listener, AppState::new(args.chem_dir))
        .await
        .context("serving")
}
