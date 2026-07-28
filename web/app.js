// batsim browser client.
//
// Two backends, one engine. `WasmBackend` calls `sim-wasm` directly — the same
// `sim-core` the tests link against, compiled to WebAssembly, running in this tab.
// `SocketBackend` drives `sim-server` over a WebSocket instead. Both hand back frames
// of the same shape, which is why everything below the backends is shared.
//
// Things this file has to know about the engine's encodings, all of them decided in
// Rust and none of them negotiable here:
//
//  * Engine enums are **externally tagged**: `{"Current": -5.0}`, `"Rest"`,
//    `{"SoftInternalShort": {...}}`. Same over the socket and over the wasm boundary —
//    one engine should not have two dialects. See `demandJson` below.
//  * `EventFlags` crosses as a `" | "`-joined **string** of names, and `""` means no
//    flags. Not a bitmask integer, and — the two-minute bug — `"".split(" | ")` is
//    `[""]`, a one-element array holding an empty string, not an empty array. See
//    `parseFlags`.
//  * `soc_bms` is `null` when there is no BMS. That is "no estimate exists", which is a
//    different fact from "the estimate is zero", and the readout renders it as absent.

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

const $ = (id) => document.getElementById(id);

const banner = $("banner");
function showBanner(text, kind = "error") {
  banner.textContent = text;
  banner.className = `show ${kind === "info" ? "info" : ""}`;
}
function clearBanner() {
  banner.className = "";
}

let wasm = null;
try {
  // Dynamic, so a missing bundle is a message rather than a blank page. `pkg/` is a
  // build artifact and is deliberately not committed, so this is the state of a fresh
  // clone, not an exotic failure.
  wasm = await import("./pkg/sim_wasm.js");
  await wasm.default();
  $("versions").textContent =
    `api ${wasm.wasm_api_version()} · snapshot ${wasm.snapshot_version()}`;
} catch (e) {
  showBanner(
    "Could not load the wasm bundle from ./pkg/.\n\n" +
      "Build it from the workspace root:\n" +
      "    wasm-pack build crates/sim-wasm --target web --out-dir ../../web/pkg\n\n" +
      "then reload this page. (The bundle is a build artifact and is not committed.)\n\n" +
      `Underlying error: ${e}`,
  );
  throw e;
}

// ---------------------------------------------------------------------------
// Encodings
// ---------------------------------------------------------------------------

/** Externally-tagged `Demand`, the way both backends spell it. */
function demandJson(mode, value) {
  return mode === "Rest" ? '"Rest"' : JSON.stringify({ [mode]: value });
}

/**
 * `EventFlags` arrives as `"OV | PLATING_RISK"`, or `""` for none.
 *
 * The `filter` is the whole point: `"".split(" | ")` yields `[""]`, so without it an
 * unremarkable step renders a flag chip with no name on it.
 */
function parseFlags(s) {
  return (s || "").split(" | ").filter((f) => f.length > 0);
}

const SEVERE = new Set(["THERMAL_RUNAWAY", "VENTED", "CONTACTOR_OPEN", "PLATING_RISK"]);

const K = 273.15;
const toC = (k) => k - K;

// ---------------------------------------------------------------------------
// Backends
// ---------------------------------------------------------------------------

/** The engine, in this tab. */
class WasmBackend {
  static label = "in-page (wasm)";

  constructor(sim) {
    this.sim = sim;
  }

  static async create(scenarioText) {
    const id = wasm.chemistry_id_of(scenarioText);
    let chemText;
    if (id !== undefined) {
      // No filesystem in a browser: the chemistry arrives as text, fetched from the
      // directory the server hands out at /chemistries. Asking Rust which file to
      // fetch keeps TOML parsing out of this file entirely.
      const res = await fetch(`/chemistries/${id}.toml`);
      if (!res.ok) throw new Error(`GET /chemistries/${id}.toml -> ${res.status}`);
      chemText = await res.text();
    }
    return new WasmBackend(new wasm.Sim(scenarioText, chemText));
  }

  facts() {
    return JSON.parse(this.sim.facts());
  }

