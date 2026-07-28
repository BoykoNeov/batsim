//! The `sim-server` binary.
//!
//! ```text
//! cargo run -p sim-server -- [--bind ADDR] [--chem-dir DIR] [--web-dir DIR] [--scenario-dir DIR]
//! ```
//!
//! Every default is relative to the working directory and matches the repo layout, so
//! `cargo run -p sim-server` from the workspace root serves the shipped chemistries,
//! the shipped scenarios and the demo page with no arguments at all.
//!
//! Argument parsing is by hand. A `clap` dependency would buy nothing over four flags
//! and would be a dependency added to this crate for the convenience of its own command
//! line rather than for the job it does.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use sim_server::{serve, AppState, StaticDirs, API_VERSION};
use tokio::net::TcpListener;

struct Args {
    bind: String,
    chem_dir: PathBuf,
    static_dirs: StaticDirs,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".into(),
            chem_dir: PathBuf::from("chemistries"),
            static_dirs: StaticDirs::default(),
        }
    }
}

const USAGE: &str = "\
usage: sim-server [--bind ADDR] [--chem-dir DIR] [--web-dir DIR] [--scenario-dir DIR]

  --bind ADDR         address to listen on (default 127.0.0.1:8080)
  --chem-dir DIR      directory holding <chemistry_id>.toml, also served at
                      /chemistries (default ./chemistries)
  --web-dir DIR       directory holding the demo page and its pkg/ bundle, served
                      at /app/ (default ./web)
  --scenario-dir DIR  directory holding scenario TOML, served at /scenarios
                      (default ./scenarios)

The demo page needs a wasm bundle that is not committed:
  wasm-pack build crates/sim-wasm --target web --out-dir ../../web/pkg
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
            "--web-dir" => {
                args.static_dirs.web = argv.next().context("--web-dir needs a directory")?.into();
            }
            "--scenario-dir" => {
                args.static_dirs.scenarios = argv
                    .next()
                    .context("--scenario-dir needs a directory")?
                    .into();
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
    if !args.static_dirs.scenarios.is_dir() {
        tracing::warn!(
            scenario_dir = %args.static_dirs.scenarios.display(),
            "scenario directory does not exist; /scenarios will 404 and the demo page's \
             picker will be empty (the REST and WebSocket API are unaffected)"
        );
    }
    // The bundle is a build artifact and is deliberately not committed, so a fresh
    // clone lands here. Say the command rather than the symptom — "404" in a browser
    // tells nobody to run wasm-pack.
    if !args.static_dirs.web.join("pkg").is_dir() {
        tracing::warn!(
            web_dir = %args.static_dirs.web.display(),
            "no pkg/ under the web directory, so /app/ cannot load the engine; build it \
             with: wasm-pack build crates/sim-wasm --target web --out-dir ../../web/pkg"
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
        web_dir = %args.static_dirs.web.display(),
        scenario_dir = %args.static_dirs.scenarios.display(),
        "sim-server listening; demo page at http://{addr}/app/"
    );

    let state = AppState::new(args.chem_dir).with_static_dirs(args.static_dirs);
    serve(listener, state).await.context("serving")
}
