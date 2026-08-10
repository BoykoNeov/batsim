//! A real client for the two WebSocket test binaries: a bound port, a socket, and
//! just enough HTTP to reach the REST routes from outside the process.
//!
//! `tests/rest.rs` drives the router through `tower::ServiceExt::oneshot`, which is
//! right for testing handlers. It is not right for slice C, where the claim is that an
//! **external script** can run an experiment — a claim that is only true if the bytes
//! actually go through a socket. So everything here talks to a listener.
//!
//! The HTTP half is hand-written rather than pulled in as a dependency. It handles
//! exactly what these tests need — one request, one response, `Content-Length`
//! bodies — which is what axum produces for a `Json` reply, and nothing more.

#![allow(dead_code)] // Each test binary uses a different subset.

use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use sim_server::protocol::{Command, Event, Frame};
use sim_server::{serve, AppState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

/// The repo's own `chemistries/` directory, so `chemistry = "lfp_26650_generic"`
/// resolves the way it will for a person running the binary from the workspace root.
#[must_use]
pub fn chem_dir() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../chemistries")
}

/// Bind an ephemeral port, serve on it, and return the address.
///
/// Port 0 rather than a fixed port: these tests run concurrently with each other by
/// default, and a fixed port would make them fail in a way that looks like a protocol
/// bug.
pub async fn spawn(state: AppState) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("an ephemeral port");
    let address = listener.local_addr().expect("the bound address");
    tokio::spawn(async move {
        // The task outlives the test only if the test leaks it, which is fine: the
        // process exits at the end of the binary.
        let _ = serve(listener, state).await;
    });
    address
}

/// One HTTP request/response against a live server.
///
/// Returns the status code and the body. Panics on anything it does not understand,
/// which is the right behaviour for a test helper — a surprise here is a test bug, not
/// a result.
pub async fn http(
    address: SocketAddr,
    method: &str,
    path: &str,
    content_type: Option<&str>,
    body: &str,
) -> (u16, String) {
    let mut stream = TcpStream::connect(address).await.expect("connect");

    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\
         Content-Length: {}\r\n",
        body.len()
    );
    if let Some(ct) = content_type {
        request.push_str(&format!("Content-Type: {ct}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);

    stream
        .write_all(request.as_bytes())
        .await
        .expect("write request");

    // `Connection: close` means the server closes when it is done, so reading to EOF
    // is an unambiguous end-of-response and no chunk or length parsing is needed.
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("read response");
    let raw = String::from_utf8(raw).expect("responses here are UTF-8");

    let (head, body) = raw
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("no header/body split in {raw:?}"));
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("no status in {head:?}"));

    (status, body.to_owned())
}

/// Create a session from scenario TOML and return its id.
pub async fn create_session(address: SocketAddr, toml: &str) -> u64 {
    let (status, body) = http(address, "POST", "/sessions", Some("application/toml"), toml).await;
    assert_eq!(status, 201, "session not created: {body}");
    let value: serde_json::Value = serde_json::from_str(&body).expect("JSON reply");
    value["id"].as_u64().expect("an id")
}

/// A WebSocket attached to one session.
pub struct Client {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl Client {
    /// Attach to a session. The returned client has **not** yet read its hello frame.
    pub async fn attach(address: SocketAddr, id: u64) -> Self {
        let (socket, _) = connect_async(format!("ws://{address}/sessions/{id}/ws"))
            .await
            .expect("the upgrade succeeds");
        Self { socket }
    }

    /// Attach and consume the hello frame, returning it.
    pub async fn attach_and_greet(address: SocketAddr, id: u64) -> (Self, Event) {
        let mut client = Self::attach(address, id).await;
        let hello = client.next_event().await;
        assert!(
            matches!(hello, Event::Hello { .. }),
            "the first frame on every socket is Hello, got {hello:?}"
        );
        (client, hello)
    }

    /// Send a command.
    pub async fn send(&mut self, command: &Command) {
        let text = serde_json::to_string(command).expect("commands serialize");
        self.socket
            .send(Message::text(text))
            .await
            .expect("the socket accepts the command");
    }

    /// Send arbitrary text, for the cases where a well-formed `Command` is exactly what
    /// is *not* being tested.
    pub async fn send_raw(&mut self, text: &str) {
        self.socket
            .send(Message::text(text.to_owned()))
            .await
            .expect("the socket accepts the text");
    }