  setEnv(tAmbientK) {
    this.sim.set_env(tAmbientK, undefined);
  }

  step(dt, nSteps, demand, reportEvery) {
    return JSON.parse(this.sim.step_many(dt, nSteps, demand, reportEvery));
  }

  restart(bmsEnabled) {
    this.sim.restart(bmsEnabled);
  }

  snapshot() {
    return this.sim.snapshot();
  }

  restore(json) {
    this.sim.restore(json);
  }

  close() {
    // wasm-bindgen objects hold linear memory until dropped explicitly.
    this.sim.free();
  }
}

/**
 * The engine, behind `sim-server`.
 *
 * Exists so the WebSocket protocol has a live client and does not rot — the page does
 * not need it for physics. One socket round trip per animation frame, which is exactly
 * the cost the batch-stepping design exists to make optional: `dt` is still the
 * page's, and `n_steps` still comes from the accumulator, so the *trajectory* is
 * identical to the in-page one however long the network takes.
 */
class SocketBackend {
  static label = "server (WebSocket)";

  constructor(sessionId, socket, hello) {
    this.sessionId = sessionId;
    this.socket = socket;
    this.hello = hello;
    this.pending = null;
    // Commands are serialised through a promise chain rather than rejected when one is
    // already in flight. Without it, dragging the ambient slider during a batch would
    // race the `Step` that is outstanding — and the server's rule is that commands are
    // never dropped and never reordered, so the client should not be the thing that
    // breaks it.
    this.queue = Promise.resolve();
    this.frames = [];
    this.factsCache = {
      series: hello.pack.series,
      parallel: hello.pack.parallel,
      sim_time_s: hello.pack.sim_time_s,
      // The server has no "rebuild without the BMS" operation and this page will not
      // rewrite someone's scenario to fake one, so the toggle is inert here. Reported
      // honestly rather than guessed: a socket session is whatever the scenario said.
      has_bms: null,
      scenario_has_bms: null,
      sensor_faults_dropped: 0,
    };

    socket.onmessage = (ev) => this.#onEvent(JSON.parse(ev.data));
    socket.onclose = () => this.#reject(new Error("the server closed the socket"));
    socket.onerror = () => this.#reject(new Error("socket error"));
  }

  static async create(scenarioText) {
    const res = await fetch("/sessions", {
      method: "POST",
      headers: { "Content-Type": "application/toml" },
      body: scenarioText,
    });
    if (!res.ok) throw new Error(`POST /sessions -> ${res.status}: ${await res.text()}`);
    const { id } = await res.json();

    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    const socket = new WebSocket(`${proto}//${location.host}/sessions/${id}/ws`);
    const hello = await new Promise((resolve, reject) => {
      socket.onopen = () => {};
      socket.onerror = () => reject(new Error("could not open the WebSocket"));
      socket.onmessage = (ev) => {
        const event = JSON.parse(ev.data);
        if (event.Hello) resolve(event.Hello);
        else reject(new Error(`expected Hello first, got ${Object.keys(event)[0]}`));
      };
    });
    if (hello.role !== "writer") {
      throw new Error("this session already has a writer; another tab is driving it");
    }
    return new SocketBackend(id, socket, hello);
  }

