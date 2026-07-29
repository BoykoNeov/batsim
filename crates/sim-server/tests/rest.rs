//! The REST surface, driven through the router.
//!
//! Most of these use `tower::ServiceExt::oneshot`, which hands a request straight to
//! the `Router` — no port, no client dependency, and no flake from binding. The one
//! thing that cannot reach is whether the *binary's* path (bind → `axum::serve` →
//! accept) works at all, so exactly one test does that over a real ephemeral port with
//! a hand-written request. Slice C's exit gate lives on a real socket throughout; this
//! is the smallest thing that keeps `serve` from being untested until then.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sim_server::{app, AppState};
use tower::ServiceExt;

const CC_DISCHARGE: &str = include_str!("../../../scenarios/cc_discharge_lfp.toml");
const SOFT_SHORT: &str = include_str!("../../../scenarios/soft_short_under_a_lying_sensor.toml");

/// The repo's own `chemistries/` directory, so `chemistry = "lfp_26650_generic"`
/// resolves the way it will for a person running the binary from the workspace root.
fn state() -> AppState {
    AppState::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../chemistries"))
}

/// Send a request and read the whole response.
async fn send(state: &AppState, req: Request<Body>) -> (StatusCode, String) {
    let response = app(state.clone())
        .oneshot(req)
        .await
        .expect("the router is infallible");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    (status, String::from_utf8(bytes.to_vec()).expect("utf-8"))
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("expected JSON, got {text:?}: {e}"))
}

fn get(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request")
}

fn post(path: &str, content_type: Option<&str>, body: &str) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri(path);
    if let Some(ct) = content_type {
        builder = builder.header("content-type", ct);
    }
    builder.body(Body::from(body.to_owned())).expect("request")
}

