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