  #reject(err) {
    if (this.pending) {
      this.pending.reject(err);
      this.pending = null;
    }
  }

  #onEvent(event) {
    const [tag, body] = Object.entries(event)[0];
    switch (tag) {
      case "Telemetry":
        this.frames.push(body);
        break;
      case "BatchComplete":
        this.factsCache.sim_time_s = body.sim_time_s;
        this.#resolve(this.frames.splice(0));
        break;
      case "Snapshot":
        this.#resolve(JSON.stringify(body.snapshot));
        break;
      case "Restored":
        this.factsCache.sim_time_s = body.pack.sim_time_s;
        this.#resolve(null);
        break;
      case "EnvSet":
      case "Pong":
      case "FaultsCleared":
      case "FaultScheduled":
      case "BmsFaultCleared":
        this.#resolve(null);
        break;
      case "Dropped":
        // A writer never sees this — its batch replies are the experiment's record.
        // If it ever arrives, say so rather than drawing a smooth line through a hole.
        showBanner(`the server dropped ${body.count} frames`, "info");
        break;
      case "Error":
        this.#reject(new Error(`${body.code}: ${body.message}`));
        break;
      default:
        showBanner(`unrecognised event from the server: ${tag}`, "info");
    }
  }

  #resolve(value) {
    if (this.pending) {
      this.pending.resolve(value);
      this.pending = null;
    }
  }

  /** One command, one reply. The socket is strictly request/response for a writer. */
  #send(command) {
    const sent = this.queue.then(
      () =>
        new Promise((resolve, reject) => {
          this.pending = { resolve, reject };
          this.socket.send(JSON.stringify(command));
        }),
    );
    // The chain must survive a rejected command, or one error wedges the socket for
    // the rest of the session. The caller still sees the rejection through `sent`.
    this.queue = sent.catch(() => {});
    return sent;
  }

  facts() {
    return this.factsCache;
  }

  setEnv(tAmbientK) {
    return this.#send({ SetEnv: { env: { t_ambient: tAmbientK, t_coolant: null } } });
  }

  step(dt, nSteps, demand, reportEvery) {
    return this.#send({
      Step: {
        dt,
        n_steps: nSteps,
        demand: JSON.parse(demand),
        report_every_n_steps: reportEvery,
      },
    });
  }

  /**
   * Unreachable, and a guard rather than dead code.
   *
   * There is no "rebuild without the BMS" over the wire, and there should not be: the
   * server builds the pack the scenario asked for. Restarting a socket session means
   * throwing the session away and creating another, which the page does by reloading
   * the scenario — see the Restart handler. If this ever fires, that fork is missing.
   */
  restart() {
    throw new Error(
      "a socket session restarts by being replaced, not rebuilt — reload the scenario",
    );
  }

  snapshot() {
    return this.#send("Snapshot");
  }

  restore(json) {
    return this.#send({ Restore: { snapshot: JSON.parse(json) } });
  }

  close() {
    this.socket.onclose = null;
    this.socket.close();
    // Best effort: the tab may be going away. `keepalive` is what makes the request
    // survive an unload, which is when this matters most.
    fetch(`/sessions/${this.sessionId}`, { method: "DELETE", keepalive: true }).catch(
      () => {},
    );
  }
}

// ---------------------------------------------------------------------------
// Plotting — hand-rolled, because this is a polyline and two axes
// ---------------------------------------------------------------------------

const PLOT_BG = "#1c1f26";
const PLOT_GRID = "#2b303b";
const PLOT_INK = "#939bab";

/** Nice-ish tick step for a range: 1, 2 or 5 times a power of ten. */
function tickStep(span, target) {
  if (!(span > 0)) return 1;
  const raw = span / target;
  const mag = 10 ** Math.floor(Math.log10(raw));
  for (const m of [1, 2, 5, 10]) {
    if (raw <= m * mag) return m * mag;
  }
  return 10 * mag;
}

function fitCanvas(canvas) {
  const ratio = window.devicePixelRatio || 1;
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (canvas.width !== Math.round(w * ratio) || canvas.height !== Math.round(h * ratio)) {
    canvas.width = Math.round(w * ratio);
    canvas.height = Math.round(h * ratio);
  }
  const ctx = canvas.getContext("2d");
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  return { ctx, w, h };
}

/**
 * Draw one panel: shared x (simulation time), one or more y series in one unit.
 *
 * `traces` is `[{label, color, values}]` where `values` is index-aligned with `times`.
 * A `null` value is a hole — the line breaks rather than interpolating across it,
 * which is what makes "the BMS has no estimate" look different from "the estimate is
 * zero".
 */