/// Create a session from TOML and return its id.
async fn create(state: &AppState, toml: &str) -> u64 {
    let (status, body) = send(state, post("/sessions", Some("application/toml"), toml)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    json_of(&body)["id"].as_u64().expect("an id")
}

/// The error code out of a failed response, so assertions name the contract rather
/// than a message that is allowed to be reworded.
fn error_code(body: &str) -> String {
    json_of(body)["error"]["code"]
        .as_str()
        .unwrap_or_else(|| panic!("no error code in {body}"))
        .to_owned()
}

#[tokio::test]
async fn api_root_reports_both_versions_and_says_which_is_which() {
    let state = state();
    let (status, body) = send(&state, get("/")).await;
    assert_eq!(status, StatusCode::OK);

    let root = json_of(&body);
    assert_eq!(root["api_version"], json!(sim_server::API_VERSION));
    assert_eq!(root["snapshot_version"], json!(sim_core::SNAPSHOT_VERSION));
    assert_eq!(root["sessions"], json!(0));

    // The two numbers do different jobs, and a client poking at this with curl is not
    // reading rustdoc. If the note ever stops mentioning both, it has stopped doing
    // the job it exists for.
    let note = root["note"].as_str().expect("a note");
    assert!(
        note.contains("api_version") && note.contains("snapshot_version"),
        "{note}"
    );
}

#[tokio::test]
async fn a_toml_scenario_becomes_a_session() {
    let state = state();
    let (status, body) = send(
        &state,
        post("/sessions", Some("application/toml"), CC_DISCHARGE),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    let created = json_of(&body);
    assert_eq!(created["pack"]["series"], json!(1));
    assert_eq!(created["pack"]["parallel"], json!(1));
    assert_eq!(created["pack"]["sim_time_s"], json!(0.0));
    assert_eq!(created["api_version"], json!(sim_server::API_VERSION));
}

/// No `Content-Type` at all is TOML, because TOML is what every file under
/// `scenarios/` is and `curl --data-binary @file` sends no type.
#[tokio::test]
async fn a_missing_content_type_is_read_as_toml() {
    let state = state();
    let (status, body) = send(&state, post("/sessions", None, CC_DISCHARGE)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn a_charset_parameter_does_not_confuse_the_content_type() {
    let state = state();
    let (status, body) = send(
        &state,
        post(
            "/sessions",
            Some("application/toml; charset=utf-8"),
            CC_DISCHARGE,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn a_json_scenario_becomes_the_same_session_as_its_toml() {
    let state = state();
    let scenario = sim_data::parse_scenario(SOFT_SHORT).expect("shipped scenario");
    let as_json = serde_json::to_string(&scenario).expect("Scenario serialises");

    let (status, body) = send(
        &state,
        post("/sessions", Some("application/json"), &as_json),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = json_of(&body)["id"].as_u64().expect("an id");

    let (status, body) = send(&state, get(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    let detail = json_of(&body);
    assert_eq!(detail["pack"]["series"], json!(4));
    assert_eq!(detail["pack"]["parallel"], json!(2));
    // Round trip: the scenario the server echoes back is the one that went in.
    assert_eq!(
        serde_json::from_value::<sim_data::Scenario>(detail["scenario"].clone()).unwrap(),
        scenario
    );
}

/// A scenario that inlines its chemistry needs no `chemistries/` tree at all — the
/// property that makes this server deployable without shipping one.
///
/// Both shipped scenarios name a chemistry *id*, so before this test the inline path
/// had no coverage in this crate at all, and it is the harder of the two: the
/// chemistry TOML is an arbitrary multi-line string that has to survive being embedded
/// in a scenario, stored, re-serialised as a JSON string by `GET /sessions/{id}`, and
/// parsed back. Newlines and quotes are exactly what that round trip mishandles.
///
/// `chem_dir` deliberately points at a directory that does not exist: if resolution
/// ever silently fell back to the filesystem, this would fail rather than pass for the
/// wrong reason.
#[tokio::test]
async fn a_scenario_can_inline_its_chemistry_and_survive_the_round_trip() {
    const LFP: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");

    let state = AppState::new("no/such/directory");

    // A TOML *literal* string: no escape processing, so the chemistry's own quotes and
    // backslashes pass through untouched. TOML drops the newline right after the
    // opening delimiter, so the embedded text is byte-for-byte the file.
    let scenario_toml = format!(
        "chemistry_toml = '''\n{LFP}'''\n\n{}",
        CC_DISCHARGE
            .lines()
            .filter(|l| !l.starts_with("chemistry ="))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let (status, body) = send(
        &state,
        post("/sessions", Some("application/toml"), &scenario_toml),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = json_of(&body)["id"].as_u64().expect("an id");

    let (status, body) = send(&state, get(&format!("/sessions/{id}"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let detail = json_of(&body);

    assert_eq!(
        detail["scenario"]["chemistry_toml"].as_str(),
        Some(LFP),
        "the inlined chemistry did not survive being serialised as a JSON string"
    );
    assert!(
        detail["scenario"]["chemistry"].is_null(),
        "an inlined scenario has no chemistry id"
    );

    // And the round-tripped scenario still builds the same pack, which is the claim
    // that matters: re-parsing the echoed value is not a formality.
    let echoed: sim_data::Scenario =
        serde_json::from_value(detail["scenario"].clone()).expect("a Scenario");
    echoed
        .validate()
        .expect("the echoed scenario is still valid");
    let (status, body) = send(
        &state,
        post(
            "/sessions",
            Some("application/json"),
            &serde_json::to_string(&echoed).unwrap(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

/// The one that matters for safety: `serde_json::from_str` does **not** call
/// `Scenario::validate`, so the JSON path has to reach validation some other way. If
/// it does not, the `[a-z0-9_]+` charset check on a chemistry id is bypassed and a
/// scenario body chooses which file the server reads.
#[tokio::test]
async fn the_json_path_still_validates_the_chemistry_id() {
    let state = state();
    let mut scenario = sim_data::parse_scenario(CC_DISCHARGE).expect("shipped scenario");
    scenario.chemistry = Some("../../../etc/passwd".into());

    let body = serde_json::to_string(&scenario).expect("serialises");
    let (status, body) = send(&state, post("/sessions", Some("application/json"), &body)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(error_code(&body), "invalid_scenario");
    assert!(
        body.contains("[a-z0-9_]+"),
        "the message should name the rule that rejected it: {body}"
    );
}

#[tokio::test]
async fn an_unresolvable_chemistry_id_names_the_directory() {
    let state = state();
    let scenario = CC_DISCHARGE.replace("lfp_26650_generic", "no_such_chemistry");

    let (status, body) = send(
        &state,
        post("/sessions", Some("application/toml"), &scenario),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(error_code(&body), "unknown_chemistry");
    assert!(body.contains("no_such_chemistry"), "{body}");
}

#[tokio::test]
async fn a_scenario_that_parses_but_cannot_build_is_its_own_error() {
    let state = state();
    // `initial_soc` outside [0, 1] is the engine's check, not sim-data's — the two
    // error codes stay distinguishable so a client can tell "your file is malformed"
    // from "your pack is impossible".
    let scenario = CC_DISCHARGE.replace("initial_soc = 1.0", "initial_soc = 1.5");

    let (status, body) = send(
        &state,
        post("/sessions", Some("application/toml"), &scenario),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(error_code(&body), "unbuildable_pack");
}

#[tokio::test]
async fn json_sent_as_toml_gets_a_hint_rather_than_a_parser_dump() {
    let state = state();
    let scenario = sim_data::parse_scenario(CC_DISCHARGE).expect("shipped scenario");
    let as_json = serde_json::to_string(&scenario).expect("serialises");

    let (status, body) = send(&state, post("/sessions", None, &as_json)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(error_code(&body), "malformed_body");
    assert!(
        body.contains("application/json"),
        "a JSON body sent as TOML should be told what header it wanted: {body}"
    );
}

/// `curl --data-binary @scenario.toml` is the documented way to reach this endpoint,
/// and curl labels that body `application/x-www-form-urlencoded` whether you asked it
/// to or not. This was found by running the documented command against the binary,
/// not by reading it — the 415 it used to produce made the one invocation the docs
/// promise the one invocation that failed.
#[tokio::test]
async fn curls_default_content_type_is_read_as_toml() {
    let state = state();
    let (status, body) = send(
        &state,
        post(
            "/sessions",
            Some("application/x-www-form-urlencoded"),
            CC_DISCHARGE,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn an_unaccepted_content_type_is_a_415() {
    let state = state();
    let (status, body) = send(
        &state,
        post("/sessions", Some("application/xml"), CC_DISCHARGE),
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{body}");
    assert_eq!(error_code(&body), "unsupported_media_type");
}

#[tokio::test]
async fn sessions_list_in_id_order_and_delete_individually() {
    let state = state();
    let first = create(&state, CC_DISCHARGE).await;
    let second = create(&state, SOFT_SHORT).await;

    let (status, body) = send(&state, get("/sessions")).await;
    assert_eq!(status, StatusCode::OK);
    let list = json_of(&body);
    let ids: Vec<u64> = list
        .as_array()
        .expect("an array")
        .iter()
        .map(|s| s["id"].as_u64().expect("an id"))
        .collect();
    assert_eq!(ids, vec![first, second], "a BTreeMap lists in id order");
    assert_eq!(list[0]["name"], json!("CC discharge, single LFP cell"));
    assert_eq!(
        list[0]["stepped"],
        json!(false),
        "nothing has stepped: this slice has no stepping endpoint"
    );

    let (status, _) = send(
        &state,
        Request::builder()
            .method("DELETE")
            .uri(format!("/sessions/{first}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = send(&state, get(&format!("/sessions/{first}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error_code(&body), "no_such_session");

    // The other one is untouched.
    let (status, _) = send(&state, get(&format!("/sessions/{second}"))).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_never_stepped_session_reports_null_telemetry() {
    let state = state();
    let id = create(&state, CC_DISCHARGE).await;

    let (_, body) = send(&state, get(&format!("/sessions/{id}"))).await;
    assert_eq!(
        json_of(&body)["latest_telemetry"],
        Value::Null,
        "a session that has never stepped has no telemetry, and synthesising one with a \
         dt = 0 probe step would be inventing a frame the client did not ask for"
    );
}

#[tokio::test]
async fn cells_are_series_major_and_ground_truth() {
    let state = state();
    let id = create(&state, SOFT_SHORT).await;

    let (status, body) = send(&state, get(&format!("/sessions/{id}/cells"))).await;
    assert_eq!(status, StatusCode::OK);
    let cells = json_of(&body);
    assert_eq!(cells["series"], json!(4));
    assert_eq!(cells["parallel"], json!(2));

    let array = cells["cells"].as_array().expect("an array");
    assert_eq!(array.len(), 8, "one entry per cell, series-major");

    // Ground truth, not a BMS view: scatter has already made the cells differ, and
    // this is the endpoint that shows it.
    let factors: Vec<f64> = array
        .iter()
        .map(|c| c["capacity_factor"].as_f64().expect("a factor"))
        .collect();
    assert!(
        factors.windows(2).any(|w| w[0] != w[1]),
        "the scenario has capacity scatter, so its cells must not be identical: {factors:?}"
    );

    // The ordering claim, checked against the engine rather than restated: index
    // s * parallel + p must be the cell the engine hands back for (s, p).
    let scenario = sim_data::parse_scenario(SOFT_SHORT).unwrap();
    let chem = sim_data::load_chemistry_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../chemistries/lfp_26650_generic.toml"
    ))
    .unwrap();
    let pack = scenario.build_pack(chem).unwrap();
    for s in 0..4usize {
        for p in 0..2usize {
            let want = pack.cell(s, p).unwrap();
            let got: sim_core::CellView =
                serde_json::from_value(array[s * 2 + p].clone()).expect("a CellView");
            assert_eq!(want, got, "cell at index {} is not ({s}, {p})", s * 2 + p);
        }
    }
}

/// The sensors route is the BMS's *view*, and its shape is pinned against the pack's
/// own config rather than against restated literals.
///
/// The pairing that makes it a view rather than a second ground truth is the probe
/// list: this scenario runs two probes for four groups, and the route says which two.
#[tokio::test]
async fn sensors_report_the_bms_view_and_where_its_probes_sit() {
    let state = state();
    let id = create(&state, SOFT_SHORT).await;

    let (status, body) = send(&state, get(&format!("/sessions/{id}/sensors"))).await;
    assert_eq!(status, StatusCode::OK);
    let sensors = json_of(&body);

    let v_group = sensors["v_group"].as_array().expect("an array");
    assert_eq!(
        v_group.len(),
        4,
        "one voltage per series group — parallel cells share a node, so this is the \
         finest voltage resolution any real pack has"
    );

    // Two probes on a four-group pack, and neither on the cell that shorts: the
    // under-sampling is the point of the payload, so the positions are part of it.
    assert_eq!(
        sensors["temp_probe_at"],
        json!([[0, 0], [3, 0]]),
        "probe positions must match the scenario's own [pack.bms] temp_probes"
    );
    assert_eq!(
        sensors["temp_probe_k"].as_array().expect("an array").len(),
        2
    );

    // A fresh session has never stepped, so the frame is the construction-time
    // open-circuit read: exactly zero current, at t = 0.
    assert_eq!(sensors["sampled_at_s"], json!(0.0));
    assert_eq!(
        sensors["i_pack_a"],
        json!(0.0),
        "the initial frame is an open-circuit read taken before any sensor draw"
    );
    assert_eq!(sensors["contactor_open"], json!(false));

    // The estimate is the pack's own, wrong by the configured boot error and no more.
    let soc_est = sensors["soc_est"].as_f64().expect("a number");
    assert!(
        (soc_est - 0.88).abs() < 1e-9,
        "initial_soc 0.85 + initial_soc_error 0.03, got {soc_est}"
    );
}

/// A pack with no BMS has no sensors, and that is a supported mode rather than an
/// error — principle 7. The route answers `null`, not 404 and not an empty object.
#[tokio::test]
async fn sensors_are_null_on_a_pack_with_no_bms() {
    let state = state();
    let id = create(&state, CC_DISCHARGE).await;

    let (status, body) = send(&state, get(&format!("/sessions/{id}/sensors"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        json_of(&body),
        Value::Null,
        "a BMS-less pack's sensors are absent, which is a payload and not a failure"
    );
}

#[tokio::test]
async fn a_snapshot_round_trips_through_rest_into_the_same_session() {
    let state = state();
    let id = create(&state, SOFT_SHORT).await;

    let (status, snapshot) = send(&state, get(&format!("/sessions/{id}/snapshot"))).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &state,
        post(
            &format!("/sessions/{id}/snapshot"),
            Some("application/json"),
            &snapshot,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let restored = json_of(&body);
    assert_eq!(
        restored["id"],
        json!(id),
        "restore stays in the same session"
    );
    assert_eq!(restored["pack"]["series"], json!(4));

    // And the pack really is the same one: the snapshot taken after the restore is
    // byte-identical to the one that went in. `float_roundtrip` is what makes this a
    // fact rather than a coincidence — see tests/snapshot_json.rs.
    let (_, again) = send(&state, get(&format!("/sessions/{id}/snapshot"))).await;
    assert_eq!(again, snapshot);
}

#[tokio::test]
async fn a_snapshot_of_a_differently_shaped_pack_is_refused() {
    let state = state();
    let small = create(&state, CC_DISCHARGE).await; // 1S1P
    let large = create(&state, SOFT_SHORT).await; // 4S2P

    let (_, snapshot) = send(&state, get(&format!("/sessions/{large}/snapshot"))).await;
    let (status, body) = send(
        &state,
        post(
            &format!("/sessions/{small}/snapshot"),
            Some("application/json"),
            &snapshot,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(error_code(&body), "topology_mismatch");
    assert!(body.contains("1S1P") && body.contains("4S2P"), "{body}");
}

#[tokio::test]
async fn a_snapshot_from_another_schema_version_is_refused() {
    let state = state();
    let id = create(&state, CC_DISCHARGE).await;
    let (_, snapshot) = send(&state, get(&format!("/sessions/{id}/snapshot"))).await;

    // Only the outer tag — that is the field `Pack::restore` gates on, and mangling it
    // alone keeps the body a well-formed `Snapshot` so this tests the version check
    // rather than the parser.
    let current = format!("\"version\":{}", sim_core::SNAPSHOT_VERSION);
    let stale = snapshot.replacen(&current, "\"version\":1", 1);
    assert_ne!(stale, snapshot, "the version tag should have been found");

    let (status, body) = send(
        &state,
        post(
            &format!("/sessions/{id}/snapshot"),
            Some("application/json"),
            &stale,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(error_code(&body), "bad_snapshot");
}

#[tokio::test]
async fn a_malformed_snapshot_body_is_this_servers_error_shape() {
    let state = state();
    let id = create(&state, CC_DISCHARGE).await;

    let (status, body) = send(
        &state,
        post(
            &format!("/sessions/{id}/snapshot"),
            Some("application/json"),
            "{not json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    // Not axum's default rejection text: a client parses `error.code`, and the
    // framework's plain-text rejection has none.
    assert_eq!(error_code(&body), "bad_snapshot");
}

#[tokio::test]
async fn every_session_route_404s_on_an_unknown_id() {
    let state = state();
    for path in [
        "/sessions/99",
        "/sessions/99/cells",
        "/sessions/99/sensors",
        "/sessions/99/snapshot",
    ] {
        let (status, body) = send(&state, get(path)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
        assert_eq!(error_code(&body), "no_such_session", "{path}");
    }
}

/// Every `ErrorCode`'s spelling on the wire, pinned to its Rust name.
///
/// The variant names are `snake_case`d by a serde attribute, so a rename changes the
/// string a client matches on without changing anything that reads oddly at a call
/// site — every other assertion in this file would keep passing. `ErrorCode`'s doc
/// comment says a rename bumps `API_VERSION`; this is what makes that enforceable
/// rather than aspirational, and it is the same vocabulary slice C's WebSocket `Error`
/// event will carry.
#[test]
fn error_codes_have_pinned_wire_spellings() {
    use sim_server::ErrorCode;

    for (code, spelling) in [
        (ErrorCode::MalformedBody, "malformed_body"),
        (ErrorCode::InvalidScenario, "invalid_scenario"),
        (ErrorCode::UnknownChemistry, "unknown_chemistry"),
        (ErrorCode::UnbuildablePack, "unbuildable_pack"),
        (ErrorCode::NoSuchSession, "no_such_session"),
        (ErrorCode::BadSnapshot, "bad_snapshot"),
        (ErrorCode::TopologyMismatch, "topology_mismatch"),
        (ErrorCode::UnsupportedMediaType, "unsupported_media_type"),
    ] {
        assert_eq!(
            serde_json::to_value(code).unwrap(),
            json!(spelling),
            "{code:?} changed its spelling on the wire"
        );
    }
}

/// The only test that binds a socket: proves `serve` accepts and answers at all.
///
/// The request is written by hand rather than through a client crate — one HTTP/1.1
/// GET is a handful of bytes and does not justify a dependency that exists only for
/// tests.
#[tokio::test]
async fn the_binary_path_actually_serves() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("an ephemeral port");
    let addr = listener.local_addr().expect("the bound address");
    let server = tokio::spawn(sim_server::serve(listener, state()));

    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connecting to the server we just started");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("writing the request");

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .expect("reading the response");

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.contains("\"api_version\""), "{response}");

    server.abort();
}