    /// The next event, skipping transport-level ping/pong.
    pub async fn next_event(&mut self) -> Event {
        loop {
            let message = self
                .socket
                .next()
                .await
                .expect("the socket is still open")
                .expect("a readable frame");
            match message {
                Message::Text(text) => {
                    return serde_json::from_str(&text).unwrap_or_else(|e| {
                        panic!("server sent something that is not an Event: {e}\n  {text}")
                    })
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                other => panic!("unexpected frame from the server: {other:?}"),
            }
        }
    }

    /// Read until `BatchComplete`, returning every frame that arrived before it.
    ///
    /// The terminator is what makes a batch reply self-delimiting: every frame the
    /// batch will produce has already arrived when it lands.
    pub async fn drain_batch(&mut self) -> Vec<Frame> {
        let mut frames = Vec::new();
        loop {
            match self.next_event().await {
                Event::Telemetry(frame) => frames.push(frame),
                Event::BatchComplete {
                    steps, reported, ..
                } => {
                    assert_eq!(
                        reported as usize,
                        frames.len(),
                        "BatchComplete claims {reported} frames for {steps} steps but \
                         {} arrived",
                        frames.len()
                    );
                    return frames;
                }
                other => panic!("expected telemetry or a batch terminator, got {other:?}"),
            }
        }
    }

    /// Send a step command and read its whole reply.
    pub async fn step(&mut self, command: &Command) -> Vec<Frame> {
        self.send(command).await;
        self.drain_batch().await
    }

    /// Send a command and expect exactly one event back.
    pub async fn round_trip(&mut self, command: &Command) -> Event {
        self.send(command).await;
        self.next_event().await
    }

    /// Close the socket and wait for the server to notice.
    ///
    /// Waiting matters: a test that reconnects immediately after dropping a writer
    /// would otherwise race the server's release of the writer slot and fail
    /// intermittently in a way that reads like a protocol bug.
    pub async fn close(mut self) {
        let _ = self.socket.close(None).await;
        // Read to end-of-stream; the server's half of the close handshake is the
        // signal that its socket task has returned.
        while self.socket.next().await.is_some() {}
    }
}

/// Every `f64` a [`Frame`] carries, as raw bits.
///
/// `to_bits`, not `==`: `-0.0 == 0.0` and `NaN != NaN`, so `==` can both hide a real
/// difference and invent one.
#[must_use]
pub fn frame_bits(frame: &Frame) -> Vec<u64> {
    let t = &frame.telemetry;
    let mut bits = vec![
        frame.sim_time_s.to_bits(),
        t.v_terminal.to_bits(),
        t.i_actual.to_bits(),
        t.soc_true.to_bits(),
        t.t_min.to_bits(),
        t.t_max.to_bits(),
        t.v_cell_min.to_bits(),
        t.v_cell_max.to_bits(),
        t.soh_capacity.to_bits(),
        t.soh_resistance.to_bits(),
        t.q_gen_w.to_bits(),
        t.q_runaway_w.to_bits(),
        t.q_balancing_w.to_bits(),
        t.i_balancing_a.to_bits(),
        t.i_internal_short_a.to_bits(),
        t.i_external_short_a.to_bits(),
        t.i_rejected_a.to_bits(),
    ];
    // The BMS estimate is the RNG-sensitive one; `u64::MAX` stands in for `None` so a
    // `Some`/`None` change cannot alias a value change.
    bits.push(t.soc_bms.map_or(u64::MAX, f64::to_bits));
    bits
}

/// Every `f64` a [`sim_core::Telemetry`] carries, as raw bits — the in-process half of
/// the same comparison.
#[must_use]
pub fn telemetry_bits(sim_time_s: f64, t: &sim_core::Telemetry) -> Vec<u64> {
    frame_bits(&Frame {
        step: 0,
        sim_time_s,
        telemetry: *t,
    })
}

/// The longest run of digits anywhere in the text.
///
/// A crude but honest measure of "are these numbers full-mantissa?". A `3.3` writes two
/// digits; a value that has been through several hundred steps of a scattered pack
/// writes sixteen or seventeen, and those are the only ones a lossy float parser can
/// mis-round. Without this, a comparison test can pass while comparing round numbers
/// that no encoding would ever damage.
#[must_use]
pub fn longest_digit_run(text: &str) -> usize {
    let mut best = 0;
    let mut run = 0;
    for c in text.chars() {
        if c.is_ascii_digit() {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}