function drawPanel(canvas, title, unit, times, traces, yFixed) {
  const { ctx, w, h } = fitCanvas(canvas);
  const padL = 54;
  const padR = 10;
  const padT = 20;
  const padB = 22;
  const plotW = Math.max(1, w - padL - padR);
  const plotH = Math.max(1, h - padT - padB);

  ctx.fillStyle = PLOT_BG;
  ctx.fillRect(0, 0, w, h);

  ctx.font = "11px ui-monospace, Menlo, Consolas, monospace";
  ctx.textBaseline = "middle";

  if (times.length === 0) {
    ctx.fillStyle = PLOT_INK;
    ctx.textAlign = "center";
    ctx.fillText(`${title} — no samples yet`, w / 2, h / 2);
    return;
  }

  const x0 = times[0];
  const xLast = times[times.length - 1];
  // A single sample — which is what a freshly loaded or restored session has, from the
  // zero-length read — has no time span. Widening it by an epsilon and ticking anyway
  // prints the same label six times, which looks like a broken axis rather than an
  // empty one.
  const spanned = xLast > x0;
  const x1 = spanned ? xLast : x0 + 1;

  let y0;
  let y1;
  if (yFixed) {
    [y0, y1] = yFixed;
  } else {
    y0 = Infinity;
    y1 = -Infinity;
    for (const t of traces) {
      for (const v of t.values) {
        if (v === null || !Number.isFinite(v)) continue;
        if (v < y0) y0 = v;
        if (v > y1) y1 = v;
      }
    }
    if (!Number.isFinite(y0)) [y0, y1] = [0, 1];
    const pad = (y1 - y0) * 0.08 || Math.max(Math.abs(y1) * 0.05, 0.5);
    y0 -= pad;
    y1 += pad;
  }

  const sx = (t) => padL + ((t - x0) / (x1 - x0)) * plotW;
  const sy = (v) => padT + plotH - ((v - y0) / (y1 - y0 || 1)) * plotH;

  // Grid and y labels.
  ctx.strokeStyle = PLOT_GRID;
  ctx.fillStyle = PLOT_INK;
  ctx.lineWidth = 1;
  ctx.textAlign = "right";
  const yStep = tickStep(y1 - y0, 4);
  // Decimals from the tick step, not a fixed 2. A cell-voltage panel can span 12 mV,
  // and two ticks 5 mV apart both render as "3.23" at fixed precision — a labelled
  // axis that repeats itself is worse than none.
  const decimals = Math.min(6, Math.max(0, Math.ceil(-Math.log10(yStep))));
  for (let v = Math.ceil(y0 / yStep) * yStep; v <= y1; v += yStep) {
    const y = Math.round(sy(v)) + 0.5;
    ctx.beginPath();
    ctx.moveTo(padL, y);
    ctx.lineTo(w - padR, y);
    ctx.stroke();
    ctx.fillText(v.toFixed(decimals), padL - 6, y);
  }

  // x labels.
  ctx.textAlign = "center";
  if (spanned) {
    const xStep = tickStep(x1 - x0, 6);
    for (let t = Math.ceil(x0 / xStep) * xStep; t <= x1; t += xStep) {
      const x = Math.round(sx(t)) + 0.5;
      ctx.beginPath();
      ctx.moveTo(x, padT);
      ctx.lineTo(x, padT + plotH);
      ctx.stroke();
      ctx.fillText(fmtTime(t), x, h - padB / 2);
    }
  } else {
    ctx.fillText(fmtTime(x0), padL, h - padB / 2);
  }

  // One point per pixel column is plenty; a long run has far more samples than that.
  const stride = Math.max(1, Math.floor(times.length / (plotW * 2)));

  for (const trace of traces) {
    ctx.strokeStyle = trace.color;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    let drawing = false;
    for (let i = 0; i < times.length; i += stride) {
      const v = trace.values[i];
      if (v === null || !Number.isFinite(v)) {
        drawing = false;
        continue;
      }
      const x = sx(times[i]);
      const y = sy(v);
      if (drawing) ctx.lineTo(x, y);
      else {
        ctx.moveTo(x, y);
        drawing = true;
      }
    }
    ctx.stroke();
  }

  // Title on the left, legend right-aligned against the plot's right edge. Laying the
  // legend out left-to-right after the title reads fine until a panel has two long
  // labels, and then it runs off the canvas with no scrollbar to reveal it — measure
  // first, place second.
  ctx.textAlign = "left";
  ctx.fillStyle = PLOT_INK;
  const heading = `${title} [${unit}]`;
  ctx.fillText(heading, padL, 10);

  const swatch = 10;
  const gap = 6;
  const between = 14;
  const legendW = traces.reduce(
    (sum, t) => sum + swatch + gap + ctx.measureText(t.label).width + between,
    -between,
  );
  // Clamped to the plot area rather than to "after the title": a long title must not
  // be able to push the legend off the canvas, because a canvas has no scrollbar to
  // reveal what fell off it. If the two ever collide the title is the one that can be
  // read from the axis anyway.
  let lx = Math.max(padL, w - padR - legendW);
  for (const trace of traces) {
    ctx.fillStyle = trace.color;
    ctx.fillRect(lx, 8, swatch, 3);
    ctx.fillText(trace.label, lx + swatch + gap, 10);
    lx += swatch + gap + ctx.measureText(trace.label).width + between;
  }
}

