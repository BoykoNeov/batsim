//! The three static routes, which exist for exactly one client.
//!
//! The browser page has no filesystem: it fetches scenario TOML and chemistry TOML as
//! **text** and hands them to `sim-wasm`, which is the resolution
//! `docs/plans/phase-4-server-wasm.md` chose for that adapter. And a wasm module cannot
//! be loaded from `file://` at all (CORS), so the page needs *a* server whether or not
//! it ever opens a socket. Slice B listed `tower-http` and then refused it because the
//! page it would serve did not exist yet; this is the user it was waiting for.
//!
//! These use `oneshot` rather than a real port, but unlike `tests/rest.rs` they keep
//! the response **headers** — a `.wasm` served as `text/plain` fails
//! `WebAssembly.instantiateStreaming` in a way no status code shows.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sim_server::{app, AppState, StaticDirs};
use tower::ServiceExt;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn state() -> AppState {
    AppState::new(format!("{REPO}/chemistries")).with_static_dirs(StaticDirs {
        web: format!("{REPO}/web").into(),
        scenarios: format!("{REPO}/scenarios").into(),
    })
}

struct Res {
    status: StatusCode,
    content_type: Option<String>,
    location: Option<String>,
    body: String,
}

async fn get(state: &AppState, path: &str) -> Res {
    let req = Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request");
    let response = app(state.clone())
        .oneshot(req)
        .await
        .expect("the router is infallible");
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let status = response.status();
    let content_type = header("content-type");
    let location = header("location");
    let bytes = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .expect("body");
    Res {
        status,
        content_type,
        location,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

/// `/app` without the trailing slash is a redirect, and it is load-bearing twice over.
///
/// Under `nest_service` the inner path for `/app` is the empty string — neither a file
/// nor a directory — so it 404s; and every relative URL in the page (`./app.js`,
/// `./pkg/…`) resolves against the wrong base without the slash, which would 404 the
/// script from a page that had itself loaded fine.
#[tokio::test]
async fn the_app_route_redirects_to_its_trailing_slash() {
    let res = get(&state(), "/app").await;
    assert!(
        res.status.is_redirection(),
        "expected a redirect, got {}",
        res.status
    );
    assert_eq!(res.location.as_deref(), Some("/app/"));
}

#[tokio::test]
async fn the_demo_page_and_its_script_are_served() {
    let state = state();

    let index = get(&state, "/app/").await;
    assert_eq!(index.status, StatusCode::OK);
    assert!(
        index
            .content_type
            .as_deref()
            .is_some_and(|ct| ct.starts_with("text/html")),
        "index.html should be served as HTML, got {:?}",
        index.content_type
    );
    assert!(
        index.body.contains("<title>batsim"),
        "that is not the demo page"
    );

    // The page loads this as `<script type="module">`, which browsers refuse to run
    // unless the MIME type is a JavaScript one. A wrong type here is a blank page and a
    // console message, not a 404.
    let script = get(&state, "/app/app.js").await;
    assert_eq!(script.status, StatusCode::OK);
    let ct = script.content_type.unwrap_or_default();
    assert!(
        ct.contains("javascript"),
        "app.js must be served as JavaScript or the module will not run, got {ct:?}"
    );
}

/// The two text routes the page cannot work without: `sim-wasm` takes chemistry and
/// scenario parameters as strings because there is no filesystem behind it.
#[tokio::test]
async fn chemistry_and_scenario_text_are_fetchable() {
    let state = state();

    let chem = get(&state, "/chemistries/lfp_26650_generic.toml").await;
    assert_eq!(chem.status, StatusCode::OK);
    assert!(
        chem.body.contains("\"lfp_26650_generic\"") && chem.body.contains("[ocv]"),
        "that is not the LFP parameter set"
    );

    let scenario = get(&state, "/scenarios/cc_discharge_lfp.toml").await;
    assert_eq!(scenario.status, StatusCode::OK);
    assert!(
        scenario.body.contains("chemistry = \"lfp_26650_generic\""),
        "that is not the CC discharge scenario"
    );

    // And the page's own two-step dance works end to end against these bytes: ask the
    // scenario which chemistry it needs, then fetch that. The id is charset-checked by
    // `parse_scenario`, which is what makes interpolating it into a URL safe.
    let id = sim_data::parse_scenario(&scenario.body)
        .expect("the served text parses")
        .chemistry
        .expect("this scenario names an id");
    let refetched = get(&state, &format!("/chemistries/{id}.toml")).await;
    assert_eq!(refetched.status, StatusCode::OK);
    assert_eq!(refetched.body, chem.body);
}

/// The catalogue: `/scenarios` answers what is in the directory that `/scenarios/…`
/// serves file by file.
///
/// Also the guard on the route split this needed. The directory service moved from
/// `/scenarios` to `/scenarios/` so the bare path could carry JSON, which changes the
/// prefix `ServeDir` strips — `chemistry_and_scenario_text_are_fetchable` above is the
/// other half of that check, and it fetches a file by name.
#[tokio::test]
async fn the_scenario_listing_names_every_file_in_the_directory() {
    let res = get(&state(), "/scenarios").await;
    assert_eq!(res.status, StatusCode::OK);
    let listed: Vec<serde_json::Value> = serde_json::from_str(&res.body).expect("a JSON array");

    // Every `*.toml` in the repo's own directory, and nothing else.
    let mut on_disk: Vec<String> = std::fs::read_dir(format!("{REPO}/scenarios"))
        .expect("the repo's scenario directory")
        .map(|e| {
            e.expect("a directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n.ends_with(".toml"))
        .collect();
    on_disk.sort();
    let files: Vec<String> = listed
        .iter()
        .map(|e| e["file"].as_str().expect("a file name").to_owned())
        .collect();
    assert_eq!(files, on_disk, "the listing and the directory disagree");
    assert!(!files.is_empty(), "the repo ships scenarios");

    // Sorted by name, not by whatever order the filesystem walked them in. A picker
    // whose entries move between hosts is the client-side form of the nondeterminism
    // `CLAUDE.md` bans inside the engine.
    let mut sorted = files.clone();
    sorted.sort();
    assert_eq!(files, sorted, "the listing is not sorted by file name");

    // And the facts are flattened into the entry rather than nested under a key.
    let lfp = listed
        .iter()
        .find(|e| e["file"] == "cc_discharge_lfp.toml")
        .expect("the CC discharge scenario is listed");
    assert_eq!(lfp["chemistry"], "lfp_26650_generic");
    assert_eq!(lfp["series"], 1);
    assert_eq!(lfp["parallel"], 1);
    assert_eq!(lfp["bms"], false);
    assert_eq!(lfp["aging"], false);
    assert_eq!(lfp["faults"], 0);
    assert_eq!(lfp["thermal"], "Isothermal");
    assert_eq!(lfp["cell_model"], "Ecm");
    assert!(lfp["name"].is_string(), "a scenario has a [meta] name");
    assert!(
        lfp.get("error").is_none(),
        "a parsing file carries no error"
    );

    // The trailing-slash path with no file after it is a 404 with an empty body — no
    // directory index, and in particular not a second, differently-shaped answer to the
    // question the bare path answers in JSON. Pinned because it was discovered rather
    // than chosen, and because `/app/` — the other nested service — answers its own
    // bare form with `index.html`, so the two are not alike.
    let bare = get(&state(), "/scenarios/").await;
    assert_eq!(bare.status, StatusCode::NOT_FOUND);
    assert!(
        bare.body.is_empty(),
        "no directory index, got {}",
        bare.body
    );
}

/// A scenario that does not parse is **listed, carrying its error**.
///
/// Skipping it would produce the worst report available: the author of a broken file
/// sees a picker that does not mention it, and nothing anywhere says why. Same shape as
/// the banner that erased itself — the diagnostic existed and the path through it that
/// mattered never ran.
#[tokio::test]
async fn a_scenario_that_does_not_parse_is_listed_with_its_error() {
    let dir = std::env::temp_dir().join(format!("batsim-listing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temp directory");
    std::fs::copy(
        format!("{REPO}/scenarios/cc_discharge_lfp.toml"),
        dir.join("good.toml"),
    )
    .expect("a good scenario");
    // Valid TOML, invalid scenario: no chemistry key at all, which `Scenario::validate`
    // rejects. That exercises the validation path rather than only the TOML parser.
    std::fs::write(
        dir.join("broken.toml"),
        "[meta]\nname = \"no chemistry anywhere\"\n\n[pack]\nseries = 1\nparallel = 1\n\
         initial_soc = 1.0\ninitial_temp_k = 298.15\nseed = 1\n",
    )
    .expect("a broken scenario");
    // Not a scenario and not listed: the filter is on the extension, not on hope.
    std::fs::write(dir.join("notes.md"), "# not a scenario\n").expect("a stray file");

    let state = AppState::new(format!("{REPO}/chemistries")).with_static_dirs(StaticDirs {
        web: format!("{REPO}/web").into(),
        scenarios: dir.clone(),
    });
    let res = get(&state, "/scenarios").await;
    assert_eq!(
        res.status,
        StatusCode::OK,
        "one broken file is not a failed request"
    );
    let listed: Vec<serde_json::Value> = serde_json::from_str(&res.body).expect("a JSON array");

    let files: Vec<&str> = listed.iter().map(|e| e["file"].as_str().unwrap()).collect();
    assert_eq!(
        files,
        ["broken.toml", "good.toml"],
        "the stray file is not a scenario"
    );

    let broken = &listed[0];
    assert!(
        broken["error"]
            .as_str()
            .is_some_and(|m| m.contains("chemistry")),
        "the error should name what is wrong, got {broken}"
    );
    assert!(
        broken.get("name").is_none() && broken.get("series").is_none(),
        "a file that does not parse has no facts to report, got {broken}"
    );
    assert!(listed[1]["error"].is_null() || listed[1].get("error").is_none());

    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// `ServeDir` resolves `..` before touching the disk, so these routes expose their two
/// directories and nothing above them. Worth pinning rather than trusting: this is the
/// same class of hazard the `[a-z0-9_]+` charset check on chemistry ids exists for, and
/// that one is already tested.
#[tokio::test]
async fn the_static_routes_do_not_serve_their_parents() {
    let state = state();
    for escape in [
        "/chemistries/../Cargo.toml",
        "/chemistries/..%2fCargo.toml",
        "/scenarios/../../Cargo.toml",
        "/app/../Cargo.toml",
    ] {
        let res = get(&state, escape).await;
        assert_ne!(
            res.status,
            StatusCode::OK,
            "{escape} escaped its directory and returned a body of {} bytes",
            res.body.len()
        );
    }
}

/// A fresh clone has no `web/pkg` — the bundle is a build artifact and is not
/// committed. The API must not be hostage to a build step it does not use, so a missing
/// directory 404s that one route and leaves everything else alone.
#[tokio::test]
async fn a_missing_web_directory_does_not_take_the_api_with_it() {
    let state = AppState::new(format!("{REPO}/chemistries")).with_static_dirs(StaticDirs {
        web: "no/such/directory".into(),
        scenarios: "no/such/directory".into(),
    });

    assert_eq!(get(&state, "/app/").await.status, StatusCode::NOT_FOUND);
    assert_eq!(
        get(&state, "/scenarios/cc_discharge_lfp.toml").await.status,
        StatusCode::NOT_FOUND
    );
    // The catalogue is a 500 rather than an empty list, and the distinction is the
    // point: "there are no scenarios here" and "you pointed me at nothing" are
    // different facts, and only one of them is the operator's to fix. The message names
    // the directory, because the person reading it chose that path.
    let listing = get(&state, "/scenarios").await;
    assert_eq!(listing.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        listing.body.contains("no/such/directory"),
        "the error should name the directory it could not read, got {}",
        listing.body
    );

    let root = get(&state, "/").await;
    assert_eq!(root.status, StatusCode::OK, "the API still answers");
    // And it says what to do about it in-band, because whoever hits `/app/` is holding
    // a browser rather than this source file.
    assert!(
        root.body.contains("wasm-pack build"),
        "the API root should name the command that produces the bundle, got {}",
        root.body
    );
}