function fmtTime(s) {
  if (s < 120) return `${s.toFixed(0)}s`;
  if (s < 7200) return `${(s / 60).toFixed(0)}m`;
  if (s < 172800) return `${(s / 3600).toFixed(1)}h`;
  return `${(s / 86400).toFixed(1)}d`;
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

/** Everything the plots read, kept column-wise so a trace is already an array. */
const MAX_SAMPLES = 200_000;
const history = {
  t: [],
  v_terminal: [],
  v_cell_min: [],
  v_cell_max: [],
  i_actual: [],
  i_internal_short_a: [],
  soc_true: [],
  soc_bms: [],
  t_min: [],
  t_max: [],
};

function resetHistory() {
  for (const key of Object.keys(history)) history[key].length = 0;
}

function record(frame) {
  const m = frame.telemetry;
  history.t.push(frame.sim_time_s);
  history.v_terminal.push(m.v_terminal);
  history.v_cell_min.push(m.v_cell_min);
  history.v_cell_max.push(m.v_cell_max);
  history.i_actual.push(m.i_actual);
  history.i_internal_short_a.push(m.i_internal_short_a);
  history.soc_true.push(m.soc_true * 100);
  // `null` stays `null`: no BMS means no estimate, and the line must break rather than
  // dive to zero.
  history.soc_bms.push(m.soc_bms === null ? null : m.soc_bms * 100);
  history.t_min.push(toC(m.t_min));
  history.t_max.push(toC(m.t_max));

  if (history.t.length > MAX_SAMPLES) {
    // Drop the oldest tenth in one go rather than shifting every sample.
    const drop = Math.floor(MAX_SAMPLES / 10);
    for (const key of Object.keys(history)) history[key].splice(0, drop);
  }
}

// ---------------------------------------------------------------------------
// Readouts
// ---------------------------------------------------------------------------

const READOUTS = [
  ["sim time", (m, f) => fmtTime(f.sim_time_s)],
  ["terminal", (m) => `${m.v_terminal.toFixed(3)} V`],
  ["current", (m) => `${m.i_actual.toFixed(3)} A`],
  ["soc (true)", (m) => `${(m.soc_true * 100).toFixed(1)} %`],
  ["soc (bms)", (m) => (m.soc_bms === null ? null : `${(m.soc_bms * 100).toFixed(1)} %`)],
  ["cell v", (m) => `${m.v_cell_min.toFixed(3)} / ${m.v_cell_max.toFixed(3)} V`],
  ["cell t", (m) => `${toC(m.t_min).toFixed(1)} / ${toC(m.t_max).toFixed(1)} °C`],
  ["heat", (m) => `${m.q_gen_w.toFixed(2)} W`],
  ["soh cap", (m) => `${(m.soh_capacity * 100).toFixed(2)} %`],
  ["soh res", (m) => `${m.soh_resistance.toFixed(4)} ×`],
  ["balancing", (m) => `${m.q_balancing_w.toFixed(3)} W`],
  ["short (int)", (m) => `${m.i_internal_short_a.toFixed(3)} A`],
];

const readoutEls = new Map();
{
  const host = $("readouts");
  for (const [key] of READOUTS) {
    const el = document.createElement("div");
    el.className = "readout";
    el.innerHTML = `<div class="k"></div><div class="v">—</div>`;
    el.querySelector(".k").textContent = key;
    host.appendChild(el);
    readoutEls.set(key, el.querySelector(".v"));
  }
}

function renderReadouts(telemetry, facts) {
  for (const [key, fn] of READOUTS) {
    const el = readoutEls.get(key);
    const value = telemetry ? fn(telemetry, facts) : null;
    if (value === null || value === undefined) {
      el.textContent = telemetry ? "no BMS" : "—";
      el.className = "v none";
    } else {
      el.textContent = value;
      el.className = "v";
    }
  }
}

function renderFlags(telemetry) {
  const host = $("flags");
  host.replaceChildren();
  const names = telemetry ? parseFlags(telemetry.flags) : [];
  if (names.length === 0) {
    const el = document.createElement("span");
    el.className = "flag none";
    el.textContent = telemetry ? "no flags" : "not started";
    host.appendChild(el);
    return;
  }
  for (const name of names) {
    const el = document.createElement("span");
    el.className = SEVERE.has(name) ? "flag severe" : "flag";
    el.textContent = name;
    host.appendChild(el);
  }
}

// ---------------------------------------------------------------------------
// The run loop
// ---------------------------------------------------------------------------

const state = {
  backend: null,
  scenarioText: null,
  running: false,
  busy: false,
  accumulator: 0,
  lastWallMs: null,
  latest: null,
  facts: null,
};

/**
 * Steps this frame, from wall-clock elapsed and the speed multiplier.
 *
 * This is the accumulator `CLAUDE.md` prescribes, and it lives here — on the client —
 * for the reason stated there: **the frame rate must never define the timestep**. `dt`
 * is fixed and configured; only the number of steps varies with how long the last
 * frame took. A stall makes the next frame take more steps, not a longer step.
 */
function stepsForFrame(nowMs, dt, speed) {
  if (state.lastWallMs === null) {
    state.lastWallMs = nowMs;
    return 0;
  }
  const elapsed = Math.min((nowMs - state.lastWallMs) / 1000, 0.25); // cap after a stall
  state.lastWallMs = nowMs;
  state.accumulator += elapsed * speed;
  const n = Math.floor(state.accumulator / dt);
  if (n <= 0) return 0;
  state.accumulator -= n * dt;
  return n;
}

/** The frame cap the engine enforces, mirrored so we decimate instead of being refused. */
const MAX_FRAMES_PER_CALL = 10_000;
const MAX_STEPS_PER_CALL = 1_000_000;

async function advance(nSteps) {
  const dt = Math.max(1e-6, Number($("dt").value) || 0.5);
  const mode = $("demand-mode").value;
  const demand = demandJson(mode, Number($("demand-value").value) || 0);

  const steps = Math.min(nSteps, MAX_STEPS_PER_CALL);
  // At high speed multipliers a frame can be thousands of steps. Decimating keeps the
  // reply small; it drops *reports*, never steps, so the trajectory is untouched. 300
  // frames is already more than a plot this size can resolve.
  const reportEvery = Math.max(1, Math.ceil(steps / 300));
  if (Math.ceil(steps / reportEvery) > MAX_FRAMES_PER_CALL) {
    throw new Error("frame cap exceeded — this is a bug in the page's decimation");
  }

  const frames = await state.backend.step(dt, steps, demand, reportEvery);
  for (const frame of frames) record(frame);
  if (frames.length > 0) {
    state.latest = frames[frames.length - 1].telemetry;
    state.facts = state.backend.facts();
  }
}

/**
 * Read the pack without moving it, so a freshly loaded or freshly restored session
 * shows what it *is* instead of a row of dashes until someone presses Run.
 *
 * This is what the engine's zero-length-step contract is for: `dt = 0` does not mutate
 * state, and the protocol allows it precisely so a client can sample on connect. Note
 * the frame is not a copy of the previous one — telemetry is computed from
 * start-of-step state, so a zero-length read answers "what is the pack doing now",
 * which is a different question from "what was it doing during the last step".
 *
 * It uses the *current* demand rather than `Rest`, because "what would this pack read
 * under the load I have dialled in" is the useful question.
 */
async function readNow() {
  if (!state.backend) return;
  const mode = $("demand-mode").value;
  const demand = demandJson(mode, Number($("demand-value").value) || 0);
  const frames = await state.backend.step(0, 1, demand, 1);
  for (const frame of frames) record(frame);
  if (frames.length > 0) {
    state.latest = frames[frames.length - 1].telemetry;
    state.facts = state.backend.facts();
  }
  // Paint now rather than waiting for the next animation frame. Usually that is 16 ms
  // and nobody notices — but a backgrounded tab has no animation frames at all, and a
  // page that loads into a row of dashes and stays there looks broken rather than
  // paused.
  draw();
}

function draw() {
  drawPanel($("plot-v"), "pack terminal", "V", history.t, [
    { label: "terminal", color: "#5ac8fa", values: history.v_terminal },
  ]);
  // Its own axis, so the spread between the best and worst cell stays legible however
  // many cells are in series.
  drawPanel($("plot-cv"), "cell voltage", "V", history.t, [
    { label: "min", color: "#7ddc7d", values: history.v_cell_min },
    { label: "max", color: "#ffb454", values: history.v_cell_max },
  ]);
  drawPanel($("plot-i"), "current (discharge +)", "A", history.t, [
    { label: "pack", color: "#5ac8fa", values: history.i_actual },
    { label: "int. short", color: "#ff6b6b", values: history.i_internal_short_a },
  ]);
  drawPanel(
    $("plot-soc"),
    "state of charge",
    "%",
    history.t,
    [
      { label: "truth", color: "#5ac8fa", values: history.soc_true },
      { label: "bms est.", color: "#ffb454", values: history.soc_bms },
    ],
    [0, 100],
  );
  drawPanel($("plot-t"), "cell temperature", "°C", history.t, [
    { label: "min", color: "#5ac8fa", values: history.t_min },
    { label: "max", color: "#ff6b6b", values: history.t_max },
  ]);

  renderReadouts(state.latest, state.facts ?? { sim_time_s: 0 });
  renderFlags(state.latest);
}

async function frame(nowMs) {
  requestAnimationFrame(frame);
  if (!state.backend) return;

  if (state.running && !state.busy) {
    const dt = Math.max(1e-6, Number($("dt").value) || 0.5);
    const speed = 10 ** Number($("speed").value);
    const n = stepsForFrame(nowMs, dt, speed);
    if (n > 0) {
      state.busy = true;
      try {
        await advance(n);
        clearBanner();
      } catch (e) {
        state.running = false;
        $("run").textContent = "Run";
        showBanner(String(e.message ?? e));
      } finally {
        state.busy = false;
      }
    }
  }
  draw();
}
requestAnimationFrame(frame);

// ---------------------------------------------------------------------------
// Controls
// ---------------------------------------------------------------------------

async function loadScenario() {
  const name = $("scenario").value;
  state.running = false;
  $("run").textContent = "Run";
  state.lastWallMs = null;
  state.accumulator = 0;
  state.latest = null;

  if (state.backend) {
    state.backend.close();
    state.backend = null;
  }
  resetHistory();
  clearBanner();

  try {
    const res = await fetch(`/scenarios/${name}`);
    if (!res.ok) throw new Error(`GET /scenarios/${name} -> ${res.status}`);
    state.scenarioText = await res.text();

    const Backend = $("use-socket").checked ? SocketBackend : WasmBackend;
    state.backend = await Backend.create(state.scenarioText);
    state.facts = state.backend.facts();
    applyEnv();
    afterFactsChange(Backend.label);
    await readNow();
  } catch (e) {
    showBanner(String(e.message ?? e));
  }
}

function afterFactsChange(label) {
  const f = state.facts;
  $("scenario-note").textContent =
    `${f.series}S${f.parallel}P · ${label}` +
    (f.sensor_faults_dropped
      ? ` · ${f.sensor_faults_dropped} sensor fault(s) dropped: no BMS to sense them`
      : "");

  const socket = $("use-socket").checked;
  const bms = $("bms");
  bms.disabled = socket || f.scenario_has_bms === false;
  if (f.has_bms !== null) bms.checked = f.has_bms;
  $("bms-note").textContent = socket
    ? "in-page only: the server builds the pack the scenario asked for, and this page will not rewrite it"
    : f.scenario_has_bms === false
      ? "this scenario ships no BMS, so there is nothing to switch back on"
      : "toggling rebuilds the pack from the scenario — the run restarts at t = 0";
}

function applyEnv() {
  const tC = Number($("ambient").value);
  $("ambient-label").textContent = `${tC.toFixed(1)} °C`;
  if (!state.backend) return;
  try {
    // Sync in the wasm backend, a queued command in the socket one; `Promise.resolve`
    // flattens both so the caller does not have to care which is loaded.
    Promise.resolve(state.backend.setEnv(tC + K)).catch((e) =>
      showBanner(String(e.message ?? e)),
    );
  } catch (e) {
    showBanner(String(e.message ?? e));
  }
}

$("load").onclick = loadScenario;
$("scenario").onchange = loadScenario;
$("use-socket").onchange = loadScenario;

$("reset").onclick = async () => {
  if (!state.backend) return;
  if ($("use-socket").checked) {
    // A socket session has no rebuild: restarting means a new session from the same
    // scenario, which is exactly what loading it again does.
    await loadScenario();
    return;
  }
  try {
    state.backend.restart($("bms").checked);
    resetHistory();
    state.latest = null;
    state.accumulator = 0;
    state.lastWallMs = null;
    state.facts = state.backend.facts();
    afterFactsChange(
      $("use-socket").checked ? SocketBackend.label : WasmBackend.label,
    );
    applyEnv();
    await readNow();
    clearBanner();
  } catch (e) {
    showBanner(String(e.message ?? e));
  }
};

$("bms").onchange = () => $("reset").click();

$("run").onclick = () => {
  state.running = !state.running;
  state.lastWallMs = null;
  $("run").textContent = state.running ? "Pause" : "Run";
};

$("stepone").onclick = async () => {
  if (!state.backend || state.busy) return;
  state.busy = true;
  try {
    await advance(1);
    clearBanner();
  } catch (e) {
    showBanner(String(e.message ?? e));
  } finally {
    state.busy = false;
  }
};

$("ambient").oninput = applyEnv;

$("speed").oninput = () => {
  const speed = 10 ** Number($("speed").value);
  $("speed-label").textContent = speed >= 100 ? `${speed.toFixed(0)}×` : `${speed.toFixed(1)}×`;
};
$("speed").oninput();

$("save").onclick = async () => {
  if (!state.backend) return;
  try {
    const json = await state.backend.snapshot();
    const url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `batsim-v${wasm.snapshot_version()}-t${Math.round(
      state.facts?.sim_time_s ?? 0,
    )}s.json`;
    a.click();
    URL.revokeObjectURL(url);
  } catch (e) {
    showBanner(String(e.message ?? e));
  }
};

$("restore").onclick = () => $("restore-file").click();
$("restore-file").onchange = async (ev) => {
  const file = ev.target.files?.[0];
  ev.target.value = "";
  if (!file || !state.backend) return;
  try {
    await state.backend.restore(await file.text());
    resetHistory();
    state.latest = null;
    state.accumulator = 0;
    state.lastWallMs = null;
    state.facts = state.backend.facts();
    // Without this the page shows a row of dashes after a successful restore, which
    // reads as "nothing happened" — the one impression a restore must not give.
    await readNow();
    clearBanner();
  } catch (e) {
    showBanner(String(e.message ?? e));
  }
};

window.addEventListener("beforeunload", () => state.backend?.close());

await loadScenario();
