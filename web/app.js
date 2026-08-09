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
//  * A **unit** variant crosses as a bare string, not an object — `"Rest"`, `"Pong"`,
//    `"PackCurrent"`, `"ClearFaults"`. Anything that destructures an incoming event or
//    builds an outgoing enum has to handle both shapes.
//  * Per-cell state is **not** telemetry and never arrives on a frame. It is
//    `Sim::cells()` in the tab and `GET /sessions/{id}/cells` over the network — one
//    shape, `{series, parallel, cells}`, series-major — and the page samples it on a
//    timer rather than per step.

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

/**
 * Lowest `sim_wasm::WASM_API_VERSION` this page can run against.
 *
 * Raised when the page starts *calling* something new, not every time the constant
 * moves: v3 added `Sim::sensors`, which the BMS panel below requires.
 */
const WASM_API_MIN = 3;

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

// `pkg/` is gitignored and rebuilt by hand, so this page can be newer than the wasm it
// loads — the one version pair in this workspace that can drift. Without this check the
// symptom of a stale bundle is `TypeError: sim.sensors is not a function` from somewhere
// in a render path, which names neither the cause nor the fix. Feature detection would
// answer "is the method there"; the version answers the useful question, which is "is my
// bundle old, and what do I run".
//
// Outside the `try` deliberately: inside it, the throw below is caught by the handler
// above and relabelled as a bundle that would not load, which is a different fault with
// a different fix. And it *throws* rather than only showing the banner, because boot
// continues into an automatic scenario load and that calls `clearBanner` — a warning
// erased 200 ms later is decoration, and this page genuinely cannot run.
if (wasm.wasm_api_version() < WASM_API_MIN) {
  const stale =
    `This page needs wasm api ${WASM_API_MIN} or newer, but ./pkg/ is api ` +
    `${wasm.wasm_api_version()} — the bundle is stale.\n\n` +
    "Rebuild it from the workspace root:\n" +
    "    wasm-pack build crates/sim-wasm --target web --out-dir ../../web/pkg\n\n" +
    "then reload. (The bundle is a build artifact and is not committed, so it does not " +
    "update when the Rust does.)";
  showBanner(stale);
  throw new Error(stale);
}

// ---------------------------------------------------------------------------
// Encodings
// ---------------------------------------------------------------------------

/** Externally-tagged `Demand`, the way both backends spell it. */
function demandJson(mode, value) {
  return mode === "Rest" ? '"Rest"' : JSON.stringify({ [mode]: value });
}

// ---------------------------------------------------------------------------
// CC-CV, the one demand the engine does not have
// ---------------------------------------------------------------------------

/**
 * How often the CC-CV controller is allowed to change its mind, in **simulation**
 * seconds. 10 s, the same period `[pack.aging]`'s sub-clock uses by default.
 *
 * This being a fixed period rather than "once per animation frame" is the whole design.
 * A frame delivering 500 steps and two frames delivering 200 + 300 would otherwise
 * evaluate the controller at different step indices, so the same scenario at the same
 * `dt` would take a different trajectory depending on how long a frame happened to take
 * — the frame rate defining the physics by the back door, which `CLAUDE.md` forbids
 * twice over (principle 3, and determinism in principle 4).
 *
 * Measured cost of the quantisation at the knee: +0.11 mV on NMC at 0.5 C, +0.84 mV on
 * the LG M50 cell at 1 C, and the end state unmoved in the fifth decimal of SOC. The
 * excursion is *not* monotone in the period — where the grid lands relative to the
 * crossing matters more than how long the window is — so do not read the number as a
 * trade-off curve. See docs/plans/cc-cv.md.
 */
const CCCV_PERIOD_S = 10;

/**
 * Half-width of the band the CC/CV comparison uses, in volts **per series cell**.
 *
 * Not decoration, and not a tolerance in the usual sense. The engine's `Voltage` solve
 * lands about 1.4e-4 V from the target rather than on it, and the residual shrinks as
 * the current tapers — so a plain `v >= target` comparison flips back to CC the moment
 * the residual changes sign. One CC step then re-establishes the whole
 * `I·R0 + ΣV_rc` overpotential that the voltage hold had let decay, and the next step
 * flips again. Measured on `nmc_21700_lgm50` at 1 C: **1311 flips, a terminal voltage
 * peaking 127 mV above a 4.20 V target, and a charge that reported itself done 1160 s
 * and 2.8 SOC points early** because one chattering step fell below the taper cutoff.
 *
 * 1 mV is three orders above that residual and below anything a reader can see: it moves
 * the knee 3.5 s earlier and the peak overshoot to +0.17 mV. Per *cell*, multiplied by
 * the series count, because the residual is a terminal-voltage quantity.
 */
const CCCV_BAND_V_PER_CELL = 0.001;

/** Flags that mean the current stopped because something intervened, not because it tapered. */
const PROTECTION_FLAGS = ["UT", "OT", "OV", "UV", "OC", "CONTACTOR_OPEN"];

/** What the three CC-CV fields currently say. */
function ccCvConfig() {
  return {
    i_charge: Math.abs(Number($("cccv-i").value) || 0),
    v_cell: Number($("cccv-v").value) || 0,
    i_taper: Math.abs(Number($("cccv-taper").value) || 0),
  };
}

/**
 * The controller, and it is deliberately **memoryless**: which leg to run is a function
 * of the last telemetry frame and nothing else.
 *
 * That property is worth more than it looks. A CC/CV phase flag would be client state
 * that no `Snapshot` carries, so restoring a session would resurrect whichever leg the
 * page happened to be in — and the reset would have to be threaded through load,
 * restart and restore, the same list the load generation counter already guards. With no
 * state there is nothing to get wrong: the pack's own voltage says which leg it is in.
 *
 * `series` scales both the target and the band, so the fields stay per-cell and a
 * scenario with a different topology needs no re-entry. Positive `i_charge` becomes a
 * negative demand: on this page positive is discharge everywhere, and a charger charges.
 */
function ccCvDemand(vTerminal, cfg, series) {
  const target = cfg.v_cell * series;
  const band = CCCV_BAND_V_PER_CELL * series;
  return vTerminal < target - band
    ? demandJson("Current", -cfg.i_charge)
    : demandJson("Voltage", target);
}

/**
 * Is the charge over, and what ended it?
 *
 * `|i| <= taper` alone is not an answer, because the same condition arrives three ways
 * and one of them is a pack that is *not* charged: a BMS charge inhibit takes the
 * current to zero at whatever SOC the pack happened to have reached — measured at 60.8 %
 * on `cc_cv_charge_pack.toml` cooled to −5 °C. `i_actual` is the post-BMS current, which
 * is exactly why the flags have to be read beside it. Calling that "charged" would be
 * the page lying about the one thing this mode exists to show.
 */
function ccCvDone(telemetry, cfg, series) {
  if (!telemetry || Math.abs(telemetry.i_actual) > cfg.i_taper) return null;
  const flags = parseFlags(telemetry.flags);
  const tripped = flags.filter((f) => PROTECTION_FLAGS.includes(f));
  if (tripped.length > 0) {
    return { ok: false, why: `stopped by the BMS — ${tripped.join(", ")}` };
  }
  const target = cfg.v_cell * series;
  const band = CCCV_BAND_V_PER_CELL * series;
  if (telemetry.v_terminal < target - band) {
    // The charge current is at or below the cutoff, so the very first window "finishes".
    return {
      ok: false,
      why:
        `the charge current (${cfg.i_charge.toFixed(3)} A) is not above the cutoff ` +
        `(${cfg.i_taper.toFixed(3)} A), so this stopped before it charged anything`,
    };
  }
  return { ok: true, why: `complete — tapered to ${Math.abs(telemetry.i_actual).toFixed(3)} A` };
}

// ---------------------------------------------------------------------------
// The pulse train, which is CC-CV's machinery with the feedback taken out
// ---------------------------------------------------------------------------

/** What the three pulse fields currently say. `i` is signed: positive discharges. */
function pulseConfig() {
  return {
    i: Number($("pulse-i").value) || 0,
    t_on: Math.max(0, Number($("pulse-on").value) || 0),
    t_off: Math.max(0, Number($("pulse-off").value) || 0),
  };
}

/**
 * Where in the train a given simulation time falls, **in steps**.
 *
 * Steps rather than seconds, and this is the whole reason the function exists. An edge
 * has to land on a step boundary that the simulation agrees on, not on whichever step a
 * browser frame happened to end at — `CLAUDE.md` principle 3, the same rule
 * `CCCV_PERIOD_S` is quantised by. Working in seconds and comparing floats would put the
 * edge half a step out whenever `t_on` is not an exact multiple of `dt`, and *which* way
 * it rounded would depend on accumulated floating-point drift in `sim_time_s`.
 *
 * Returns `null` for a degenerate train (both legs zero), which the caller treats as a
 * plain rest — there is no sensible edge in a period of no length.
 */
function pulsePhase(simTimeS, cfg, dt) {
  const kOn = Math.round(cfg.t_on / dt);
  const kOff = Math.round(cfg.t_off / dt);
  const kPeriod = kOn + kOff;
  if (kPeriod <= 0) return null;
  const index = Math.round(simTimeS / dt);
  const phase = ((index % kPeriod) + kPeriod) % kPeriod;
  const on = phase < kOn;
  return {
    on,
    kPeriod,
    /** Steps until the leg changes — what `advance` chops the frame at. */
    toEdge: on ? kOn - phase : kPeriod - phase,
    /** 1-based, and it keeps counting across a pause because it is derived from time. */
    pulse: Math.floor(index / kPeriod) + 1,
  };
}

/**
 * The demand itself: `Current` on the on-leg, `Rest` otherwise.
 *
 * A **pure function of simulation time**, which is strictly stronger than CC-CV's
 * memorylessness. CC-CV reads back the pack's own voltage and therefore needed a band
 * sized by the solver's residual; this reads back nothing at all, so there is no
 * residual, no chatter, and nothing a restore could resurrect — a snapshot carries
 * `sim_time_s`, and `sim_time_s` is the entire state of this controller.
 */
function pulseDemand(phase, cfg) {
  return phase !== null && phase.on ? demandJson("Current", cfg.i) : demandJson("Rest", 0);
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

  /** The engine's own per-call caps, read from the module rather than restated here. */
  limits() {
    return { maxSteps: wasm.max_steps_per_call(), maxFrames: wasm.max_frames_per_call() };
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

  /** Ground truth for every cell: `{series, parallel, cells}`, series-major. */
  cells() {
    return JSON.parse(this.sim.cells());
  }

  /** What the BMS measured, or `null` on a pack with no BMS. */
  sensors() {
    return JSON.parse(this.sim.sensors());
  }

  scheduleFault(atS, fault) {
    this.sim.schedule_fault(atS, JSON.stringify(fault));
  }

  clearFaults() {
    return this.sim.clear_faults();
  }

  clearBmsFault() {
    return this.sim.clear_bms_fault();
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
    // A unit variant crosses as a bare string (`"Pong"`), not an object — externally
    // tagged serde has no field to hang on it. Destructuring `Object.entries` on a
    // string yields `["0", "P"]`, so without this fork the `Pong` arm below is
    // unreachable and a liveness reply would land in `default` as an unrecognised
    // event. The page never sends `Ping` — only an observer may — so this has never
    // fired, but the arm was written to handle something it could not have received.
    const [tag, body] = typeof event === "string" ? [event, null] : Object.entries(event)[0];
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
        this.#resolve(null);
        break;
      // These three carry their outcome — `count`, `cleared`, `at_s` — and the panel
      // reports it. Resolving `null` here would make "cleared 3 faults" and "there
      // were none" the same visible result.
      case "FaultsCleared":
      case "FaultScheduled":
      case "BmsFaultCleared":
        this.#resolve(body);
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

  /**
   * The caps *this server* enforces, from the hello frame.
   *
   * Not the wasm module's constants, even though the defaults are the same numbers:
   * `sim-server`'s limits are configurable, so a server started with a lower cap would
   * reject batches a page sized against a hardcoded default — which is the exact
   * failure the hello frame reports `limits` to prevent.
   */
  limits() {
    return {
      maxSteps: this.hello.limits.max_steps_per_command,
      maxFrames: this.hello.limits.max_frames_per_reply,
    };
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

  /**
   * Ground truth for every cell, over REST rather than the socket.
   *
   * The WebSocket protocol has no per-cell command by design — a `Frame` carries
   * `Telemetry`, and per-cell arrays are the thing telemetry deliberately is not. So
   * this reads the session's REST view instead, which returns the identical
   * `{series, parallel, cells}` shape the wasm backend serialises. It costs a request,
   * which is why the grid samples on a timer rather than every animation frame.
   */
  async cells() {
    const res = await fetch(`/sessions/${this.sessionId}/cells`);
    if (!res.ok) throw new Error(`GET /sessions/${this.sessionId}/cells -> ${res.status}`);
    return res.json();
  }

  /**
   * What the BMS measured, over REST for the same reason `cells` is: the sensor frame
   * is per-group and per-probe, which is exactly what a `Frame` is not.
   *
   * Returns `null` for a pack with no BMS — the route answers the JSON literal, so this
   * needs no special case. Note the server's `API_VERSION` did **not** move for this
   * route: its own rule exempts additions, so a 404 here means an older server rather
   * than a version this page could have checked. See `WASM_API_MIN`, where the
   * asymmetry is the other way round.
   */
  async sensors() {
    const res = await fetch(`/sessions/${this.sessionId}/sensors`);
    if (!res.ok) throw new Error(`GET /sessions/${this.sessionId}/sensors -> ${res.status}`);
    return res.json();
  }

  async scheduleFault(atS, fault) {
    await this.#send({ ScheduleFault: { at_s: atS, fault } });
  }

  async clearFaults() {
    return (await this.#send("ClearFaults")).count;
  }

  async clearBmsFault() {
    return (await this.#send("ClearBmsFault")).cleared;
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
  // Chrome is sized from the box, not fixed: the pads below spend 42 px on axis furniture
  // regardless of height, which is a reasonable frame around a 150 px panel and most of
  // the picture in a 75 px one. One threshold rather than a continuous scale — a formula
  // would imply the chrome tracks the box, and it does not; it only gives way when it
  // would otherwise crowd out the trace.
  //
  // No panel on this page is currently short enough to take this branch (the grid is
  // 150 px, the one-column breakpoint 300). It is what lets index.html treat those
  // numbers as a free parameter, so it stays.
  const small = h < 110;
  const font = small ? 10 : 11;
  const padR = small ? 8 : 10;
  const padT = small ? 15 : 20;
  const padB = small ? 15 : 22;
  const plotH = Math.max(1, h - padT - padB);

  ctx.fillStyle = PLOT_BG;
  ctx.fillRect(0, 0, w, h);

  ctx.font = `${font}px ui-monospace, Menlo, Consolas, monospace`;
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

  // Ask for four ticks, then thin the axis until the labels clear each other. Lowering
  // the *target* instead is what suggests itself and it does not work: `tickStep` snaps
  // to 1, 2 or 5, so the same target yields 11 px spacing on one panel and 22 px on the
  // next depending on the span — asking for two gave the temperature panel a readable
  // pair and the terminal panel a single lonely gridline. The pixel gap is the thing
  // actually being protected, so it is the thing tested. A tall panel never trips this,
  // which is why full-bleed panels look exactly as they did.
  //
  // Doubling and not re-snapping: 2 → 4 → 8 leaves the axis on a round multiple of the
  // step it started from, whereas re-snapping to the 1/2/5 family can land on a step
  // that does not divide the one already drawn.
  let yStep = tickStep(y1 - y0, 4);
  const minLabelGap = font + 3;
  while (y1 > y0 && (plotH * yStep) / (y1 - y0) < minLabelGap) yStep *= 2;
  // Decimals from the tick step, not a fixed 2. A cell-voltage panel can span 12 mV,
  // and two ticks 5 mV apart both render as "3.23" at fixed precision — a labelled
  // axis that repeats itself is worse than none.
  const decimals = Math.min(6, Math.max(0, Math.ceil(-Math.log10(yStep))));
  const yTicks = [];
  for (let v = Math.ceil(y0 / yStep) * yStep; v <= y1; v += yStep) {
    yTicks.push({ v, text: v.toFixed(decimals) });
  }

  // The label gutter is measured, not assumed. `decimals` reaches 6 on a panel spanning
  // tens of microvolts — which a resting pack's cell voltage does — and "3.234567" is
  // 8 glyphs where a fixed gutter budgeted for 4. Too small and the axis runs into the
  // trace; the cost of measuring is one `measureText` per tick.
  const gap = 6;
  const padL = yTicks.reduce(
    (wide, t) => Math.max(wide, ctx.measureText(t.text).width + gap + 3),
    small ? 34 : 54,
  );
  const plotW = Math.max(1, w - padL - padR);

  const sx = (t) => padL + ((t - x0) / (x1 - x0)) * plotW;
  const sy = (v) => padT + plotH - ((v - y0) / (y1 - y0 || 1)) * plotH;

  // Grid and y labels.
  ctx.strokeStyle = PLOT_GRID;
  ctx.fillStyle = PLOT_INK;
  ctx.lineWidth = 1;
  ctx.textAlign = "right";
  for (const tick of yTicks) {
    const y = Math.round(sy(tick.v)) + 0.5;
    ctx.beginPath();
    ctx.moveTo(padL, y);
    ctx.lineTo(w - padR, y);
    ctx.stroke();
    ctx.fillText(tick.text, padL - gap, y);
  }

  // x labels. Also per available width, and for the same reason — but the constraint here
  // is the label, not the gridline: "104m" needs its own room on both sides.
  ctx.textAlign = "center";
  if (spanned) {
    const xStep = tickStep(x1 - x0, Math.max(2, Math.round(plotW / 150)));
    for (let t = Math.ceil(x0 / xStep) * xStep; t <= x1; t += xStep) {
      const x = Math.round(sx(t)) + 0.5;
      ctx.beginPath();
      ctx.moveTo(x, padT);
      ctx.lineTo(x, padT + plotH);
      ctx.stroke();
      // The gridline sits on the tick; the *label* is nudged back inside the canvas if
      // centring it there would hang it over the edge. A tick can land exactly on the
      // right edge — the last one usually does — and half of "3.3h" was being cut off
      // with nothing to scroll to reveal it. A label 4 px off its own line still reads
      // against that line; half a label reads as a different number.
      const label = fmtTime(t);
      const half = ctx.measureText(label).width / 2;
      const lxx = Math.min(Math.max(x, half + 1), w - half - 1);
      ctx.fillText(label, lxx, h - padB / 2);
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
  // Centred in the top pad, whatever that pad is, so the heading never overhangs the
  // plot's top gridline.
  const headY = padT / 2;
  ctx.fillText(heading, padL, headY);

  const swatch = small ? 8 : 10;
  const between = small ? 10 : 14;
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
    ctx.fillRect(lx, headY - 1.5, swatch, 3);
    ctx.fillText(trace.label, lx + swatch + gap, headY);
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
  // Both as "% of new", which is what lets them share one axis: capacity falls from
  // 100, resistance rises from it.
  soh_capacity: [],
  soh_resistance: [],
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
  history.soh_capacity.push(m.soh_capacity * 100);
  history.soh_resistance.push(m.soh_resistance * 100);

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
// The pack grid — one tile per cell, ground truth
// ---------------------------------------------------------------------------

/**
 * What a tile can show.
 *
 * Every entry is a field of `sim_core::CellView`. Two fields it does **not** have,
 * and the reason each is absent rather than forgotten:
 *
 *  * **per-cell voltage** — never computed per cell outside the solve;
 *  * **per-cell current** — Phase 6 slice D declined the accessor on purpose.
 *
 * So "which cell is taking the most load right now" is not on this menu. The SOC
 * spread is the same story integrated, which is why `soc` is the default.
 */
const METRICS = {
  soc: { ramp: "accent", get: (c) => c.soc * 100, dp: 2, unit: "%" },
  temp_k: { ramp: "warm", get: (c) => toC(c.temp_k), dp: 2, unit: "°C" },
  overpotential_v: { ramp: "accent", get: (c) => c.overpotential_v * 1000, dp: 1, unit: "mV" },
  soh_capacity: { ramp: "accent", get: (c) => c.soh_capacity * 100, dp: 3, unit: "%" },
  soh_resistance: { ramp: "warm", get: (c) => c.soh_resistance, dp: 4, unit: "×" },
  capacity_factor: { ramp: "accent", get: (c) => c.capacity_factor, dp: 3, unit: "×" },
  r0_factor: { ramp: "accent", get: (c) => c.r0_factor, dp: 3, unit: "×" },
  internal_short_conductance_s: {
    ramp: "warm",
    get: (c) => c.internal_short_conductance_s,
    dp: 4,
    unit: "S",
  },
};

/**
 * Sequential encodings: **one hue each, never a rainbow across a single scale.**
 *
 * The endpoints run dark → bright because the surface is dark; the light-mode
 * convention (light → dark) inverts here or the low end would glow. Two scales with
 * different hues is not a rainbow — a rainbow is several hues *within* one scale, and
 * that is what makes a heat map unreadable at the middle.
 */
const RAMPS = {
  // `flip`: past this point the tile is bright enough that light text on it loses to
  // dark text, so the ink flips to the page's darkest surface. It is per-ramp because
  // it is a luminance crossover, and green carries far more luminance per channel than
  // orange does — at the same t the accent ramp is the brighter tile, so it flips
  // earlier. Colour alone never carries a value here (the number is printed on every
  // tile), but it still has to be legible.
  accent: { lo: [18, 34, 28], hi: [46, 230, 168], flip: 0.48 },
  warm: { lo: [38, 27, 22], hi: [255, 138, 76], flip: 0.55 },
};

function rampCss(name, t) {
  const { lo, hi, flip } = RAMPS[name];
  const u = Number.isFinite(t) ? Math.min(1, Math.max(0, t)) : 0;
  const ch = (i) => Math.round(lo[i] + (hi[i] - lo[i]) * u);
  return { bg: `rgb(${ch(0)},${ch(1)},${ch(2)})`, ink: u > flip ? "#10131a" : "#e6e9ef" };
}

/** Tiles are built once per topology and repainted in place — a 100S10P pack is 1000
 *  nodes, and rebuilding those every animation frame is a stutter with no upside. */
const grid = { tiles: [], series: 0, parallel: 0, pinned: null, hovered: null, dirty: true };

function buildGrid(series, parallel) {
  const host = $("pack-grid");
  host.replaceChildren();
  host.style.gridTemplateColumns = `repeat(${parallel}, 84px)`;
  grid.tiles = [];
  grid.series = series;
  grid.parallel = parallel;
  grid.pinned = null;
  grid.hovered = null;

  // Series-major, parallel-minor — the order `cells[]` already arrives in, so tile n
  // is cells[n] and there is no index arithmetic to get wrong.
  for (let s = 0; s < series; s += 1) {
    for (let p = 0; p < parallel; p += 1) {
      const i = s * parallel + p;
      const el = document.createElement("div");
      el.className = "celltile";
      el.innerHTML = `<div class="idx"></div><div class="val"></div>`;
      el.querySelector(".idx").textContent = `${s},${p}`;
      el.onmouseenter = () => {
        grid.hovered = i;
        renderCellDetail();
      };
      el.onmouseleave = () => {
        if (grid.hovered === i) grid.hovered = null;
        renderCellDetail();
      };
      el.onclick = () => {
        grid.pinned = grid.pinned === i ? null : i;
        grid.dirty = true;
        paintGrid();
      };
      host.appendChild(el);
      grid.tiles.push({ el, val: el.querySelector(".val") });
    }
  }
}

function paintGrid() {
  // `draw()` runs every animation frame; the cells are sampled four times a second.
  // Without this guard a 100S10P pack would take a thousand style writes sixty times
  // a second to redraw values that had not moved.
  if (!grid.dirty) return;
  grid.dirty = false;

  const data = state.cells;
  if (!data) {
    if (grid.tiles.length > 0) buildGrid(0, 0);
    $("pack-lo").textContent = "—";
    $("pack-hi").textContent = "—";
    $("pack-ramp").style.background = "transparent";
    renderCellDetail();
    return;
  }
  if (data.series !== grid.series || data.parallel !== grid.parallel) {
    buildGrid(data.series, data.parallel);
  }

  const m = METRICS[$("pack-metric").value];
  const values = data.cells.map(m.get);
  let lo = Infinity;
  let hi = -Infinity;
  for (const v of values) {
    if (!Number.isFinite(v)) continue;
    if (v < lo) lo = v;
    if (v > hi) hi = v;
  }
  if (!Number.isFinite(lo)) [lo, hi] = [0, 0];
  // The pack's own range, not a fixed domain: the spread between cells is the thing
  // worth seeing, and a [0, 100] SOC axis renders a 3 mV imbalance as identical tiles.
  const span = hi - lo;

  for (let i = 0; i < grid.tiles.length; i += 1) {
    const tile = grid.tiles[i];
    const cell = data.cells[i];
    const v = values[i];
    // A pack with no spread gets the middle of the ramp rather than the bottom: every
    // tile dark would read as "no data" when it means "every cell agrees".
    const { bg, ink } = rampCss(m.ramp, span > 0 ? (v - lo) / span : 0.5);
    tile.el.style.background = bg;
    tile.el.style.color = ink;
    tile.val.textContent = Number.isFinite(v) ? v.toFixed(m.dp) : "—";
    tile.el.classList.toggle("short", cell.internal_short_conductance_s > 0);
    tile.el.classList.toggle("vented", cell.vented);
    tile.el.classList.toggle("pinned", grid.pinned === i);
  }

  const fmt = (v) => `${v.toFixed(m.dp)} ${m.unit}`;
  $("pack-lo").textContent = fmt(lo);
  $("pack-hi").textContent = fmt(hi);
  const a = rampCss(m.ramp, 0).bg;
  const b = rampCss(m.ramp, 1).bg;
  $("pack-ramp").style.background = `linear-gradient(to right, ${a}, ${b})`;
  markProbes(state.sensors?.temp_probe_at ?? []);
  renderCellDetail();
}

/** Every field of one cell's `CellView`, for the hovered tile or the pinned one. */
function renderCellDetail() {
  const host = $("pack-detail");
  if (state.cellsError) {
    host.textContent = `could not read the cells: ${state.cellsError}`;
    return;
  }
  const data = state.cells;
  const i = grid.hovered ?? grid.pinned;
  if (!data || i === null || i === undefined || !data.cells[i]) {
    host.textContent = data
      ? "hover a cell for its full ground-truth state; click to pin it."
      : "no cells read yet.";
    return;
  }
  const c = data.cells[i];
  const s = Math.floor(i / data.parallel);
  const p = i % data.parallel;
  const parts = [
    `cell (${s},${p})`,
    `soc ${(c.soc * 100).toFixed(3)} %`,
    `T ${toC(c.temp_k).toFixed(3)} °C`,
    `overpotential ${(c.overpotential_v * 1000).toFixed(2)} mV`,
    `capacity ×${c.capacity_factor.toFixed(4)}`,
    `R0 ×${c.r0_factor.toFixed(4)}`,
    `soh cap ${(c.soh_capacity * 100).toFixed(3)} %`,
    `soh res ×${c.soh_resistance.toFixed(4)}`,
  ];
  if (c.internal_short_conductance_s > 0) {
    parts.push(`internal short ${(1 / c.internal_short_conductance_s).toFixed(2)} Ω`);
  }
  if (c.runaway_energy_remaining_j > 0) {
    parts.push(`exotherm left ${(c.runaway_energy_remaining_j / 1000).toFixed(2)} kJ`);
  }
  if (c.vented) parts.push("VENTED");
  host.textContent = `${parts.join("  ·  ")}${grid.pinned === i ? "  (pinned)" : ""}`;
}

// ---------------------------------------------------------------------------
// The BMS view — what the pack knows, beside what the BMS believes
// ---------------------------------------------------------------------------

/*
 * `CLAUDE.md` principle 8: the engine knows every cell's true state, the BMS only ever
 * sees a `SensorFrame`, and the gap between them "is a feature to expose, not a bug to
 * hide". Until this panel the page could expose exactly one scalar of that gap —
 * `soc_bms` against `soc_true`.
 *
 * The order of the channels below is the physics and not the obvious layout. Leading
 * with per-group voltage was the first instinct and it is degenerate: `sim-core` moves
 * the true group voltage *straight into* the sensor frame, so voltages and probe
 * temperatures are **exact** reads and would draw as pixel-identical to the truth
 * forever on a healthy pack, which reads as a broken panel. What actually diverges,
 * in causal order:
 *
 *   1. current      — offset + noise on every step. The root cause.
 *   2. state of charge — coulomb-counts (1), so it inherits the error and integrates it.
 *   3. temperature  — the probes read exactly, but only where they are.
 *   4. group voltage — exact *until a fault lies about it*, which is the point of it.
 *
 * Truth wears the accent hue and belief wears the warm one throughout, the same pair
 * the SOC plot already gives those two entities: colour follows the entity, so a
 * reader who has learnt "amber is what the BMS thinks" on one panel keeps it here.
 */

/** °C where the channel is a temperature, plain otherwise. */
const fmtSigned = (v, dp, unit) => `${v >= 0 ? "+" : "−"}${Math.abs(v).toFixed(dp)} ${unit}`;

/**
 * How far outside the true envelope a sensed group voltage must sit before the panel
 * calls it a lie \[V\].
 *
 * It is **not** floating-point slack, and it is no longer the sampler slack an earlier
 * draft claimed: `comparable` below already refuses to judge unless the frame's
 * `sampled_at_s` matches the pack's clock, so by the time this constant is consulted
 * the dot and the band are from the same step and an honest read is *bit*-identical to
 * one of the values the envelope was taken from. Nothing this guards can be named.
 *
 * It stays because the cost of being wrong is asymmetric — a spurious "outside truth"
 * accuses working hardware — and because 1 mV cannot mask a real fault: an injected
 * offset is tens of millivolts, two orders up.
 */
const LIE_TOLERANCE_V = 1e-3;

function renderBms() {
  const host = $("bms-body");
  const when = $("bms-when");
  const sensors = state.sensors;

  if (!sensors) {
    when.textContent = "";
    host.replaceChildren();
    const note = document.createElement("div");
    note.className = "note";
    note.textContent = state.backend
      ? "This pack has no BMS, so it has no sensors and nothing to believe. The physics " +
        "below is unchanged — that contrast is the point of the toggle."
      : "Load a scenario to see what its BMS can and cannot measure.";
    host.appendChild(note);
    return;
  }

  const t = state.latest;
  const simTime = state.facts?.sim_time_s ?? 0;
  // Sampling is gated on `dt > 0`, so a paused pack's frame is legitimately old and a
  // zero-length read does not refresh it. Saying so is what keeps a stale frame from
  // reading as a broken panel — which is exactly how it looks otherwise.
  const lag = simTime - sensors.sampled_at_s;
  // A pack that has never advanced has never *sampled*: the frame it carries is the
  // construction-time open-circuit read — every group at OCV(initial_soc), the current
  // sensor an exact zero it has not earned. `readNow()` meanwhile reports telemetry
  // under the page's standing demand, so the two describe the same instant and not the
  // same pack. Comparing them tags every group as lying on a pack where nothing is
  // wrong, which is a false accusation and not a stale-data warning — the clocks agree,
  // so no timestamp check can catch it.
  const booted = simTime > 0;
  const comparable = Boolean(t) && booted && lag <= 1e-9;
  when.textContent = !booted
    ? "boot read — the sensors have not sampled yet"
    : lag > 1e-9
      ? `sampled at ${sensors.sampled_at_s.toFixed(1)} s — ${lag.toFixed(1)} s behind the clock, because sensors sample only on a step with dt > 0`
      : `sampled at ${sensors.sampled_at_s.toFixed(1)} s`;
  when.classList.toggle("stale", !booted || lag > 1e-9);

  host.replaceChildren();

  // ---- the three scalar channels, in causal order
  const gaps = document.createElement("div");
  gaps.className = "gaps";
  const probeMax = sensors.temp_probe_k.length
    ? Math.max(...sensors.temp_probe_k)
    : null;
  const channels = [
    {
      k: "current",
      truth: t ? `${t.i_actual.toFixed(3)} A` : "—",
      belief: `${sensors.i_pack_a.toFixed(3)} A`,
      gap: t ? fmtSigned(sensors.i_pack_a - t.i_actual, 3, "A") : "—",
      why: "a configured offset plus a noise draw, on every single step",
    },
    {
      k: "state of charge",
      truth: t ? `${(t.soc_true * 100).toFixed(2)} %` : "—",
      belief: `${(sensors.soc_est * 100).toFixed(2)} %`,
      gap: t ? fmtSigned((sensors.soc_est - t.soc_true) * 100, 2, "pt") : "—",
      why: "coulomb-counts the current above, so it inherits that error and integrates it",
    },
    {
      k: "temperature",
      truth: t ? `${toC(t.t_max).toFixed(2)} °C` : "—",
      belief: probeMax === null ? "no probes" : `${toC(probeMax).toFixed(2)} °C`,
      gap: t && probeMax !== null ? fmtSigned(probeMax - t.t_max, 2, "K") : "—",
      why: `${sensors.temp_probe_k.length} probe(s) read exactly, but only where they sit`,
    },
  ];
  for (const c of channels) {
    const el = document.createElement("div");
    el.className = "gap";
    el.innerHTML =
      `<div class="k"></div>` +
      `<div class="pair"><span class="sw truth"></span><span class="lab">truth</span><span class="num tv"></span></div>` +
      `<div class="pair"><span class="sw belief"></span><span class="lab">BMS</span><span class="num bv"></span></div>` +
      `<div class="delta"></div><div class="why"></div>`;
    el.querySelector(".k").textContent = c.k;
    el.querySelector(".tv").textContent = c.truth;
    el.querySelector(".bv").textContent = c.belief;
    el.querySelector(".delta").textContent = c.gap;
    el.querySelector(".why").textContent = c.why;
    gaps.appendChild(el);
  }
  host.appendChild(gaps);

  // ---- the fourth channel: one sensed voltage per group against the true envelope
  //
  // A dot plot rather than bars. The interesting window is tens of millivolts on a
  // 3.3 V cell, so the axis cannot start at zero — and a bar on a truncated axis is
  // the textbook way to draw a difference that is not there. A dot carries a position
  // and claims nothing about the distance to an origin it never touches.
  const rows = document.createElement("div");
  rows.className = "groups";

  const lo = t ? Math.min(t.v_cell_min, ...sensors.v_group) : Math.min(...sensors.v_group);
  const hi = t ? Math.max(t.v_cell_max, ...sensors.v_group) : Math.max(...sensors.v_group);
  // A pack whose groups all agree has zero span; give it a window rather than dividing
  // by nothing and putting every dot at the left edge.
  const pad = Math.max((hi - lo) * 0.15, 5e-4);
  const axisLo = lo - pad;
  const axisHi = hi + pad;
  const pct = (v) => `${((v - axisLo) / (axisHi - axisLo)) * 100}%`;

  for (let g = 0; g < sensors.v_group.length; g += 1) {
    const v = sensors.v_group[g];
    const lying =
      comparable &&
      (v < t.v_cell_min - LIE_TOLERANCE_V || v > t.v_cell_max + LIE_TOLERANCE_V);
    const row = document.createElement("div");
    row.className = `grow${lying ? " lying" : ""}`;
    row.innerHTML =
      `<span class="gi"></span><div class="track">` +
      `<div class="band"></div><div class="dot"></div></div>` +
      `<span class="gv"></span><span class="tag"></span>`;
    row.querySelector(".gi").textContent = `g${g}`;
    row.querySelector(".gv").textContent = `${v.toFixed(4)} V`;
    if (t) {
      const band = row.querySelector(".band");
      band.style.left = pct(t.v_cell_min);
      band.style.width = `${((t.v_cell_max - t.v_cell_min) / (axisHi - axisLo)) * 100}%`;
      band.title = `true group voltages span ${t.v_cell_min.toFixed(4)}–${t.v_cell_max.toFixed(4)} V`;
    }
    const dot = row.querySelector(".dot");
    dot.style.left = pct(v);
    dot.title = `group ${g} sensor reads ${v.toFixed(4)} V`;
    // Never colour alone: a faulted sensor gets a word as well as a ring.
    row.querySelector(".tag").textContent = lying ? "outside truth" : "";
    rows.appendChild(row);
  }
  host.appendChild(rows);

  const legend = document.createElement("div");
  legend.className = "glegend";
  legend.innerHTML =
    `<span class="sw belief"></span><span>what the group sensor reads</span>` +
    `<span class="sw bandsw"></span><span>the true spread across every group</span>` +
    `<span class="ax"></span>`;
  legend.querySelector(".ax").textContent = `${axisLo.toFixed(3)} – ${axisHi.toFixed(3)} V`;
  host.appendChild(legend);

  const note = document.createElement("div");
  note.className = "note";
  note.textContent =
    "Group voltages are exact reads until something lies about one: parallel cells " +
    "share a node, so this is the finest voltage resolution any real pack has, and a " +
    "weak cell hiding inside a healthy group is invisible here even when nothing is " +
    "faulted. Inject a SensorOffset on a GroupVoltage to watch a dot leave the band.";
  if (!comparable) {
    note.textContent += booted
      ? " The dots and the band are from different instants right now, so no dot is " +
        "called a liar until the sensors sample again."
      : " Nothing is being compared yet: the band is this pack under its present " +
        "demand, while the dots are the open-circuit read taken when it was built. " +
        "Step once and they become the same instant.";
  }
  host.appendChild(note);

}

/**
 * Ring the instrumented cells on the ground-truth grid.
 *
 * Called from `paintGrid` rather than from `renderBms`, because `paintGrid` is what
 * rebuilds the tiles when the topology changes — ringing them from anywhere else means
 * the rings survive until the next rebuild and then silently vanish.
 *
 * This is the temperature channel's spatial half: the probes read *exactly*, so the
 * only way to see their error is to see where they are not.
 */
function markProbes(positions) {
  if (!grid.tiles.length) return;
  const wanted = new Set(positions.map(([s, p]) => s * grid.parallel + p));
  for (let i = 0; i < grid.tiles.length; i += 1) {
    grid.tiles[i].el.classList.toggle("probed", wanted.has(i));
  }
}

/**
 * Sample the cells, at most every `CELLS_PERIOD_MS`.
 *
 * Throttled because one backend pays a REST round trip for this and the other
 * serialises every cell to JSON — neither is worth doing sixty times a second to
 * repaint tiles a person reads a few times a second.
 *
 * A failure here writes to the detail line, not the banner: this runs on a timer, and
 * a background poll that hijacks the page's error banner would bury whatever the run
 * itself was trying to report.
 */
const CELLS_PERIOD_MS = 250;

async function refreshCells(force = false) {
  if (!state.backend || state.cellsBusy) return;
  const now = performance.now();
  if (!force && now - state.cellsAtMs < CELLS_PERIOD_MS) return;
  state.cellsBusy = true;
  state.cellsAtMs = now;
  // Which pack this read is *about*. Loading a scenario replaces the backend, and a
  // read already in flight against the old one still resolves — writing a pack the
  // page has moved on from into `state`, where the grid rebuilds to its topology and
  // the BMS panel draws its probes. It self-heals on the next tick, which is precisely
  // what makes it worth catching: a wrong pack that corrects itself in 250 ms looks
  // like a rendering glitch rather than the stale read it is.
  const from = state.backend;
  try {
    // Ground truth and the BMS's belief are read as a pair, because the panel compares
    // them directly and a gap opened by the sampler would be read as the BMS's.
    //
    // A pair, not an instant: the socket backend spends two independent round trips
    // here, and a session another client is stepping can advance between them. That is
    // what `comparable` in `renderBms` exists for — it declines to call any group a
    // liar unless the frame's own `sampled_at_s` still matches the pack's clock.
    const [cells, sensors] = await Promise.all([
      Promise.resolve(from.cells()),
      Promise.resolve(from.sensors()),
    ]);
    if (state.backend !== from) return;
    state.cells = cells;
    state.sensors = sensors;
    state.cellsError = null;
  } catch (e) {
    if (state.backend !== from) return;
    state.cells = null;
    state.sensors = null;
    state.cellsError = String(e.message ?? e);
  } finally {
    state.cellsBusy = false;
    grid.dirty = true;
  }
  renderBms();
}

// ---------------------------------------------------------------------------
// Fault injection
// ---------------------------------------------------------------------------

/**
 * The five `sim_core::Fault` variants as forms.
 *
 * `int: true` marks a topology or sensor index — the engine's fields are `u16`, and a
 * number input will happily hand back `1.5`, which serde rejects with a message about
 * the wire format rather than about the pack.
 */
const FAULT_FORMS = {
  SoftInternalShort: [
    { k: "s", label: "series index", value: 0, step: 1, min: 0, int: true },
    { k: "p", label: "parallel index", value: 0, step: 1, min: 0, int: true },
    { k: "ohms", label: "leak [Ω] — lower drains faster", value: 5, step: 0.5, wide: true },
  ],
  ExternalShort: [
    { k: "ohms", label: "short [Ω] across the terminals", value: 0.5, step: 0.1, wide: true },
  ],
  WeakCell: [
    { k: "s", label: "series index", value: 0, step: 1, min: 0, int: true },
    { k: "p", label: "parallel index", value: 0, step: 1, min: 0, int: true },
    // "replaces" is not a hint, it is the semantics: `Fault::WeakCell`'s doc says the
    // new factors replace the cell's scatter draw rather than multiplying onto it.
    { k: "capacity_factor", label: "capacity × (replaces)", value: 0.8, step: 0.05 },
    { k: "r0_factor", label: "R0 × (replaces)", value: 1.5, step: 0.1 },
  ],
  SensorStuck: [
    { k: "sensor", label: "sensor", kind: "sensor", wide: true },
    { k: "value", label: "frozen reading [V / K / A]", value: 3.3, step: 0.05, wide: true },
  ],
  SensorOffset: [
    { k: "sensor", label: "sensor", kind: "sensor", wide: true },
    { k: "offset", label: "added to every reading [V / K / A]", value: 0.12, step: 0.01, wide: true },
  ],
};

const SENSOR_KINDS = [
  ["GroupVoltage", "group voltage — index is the series position"],
  ["TempProbe", "temp probe — index into the BMS's probe list"],
  ["PackCurrent", "pack current — the one sensor, no index"],
];

function buildFaultForm() {
  const host = $("fault-fields");
  host.replaceChildren();
  for (const f of FAULT_FORMS[$("fault-kind").value]) {
    const wrap = document.createElement("div");
    if (f.wide || f.kind === "sensor") wrap.className = "wide";
    const label = document.createElement("label");
    label.textContent = f.label;
    wrap.appendChild(label);

    if (f.kind === "sensor") {
      const sel = document.createElement("select");
      sel.id = "fault-sensor-kind";
      for (const [value, text] of SENSOR_KINDS) {
        const opt = document.createElement("option");
        opt.value = value;
        opt.textContent = text;
        sel.appendChild(opt);
      }
      const idx = document.createElement("input");
      idx.type = "number";
      idx.id = "fault-sensor-idx";
      idx.value = "0";
      idx.min = "0";
      idx.step = "1";
      idx.style.marginTop = "4px";
      // `PackCurrent` is a unit variant: there is no index to ask for, so asking would
      // invite a number the encoding has nowhere to put.
      sel.onchange = () => {
        idx.style.display = sel.value === "PackCurrent" ? "none" : "";
      };
      wrap.appendChild(sel);
      wrap.appendChild(idx);
    } else {
      const input = document.createElement("input");
      input.type = "number";
      input.id = `fault-f-${f.k}`;
      input.value = String(f.value);
      input.step = String(f.step ?? 1);
      if (f.min !== undefined) input.min = String(f.min);
      wrap.appendChild(input);
    }
    host.appendChild(wrap);
  }
}

/** The externally-tagged `Fault` the form currently describes. */
function currentFault() {
  const kind = $("fault-kind").value;
  const body = {};
  for (const f of FAULT_FORMS[kind]) {
    if (f.kind === "sensor") {
      const sensorKind = $("fault-sensor-kind").value;
      const i = Math.max(0, Math.round(Number($("fault-sensor-idx").value) || 0));
      // A unit variant crosses as a bare string; the other two are newtypes.
      body.sensor = sensorKind === "PackCurrent" ? "PackCurrent" : { [sensorKind]: i };
    } else {
      const raw = Number($(`fault-f-${f.k}`).value);
      body[f.k] = f.int ? Math.max(0, Math.round(raw || 0)) : raw;
    }
  }
  return { [kind]: body };
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
  cells: null,
  sensors: null,
  cellsBusy: false,
  cellsAtMs: 0,
  cellsError: null,
  // Why the picker below is the page's own hand-written list rather than the server's,
  // or null when the listing loaded. Sticky for the session: nothing retries it.
  scenarioListError: null,
};

/**
 * The guided path's own state. Declared up here, beside `state`, rather than down with
 * the rest of the path code: `frame` reads `path.until` and the first animation frame
 * can fire while boot is still awaiting its scenario load, which would hit the temporal
 * dead zone of a `const` declared further down.
 *
 * `until` is a simulation time in seconds, or null when no step is running to a mark.
 */
const path = { on: false, i: 0, until: null, busy: false };

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

async function advance(nSteps) {
  const dt = Math.max(1e-6, Number($("dt").value) || 0.5);
  const mode = $("demand-mode").value;

  // Whichever backend is loaded reports its own caps — the wasm module from its
  // constants, the server from its hello frame. Nothing here restates them.
  const { maxSteps, maxFrames } = state.backend.limits();
  const steps = Math.min(nSteps, maxSteps);
  // At high speed multipliers a frame can be thousands of steps. Decimating keeps the
  // reply small; it drops *reports*, never steps, so the trajectory is untouched. 300
  // frames is already more than a plot this size can resolve.
  const reportEvery = Math.max(1, Math.ceil(steps / 300), Math.ceil(steps / maxFrames));
  if (Math.ceil(steps / reportEvery) > maxFrames) {
    throw new Error("frame cap exceeded — this is a bug in the page's decimation");
  }

  const take = async (n, demand) => {
    const frames = await state.backend.step(dt, n, demand, reportEvery);
    for (const frame of frames) record(frame);
    if (frames.length > 0) {
      state.latest = frames[frames.length - 1].telemetry;
      state.facts = state.backend.facts();
    }
  };

  if (mode === "Pulse") {
    // Same chopping as CC-CV below, and for the same reason — but the boundaries are
    // the train's own edges rather than a fixed sub-clock, so there is no third
    // quantisation to explain: the demand changes exactly where `pulsePhase` says and
    // nowhere else. No completion test, because a train does not finish; the guided
    // path's mark or the reader stops it.
    const cfg = pulseConfig();
    let left = steps;
    while (left > 0) {
      const phase = pulsePhase(state.facts?.sim_time_s ?? 0, cfg, dt);
      const n = phase === null ? left : Math.min(left, phase.toEdge);
      await take(n, pulseDemand(phase, cfg));
      left -= n;
    }
    return;
  }

  if (mode !== "CcCv") {
    await take(steps, demandJson(mode, Number($("demand-value").value) || 0));
    return;
  }

  // The CC-CV path. One backend call per *decision window* instead of one per frame:
  // the frame's steps are chopped at multiples of the sub-clock, so which step the leg
  // changes on is a property of the simulation and not of how the browser scheduled us.
  //
  // The grid is anchored to `sim_time_s`, which the backend reports and a snapshot
  // carries — not to a counter this page keeps. That is what makes a restore land back
  // on the same decision points without the page remembering anything about the run it
  // is resuming.
  const cfg = ccCvConfig();
  const series = state.facts?.series ?? 1;
  const k = Math.max(1, Math.round(CCCV_PERIOD_S / dt));
  let left = steps;
  while (left > 0) {
    const index = Math.round((state.facts?.sim_time_s ?? 0) / dt);
    const toBoundary = k - ((index % k) + k) % k;
    const n = Math.min(left, toBoundary === 0 ? k : toBoundary);
    // The pack's own voltage decides, from the last frame the backend reported. It is
    // one step stale, which the 1 mV band swallows three orders over.
    await take(n, ccCvDemand(state.latest?.v_terminal ?? 0, cfg, series));
    left -= n;

    const done = ccCvDone(state.latest, cfg, series);
    if (done) {
      // Stopping here rather than letting the loop run on is the point of having a
      // completion test at all: a finished charge that keeps stepping either sits at
      // the clamp or, with a BMS in the way, quietly holds a pack that was never
      // charged at the SOC where protection stopped it.
      state.running = false;
      $("run").textContent = "Run";
      return;
    }
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
 *
 * In CC-CV mode there is no previous frame to decide a leg from — this is the call that
 * produces the first one — so the probe is taken on the CC leg, which is the leg any
 * pack below the target would be on anyway. A pack already above it reads one frame of
 * a current it will not be given, and the first real window corrects it.
 *
 * `Pulse` needs no such concession and is the contrast worth noticing: its leg is a
 * function of `sim_time_s`, which is known here, so the probe is taken under exactly
 * the demand the next real step will use — including `Rest`, if the train is between
 * pulses.
 */
async function readNow() {
  if (!state.backend) return;
  const mode = $("demand-mode").value;
  const dt = Math.max(1e-6, Number($("dt").value) || 0.5);
  let demand;
  if (mode === "CcCv") {
    demand = demandJson("Current", -ccCvConfig().i_charge);
  } else if (mode === "Pulse") {
    const cfg = pulseConfig();
    demand = pulseDemand(pulsePhase(state.facts?.sim_time_s ?? 0, cfg, dt), cfg);
  } else {
    demand = demandJson(mode, Number($("demand-value").value) || 0);
  }
  const frames = await state.backend.step(0, 1, demand, 1);
  for (const frame of frames) record(frame);
  if (frames.length > 0) {
    state.latest = frames[frames.length - 1].telemetry;
    state.facts = state.backend.facts();
  }
  // Forced past the throttle: this runs after a load, a restart and a restore, and each
  // of those can change the topology the grid is built for.
  await refreshCells(true);
  // Paint now rather than waiting for the next animation frame. Usually that is 16 ms
  // and nobody notices — but a backgrounded tab has no animation frames at all, and a
  // page that loads into a row of dashes and stays there looks broken rather than
  // paused.
  draw();
}

/**
 * Which leg the charge is on, in words, under the CC-CV fields.
 *
 * Derived entirely from the last telemetry frame and the fields, like the controller
 * itself — there is no leg stored anywhere for this to read, and that is deliberate.
 */
function ccCvNote() {
  if ($("demand-mode").value !== "CcCv") return;
  const el = $("cccv-note");
  const cfg = ccCvConfig();
  const series = state.facts?.series ?? 1;
  const target = cfg.v_cell * series;
  const band = CCCV_BAND_V_PER_CELL * series;
  const t = state.latest;
  if (!t) {
    el.textContent = `${cfg.i_charge.toFixed(3)} A up to ${target.toFixed(3)} V (${series} in series), done below ${cfg.i_taper.toFixed(3)} A.`;
    return;
  }
  const soc = `${(t.soc_true * 100).toFixed(1)} %`;
  const done = ccCvDone(t, cfg, series);
  if (done) {
    el.textContent = done.ok
      ? `${done.why}, at ${soc}. Raise the target or lower the cutoff to carry on.`
      : `${done.why}. The pack is at ${soc}.`;
    return;
  }
  if (t.v_terminal < target - band) {
    // `i_actual` is what the pack got, not what was asked for: when a BMS derates a
    // charge the two differ silently, and this is the only place the page says so. The
    // test is `i_actual < 0` and not `|i_actual| != i_charge`, because a frame taken
    // under some *other* demand — the moment the mode is switched, before the probe
    // lands — is not evidence of a derate. Written the loose way first, and it
    // immediately accused a 2 A discharge of being a limited charge.
    const charging = t.i_actual < 0;
    const shortfall = charging && cfg.i_charge - Math.abs(t.i_actual) > 1e-6;
    el.textContent =
      `constant current: ${Math.abs(t.i_actual).toFixed(3)} A ${charging ? "in" : "out"}, ` +
      `terminal ${t.v_terminal.toFixed(3)} V of ${target.toFixed(3)} V, ${soc}.` +
      (shortfall ? ` Asked for ${cfg.i_charge.toFixed(3)} A — something is limiting it.` : "");
    return;
  }
  el.textContent =
    t.i_actual > cfg.i_taper
      ? `holding ${target.toFixed(3)} V, but this pack is *above* that target, so the hold ` +
        `is drawing ${t.i_actual.toFixed(3)} A out of it rather than charging. Raise the target.`
      : `holding ${target.toFixed(3)} V: ${Math.abs(t.i_actual).toFixed(3)} A and falling, ` +
        `${soc}. Done below ${cfg.i_taper.toFixed(3)} A.`;
}

/**
 * Where the train is, in words, under the pulse fields.
 *
 * Like `ccCvNote` this derives everything it says, but from a stricter source: the
 * *leg* and the *pulse number* come from `sim_time_s` alone, so they are right even
 * before a single frame has been recorded. Only the measured current comes from
 * telemetry, and it is labelled as what the pack got rather than what was asked for —
 * a BMS between the two is exactly the sort of thing this page exists to show.
 */
function pulseNote() {
  if ($("demand-mode").value !== "Pulse") return;
  const el = $("pulse-note");
  const cfg = pulseConfig();
  const dt = Math.max(1e-6, Number($("dt").value) || 0.5);
  const phase = pulsePhase(state.facts?.sim_time_s ?? 0, cfg, dt);
  if (phase === null) {
    el.textContent = "both legs are zero, so there is no train — this rests.";
    return;
  }
  const secs = (phase.toEdge * dt).toFixed(0);
  // The leg is the leg of the step about to be taken; `i_actual` is from the step just
  // taken. Sitting exactly on an edge those are different legs, and the line would read
  // "on: 0.000 A measured" — the milder cousin of the CC-CV status line that accused a
  // discharge of being a limited charge. So the measured current is shown only when the
  // step that produced it was on this same leg.
  // At t = 0 there is no previous step, so whatever frame exists is the `dt = 0` probe
  // `readNow` took under this very leg — freshest it could be. Elsewhere the check errs
  // towards saying less: a probe taken while sitting exactly on an edge is suppressed
  // even though it was valid, which is the harmless direction.
  const simT = state.facts?.sim_time_s ?? 0;
  const previous = simT <= 0 ? phase : pulsePhase(simT - dt, cfg, dt);
  const fresh = state.latest && previous !== null && previous.on === phase.on;
  const measured = fresh ? `${state.latest.i_actual.toFixed(3)} A measured, ` : "";
  el.textContent = phase.on
    ? `pulse ${phase.pulse}, on: ${measured}${secs} s to the rest.`
    : `pulse ${phase.pulse}, resting: ${measured}${secs} s to the next pulse.`;
}

function draw() {
  ccCvNote();
  pulseNote();
  drawPanel($("plot-v"), "pack terminal", "V", history.t, [
    { label: "terminal", color: "#2ee6a8", values: history.v_terminal },
  ]);
  // Its own axis, so the spread between the best and worst cell stays legible however
  // many cells are in series.
  drawPanel($("plot-cv"), "cell voltage", "V", history.t, [
    { label: "min", color: "#7ddc7d", values: history.v_cell_min },
    { label: "max", color: "#ffb454", values: history.v_cell_max },
  ]);
  drawPanel($("plot-i"), "current (discharge +)", "A", history.t, [
    { label: "pack", color: "#2ee6a8", values: history.i_actual },
    { label: "int. short", color: "#ff6b6b", values: history.i_internal_short_a },
  ]);
  drawPanel(
    $("plot-soc"),
    "state of charge",
    "%",
    history.t,
    [
      { label: "truth", color: "#2ee6a8", values: history.soc_true },
      { label: "bms est.", color: "#ffb454", values: history.soc_bms },
    ],
    [0, 100],
  );
  drawPanel($("plot-t"), "cell temperature", "°C", history.t, [
    { label: "min", color: "#2ee6a8", values: history.t_min },
    { label: "max", color: "#ff6b6b", values: history.t_max },
  ]);
  // One axis for both, because both are percentages *of new*. `CLAUDE.md` refuses to
  // model capacity fade without the matching resistance growth, and this is that rule
  // drawn: the two lines leave 100 % together and never come back.
  drawPanel($("plot-soh"), "state of health (% of new)", "%", history.t, [
    { label: "capacity", color: "#2ee6a8", values: history.soh_capacity },
    { label: "resistance", color: "#ffb454", values: history.soh_resistance },
  ]);

  renderReadouts(state.latest, state.facts ?? { sim_time_s: 0 });
  renderFlags(state.latest);
  paintGrid();
}

async function frame(nowMs) {
  requestAnimationFrame(frame);
  if (!state.backend) return;

  // Deliberately not awaited: the grid is a view, and making the physics frame wait on
  // a REST round trip would let the socket backend's latency set the step rate — the
  // one thing `CLAUDE.md` says a client must never do. It self-throttles and
  // self-guards against overlap.
  refreshCells();

  if (state.running && !state.busy) {
    const dt = Math.max(1e-6, Number($("dt").value) || 0.5);
    const speed = 10 ** Number($("speed").value);
    let n = stepsForFrame(nowMs, dt, speed);
    // A guided step runs to a stated simulation time. At 800x one frame is thousands of
    // steps, so without this clamp a step would sail past its own mark by most of a
    // lesson. Clamping the *count* keeps `dt` fixed — the step is never shortened to fit,
    // which would be the frame rate defining the timestep by another route.
    if (path.until !== null) {
      const remaining = path.until - (state.facts?.sim_time_s ?? 0);
      n = Math.min(n, Math.max(0, Math.ceil(remaining / dt)));
    }
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
  // Checked outside the `running` branch so a step whose mark is already behind it — a
  // Back that reloaded, or a zero-length step — settles instead of waiting for a step
  // that will never be taken.
  if (path.until !== null && (state.facts?.sim_time_s ?? 0) >= path.until - 1e-9) {
    pathArrived();
  }
  draw();
}
requestAnimationFrame(frame);

// ---------------------------------------------------------------------------
// Controls
// ---------------------------------------------------------------------------

/**
 * Fill the scenario picker from `GET /scenarios`.
 *
 * The `<option>` list in `index.html` is a **fallback, not the source of truth**. It
 * exists for one case: a page newer than the server it was fetched from, where this
 * route 404s. Leaving the picker empty there would be a silent failure — the reader sees
 * a control with nothing in it and no reason given — so the hand-written list stays and
 * the banner says what happened. That list may be stale; this one cannot be, and adding
 * a scenario is a TOML file with no HTML edit behind it.
 *
 * A file that does not parse is listed **disabled, carrying its error**, which is the
 * same choice the route makes and for the same reason: an author whose scenario has
 * vanished from the picker has nowhere to look.
 */
async function loadScenarioList() {
  const res = await fetch("/scenarios");
  if (!res.ok) throw new Error(`GET /scenarios -> ${res.status}`);
  const listed = await res.json();
  if (!Array.isArray(listed) || listed.length === 0) {
    throw new Error("GET /scenarios returned no scenarios");
  }

  const select = $("scenario");
  const wanted = select.value;
  select.replaceChildren();
  for (const entry of listed) {
    const option = document.createElement("option");
    option.value = entry.file;
    if (entry.error) {
      option.disabled = true;
      option.textContent = `${entry.file} — will not load: ${entry.error}`;
    } else {
      option.textContent = `${entry.file.replace(/\.toml$/, "")} — ${scenarioSummary(entry)}`;
    }
    select.appendChild(option);
  }
  // Keep the reader (or the guided path) pointed where they were, if it survived.
  if (listed.some((e) => e.file === wanted && !e.error)) select.value = wanted;
}

/** The half-line after the file name: topology, then what is switched on. */
function scenarioSummary(e) {
  const on = [];
  if (e.bms) on.push("BMS");
  if (e.aging) on.push("aging");
  if (e.thermal !== "Isothermal") on.push("thermal");
  if (e.faults) on.push(e.faults === 1 ? "1 fault" : `${e.faults} faults`);
  // `cell_model` is the scenario's own value — a string for the equivalent circuit, an
  // object for anything else — so anything that is not "Ecm" is worth naming.
  if (typeof e.cell_model !== "string") on.push(Object.keys(e.cell_model)[0]);
  return `${e.series}S${e.parallel}P, ${on.length ? on.join(", ") : "nothing on"}`;
}

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
  // The next scenario may be a different topology, so the grid is rebuilt rather than
  // repainted — and a pin is an index into a pack that no longer exists.
  state.cells = null;
  state.sensors = null;
  state.cellsError = null;
  grid.pinned = null;
  grid.hovered = null;
  grid.dirty = true;
  renderBms();
  clearBanner();

  // Ticket for this load. Two awaits below can interleave with a later call — a fetch
  // and a backend build, and over the socket the second is a REST create plus a
  // WebSocket handshake, so the spread between two loads is milliseconds wide.
  const seq = ++loadSeq;
  try {
    const res = await fetch(`/scenarios/${name}`);
    if (!res.ok) throw new Error(`GET /scenarios/${name} -> ${res.status}`);
    const text = await res.text();

    const Backend = $("use-socket").checked ? SocketBackend : WasmBackend;
    const backend = await Backend.create(text);
    // Nothing above this line touched `state`. A load that has been overtaken stops
    // here and takes its backend with it — over the socket that is a live session, so
    // dropping the reference without closing it would leak one per keystroke.
    if (seq !== loadSeq) {
      backend.close();
      return;
    }

    state.scenarioText = text;
    state.backend = backend;
    state.facts = backend.facts();
    applyEnv();
    afterFactsChange(Backend.label);
    await readNow();
  } catch (e) {
    // A stale load's failure is not this page's news — the pack it was about is not the
    // pack on screen, and the banner belongs to whatever loaded last.
    if (seq === loadSeq) showBanner(String(e.message ?? e));
  }
}

/**
 * Which `loadScenario` call is the current one.
 *
 * Found by arrowing through the picker over the socket: four `change` events in a row
 * start four overlapping loads, and **the one that finishes last wins, which is not the
 * one that started last**. The page settled showing a 1S1P pack at 100 % beside a picker
 * reading `calendar_fade_hot` — a 4S2P pack at 95 %. Every control on the page was then
 * driving a scenario the reader had not chosen.
 *
 * The race predates the picker being served, but two options made it nearly unreachable
 * and five make it a normal keyboard interaction. `path.busy` guards the guided path and
 * guards nothing here, because a reader turning the knob directly is not in a step.
 */
let loadSeq = 0;

function afterFactsChange(label) {
  const f = state.facts;
  $("scenario-note").textContent =
    `${f.series}S${f.parallel}P · ${label}` +
    (f.sensor_faults_dropped
      ? ` · ${f.sensor_faults_dropped} sensor fault(s) dropped: no BMS to sense them`
      : "") +
    (state.scenarioListError
      ? ` · scenario list unavailable (${state.scenarioListError}); the picker above is ` +
        `this page's built-in list and may be out of date with the server`
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

  // Whether a sensor fault is even injectable is a property of the pack that just
  // loaded, so the hint is re-evaluated here rather than only when the variant changes.
  sensorHint();
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
  // A guided step's note reports whether it is running, so the note has to be re-rendered
  // by whatever changes that — including this button, which is the one the note tells the
  // reader they are free to press.
  if (path.on) renderStep();
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

/**
 * Show the fields the selected mode actually uses.
 *
 * The single `value` box cannot serve CC-CV: the mode needs three numbers, and one of
 * them is entered with the *opposite* sign convention to everything else on this page
 * (positive charges, because it is a charge current). Nor `Pulse`, which needs three of
 * its own — and keeps the page's usual sign convention, so the two groups do not share
 * a current field however similar they look. Three field groups, one visible.
 */
function applyDemandMode() {
  const mode = $("demand-mode").value;
  const cccv = mode === "CcCv";
  const pulse = mode === "Pulse";
  $("demand-simple").style.display = cccv || pulse ? "none" : "";
  $("demand-cccv").style.display = cccv ? "" : "none";
  $("demand-pulse").style.display = pulse ? "" : "none";
  if (cccv) ccCvNote();
  if (pulse) pulseNote();
}

$("demand-mode").onchange = () => {
  applyDemandMode();
  // A dt = 0 probe under the demand just selected. Without it the readouts — and the
  // CC-CV status line, which reasons about them — describe the pack under the *previous*
  // demand until someone presses Run. The guided path already does this after setting a
  // step's controls, for the same reason; a reader turning the knob deserves it too.
  readNow();
};
applyDemandMode();

$("pack-metric").onchange = () => {
  grid.dirty = true;
  paintGrid();
};

// ---------------------------------------------------------------------------
// Fault controls
// ---------------------------------------------------------------------------

$("fault-kind").onchange = () => {
  buildFaultForm();
  sensorHint();
};
buildFaultForm();

function faultNote(text) {
  $("fault-note").textContent = text;
}

/**
 * Warn before the engine has to.
 *
 * A pack built without a BMS has no sensors — not "sensors nobody reads", none — so
 * `schedule_fault` **refuses** a sensor fault outright ("fault targets GroupVoltage(0),
 * which this pack has no such sensor for"). It does not silently drop it. That is a
 * different mechanism from `facts.sensor_faults_dropped`, which counts faults a
 * *scenario file* declared against a pack whose BMS was switched off at build time.
 * Getting the two confused produces a panel that promises a fault will be dropped and
 * then shows an error instead.
 */
function sensorHint() {
  const isSensor = $("fault-kind").value.startsWith("Sensor");
  faultNote(
    isSensor && state.facts?.has_bms === false
      ? "This pack has no BMS, so it has no sensors and the engine will refuse a sensor fault. Switch the BMS on, or pick a fault that acts on the cells."
      : "",
  );
}

$("fault-inject").onclick = async () => {
  if (!state.backend) return;
  try {
    const delay = Math.max(0, Number($("fault-delay").value) || 0);
    const atS = (state.facts?.sim_time_s ?? 0) + delay;
    await Promise.resolve(state.backend.scheduleFault(atS, currentFault()));

    // "Queued", never "applied". `FaultQueue::take_due` fires on `at_s < t_end`, so a
    // fault dated now waits for the next *real* step — and `readNow`'s zero-length read
    // deliberately does not provide one, because a read must not mutate the pack.
    faultNote(
      `queued at t = ${fmtTime(atS)}. It fires on the next step, so nothing changes while the run is paused.`,
    );
    clearBanner();
  } catch (e) {
    // A rejection here is the engine refusing the fault — an out-of-topology cell, a
    // non-positive resistance, a sensor this pack does not have — and that belongs in
    // the banner, where the message can be read in full.
    showBanner(String(e.message ?? e));
    sensorHint();
  }
};

$("fault-clear").onclick = async () => {
  if (!state.backend) return;
  try {
    const count = await Promise.resolve(state.backend.clearFaults());
    faultNote(
      count === 0
        ? "nothing was queued."
        : `cleared ${count} fault(s) — the queue, plus anything that had already fired: the external short, every cell's internal leak, every sensor corruption. The one thing it cannot undo is a fired WeakCell, which stops being a fault the moment it lands and becomes part of that cell's manufacturing spread.`,
    );
    clearBanner();
  } catch (e) {
    showBanner(String(e.message ?? e));
  }
};

$("fault-clear-bms").onclick = async () => {
  if (!state.backend) return;
  try {
    const cleared = await Promise.resolve(state.backend.clearBmsFault());
    faultNote(
      cleared
        ? "latched BMS fault cleared; the contactor closes again if the pack is fit for it."
        : "there was no latched BMS fault to clear.",
    );
    clearBanner();
  } catch (e) {
    showBanner(String(e.message ?? e));
  }
};

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

// ---------------------------------------------------------------------------
// The guided path
// ---------------------------------------------------------------------------
//
// The page is a complete instrument panel and, without this, an empty lesson: every
// control `CLAUDE.md`'s pedagogy sentence asks for exists, and nothing tells a reader
// which one to turn or what to look at afterwards.
//
// A step is a **record**, and one renderer walks any of them. That is the whole design
// decision, and it is not tidiness: the reason this repo has two scenarios is that the
// `<option>` list is hardcoded, so every added scenario costs an HTML edit. Bespoke
// markup per lesson would reproduce that disease one layer up, in the part of the page
// most likely to grow. As records, a new lesson is an array entry — and when the
// scenario-listing route lands it serves this same shape.
//
// Fields:
//   scenario   file under /scenarios; a step reloads only when it must (below)
//   transport  "wasm" when the step cannot run over the socket, else undefined
//   demand     {mode, value} written into the sidebar inputs, which is all it takes:
//              `advance` and `readNow` re-read them on every call
//   ambient_c  written to the slider, then `applyEnv`
//   bms        true/false to force, null to leave whatever is loaded
//   speed_x    real-time multiplier; the slider is its base-10 log
//   until_s    absolute simulation time the step runs to, then pauses
//   reload     true for a step that must start from t = 0 whichever direction it is
//              entered from; omitted means "reload only when you must" (see applyStep)
//   watch      element ids to outline
//   prose      paragraphs; backticks become <code>
//   expect     what the reader should end up seeing. Prose, never an assertion — a page
//              that argues with a reader about a slider they moved on purpose is worse
//              than one that says nothing.

const LESSONS = [
  {
    id: "bare-curve",
    title: "One cell, and nothing else",
    scenario: "cc_discharge_lfp.toml",
    demand: { mode: "Current", value: 2 },
    ambient_c: 25,
    bms: null,
    speed_x: 800,
    until_s: 4200,
    watch: ["plot-v"],
    prose: [
      "A single LFP cell at 100 % charge, isothermal, with no BMS, no aging and nothing wrong with it. Every model beyond the equivalent circuit is switched off, so this trace is the ECM itself with nothing layered on top.",
      "2 A out of this cell is 0.87 C — it holds 2.303 Ah, not a round number, because the figure is the *usable* window fitted to the reference model rather than a marketing capacity. Watch the very first step: the voltage drops instantly by `I·R0`. That step is resistance, not charge — rest the cell and it comes straight back. The slower sag over the following minute is the RC pair filling.",
    ],
    expect:
      "A long, nearly flat middle. LFP's open-circuit voltage barely moves between 20 % and 80 % charge, which is exactly what will make its charge state so hard to measure three steps from now. Then the knee, at about 69 minutes, where the cell empties — and a `SOC_CLAMPED_LOW` flag, which is the coulomb counter reporting that it was asked for charge the cell no longer had.",
  },
  {
    id: "same-discharge-other-chemistry",
    title: "The same discharge, a different chemistry",
    scenario: "cc_discharge_nmc.toml",
    // 2.60 A on a 3.0 Ah cell is 0.868 C — the rate step 1 used, not the current. Same
    // fraction of the cell per hour means both runs empty at the same moment (4146 s
    // and 4148 s, measured), so the time axis is not one of the differences and the
    // voltage is the only thing left to look at.
    demand: { mode: "Current", value: 2.6 },
    ambient_c: 25,
    bms: null,
    speed_x: 800,
    until_s: 4200,
    watch: ["plot-v"],
    prose: [
      "The same experiment on an NMC cell: one cell, isothermal, nothing protecting it, discharged at the same 0.868 C. The scenario file differs from the last one in exactly one field — the chemistry id — which is what makes anything you see here attributable.",
      "This cell holds 3.0 Ah against the LFP cell's 2.303, so the same C-rate is 2.6 A rather than 2. Both empty within two seconds of each other; the shapes are what differ.",
    ],
    expect:
      "A curve that slopes the whole way down instead of sitting on a plateau. Between 90 % and 20 % charge this cell falls 481 mV — 4.030 V to 3.549 — where the LFP cell fell 168 across the same window. That ratio is the whole difference: on this chemistry, a voltage reading tells you the charge level; on LFP it barely does. Load `cc_discharge_lgm50` from the picker for a third (620 mV, a 5.15 Ah cell fitted from PyBaMM's Chen2020), then hold onto this — two steps from now an LFP state-of-charge estimator will refuse to correct itself, and this is why. At the end you get the same `SOC_CLAMPED_LOW` as before, and the same silence around it: nothing here is protecting this cell either, so it finishes at 2.92 V against a 3.00 V cutoff with no complaint from anything but the coulomb counter.",
  },
  {
    id: "pack-disagrees",
    title: "A pack disagrees with itself",
    scenario: "soft_short_under_a_lying_sensor.toml",
    demand: { mode: "Current", value: 6 },
    ambient_c: 25,
    bms: true,
    speed_x: 200,
    until_s: 300,
    watch: ["pack"],
    prose: [
      "Eight cells now — 4 in series, 2 in parallel — built with 2 % capacity and 3 % resistance scatter. That is a manufacturing spread, not a fault: no two cells off a line are identical.",
      "The pack solve does not average them. Each parallel group is solved as a node, so the two cells in a pair share a voltage and the current splits by state: the lower-resistance one takes more of the load than its twin, and then ages slightly faster for having done so.",
    ],
    expect:
      "Switch the grid between state of charge and overpotential. The SOC tiles start identical and fan out as the run goes — a few hundredths of a percent by the end — while the overpotential tiles differ from the first step, because resistance scatter shows up there first. Click a tile to pin its full state; the legend prints both ends of the scale, which is the pack's own min and max, not a fixed axis. Then compare the spread against the headline `soc (true)`: the aggregate sits right in the middle of the tiles and moves smoothly, saying nothing at all about the half-point of disagreement underneath it. That is not a flaw in the readout — it is what an aggregate *is*, and it is the reason this grid exists.",
  },
  {
    id: "belief-drifts",
    title: "What the BMS believes",
    scenario: "soft_short_under_a_lying_sensor.toml",
    demand: { mode: "Current", value: 6 },
    ambient_c: 25,
    bms: true,
    speed_x: 200,
    until_s: 600,
    watch: ["bms", "plot-soc"],
    prose: [
      "The engine knows every cell's true state. The BMS is not allowed to look at it. It sees one voltage per parallel group, two temperature probes for eight cells, and one current sensor — and this pack's current sensor reads 20 mA high with 10 mA of noise on top, on every single step.",
      "Its state of charge is coulomb counting on that sensor, so the error does not average out, it integrates. It also started 3 % wrong, because a BMS that has just been powered on does not know what it is holding.",
    ],
    expect:
      "A gap of about three points that simply never closes — the estimate sits above the truth from the first step to the last. Most of that is the boot error it started with, and the sensor offset adds only a fraction of a point over ten minutes; the offset is the mechanism that would run away over a long drive, and the boot error is what you can see today. What matters is that neither is corrected. The fix needs a rested pack and a sloped OCV curve, and the first step showed you how flat LFP's curve is through the middle: below a configured slope the estimator declines to correct rather than amplify its own noise. That is the design working, not failing — the pack simply cannot be asked how full it is.",
  },
  {
    id: "lying-sensor",
    title: "A short, and a sensor that hides it",
    scenario: "soft_short_under_a_lying_sensor.toml",
    demand: { mode: "Current", value: 6 },
    ambient_c: 25,
    bms: true,
    speed_x: 200,
    until_s: 1200,
    watch: ["bms", "flags"],
    prose: [
      "At t = 600 s this scenario springs a 5 Ω internal short on cell (1,0) and, in the same instant, lands a +120 mV offset on the voltage sensor for the group that cell sits in. Both are scheduled in the file; neither is an animation.",
      "A soft internal short drains the whole parallel group it sits in, not just its own cell, and it self-heats while doing it. The offset is sized to cover the sag.",
    ],
    expect:
      "Group 1's sensed voltage separates from truth by 120 mV and stays there — the only channel where a sensor fault is visible at all, since voltage and probe temperature are otherwise exact reads. The internal-short trace on the current plot lifts off zero, and the temperature grid finds a new hottest cell. The BMS sees four healthy groups the whole time and never trips.",
  },
  {
    id: "protection-on",
    title: "Protection, doing its job",
    scenario: "soft_short_under_a_lying_sensor.toml",
    demand: { mode: "Current", value: 40 },
    ambient_c: 25,
    bms: true,
    speed_x: 20,
    until_s: 60,
    watch: ["flags", "readouts"],
    prose: [
      "Fresh pack, and now an unreasonable demand. Eight cells in 4S2P is 4.61 Ah at pack level, and the chemistry is rated 3 C continuous, so the discharge limit lands just under 14 A. We are asking for 40.",
      "The BMS response is graduated: it clamps the demand long before it considers opening anything.",
    ],
    expect:
      "An `OC` flag, and the current readout pinned near 14 A while the demand box still reads 40. The demand is what you asked for; `i_actual` is what you got. Nothing in the sidebar changed to make that happen — the clamp is downstream of you.",
  },
  {
    id: "protection-off",
    title: "The same demand, with nothing watching",
    scenario: "soft_short_under_a_lying_sensor.toml",
    transport: "wasm",
    demand: { mode: "Current", value: 40 },
    ambient_c: 25,
    bms: false,
    speed_x: 40,
    until_s: 450,
    watch: ["flags", "plot-cv", "plot-t"],
    prose: [
      "Same pack, same 40 A, BMS removed. `CLAUDE.md` calls this a supported and interesting mode rather than an error, and it is the contrast the entire protection layer exists to justify.",
      "Notice what is *missing* as much as what happens. `OV`, `UV`, `OC` and `OT` are raised in one file — the BMS — and nowhere else. With it gone the pack does not merely fail to stop: it fails to say anything.",
    ],
    expect:
      "The current readout now obeys the demand exactly. Cell voltage dives well under the 2.0 V the datasheet allows, temperature climbs, and the only flag you will see is `SOC_CLAMPED_LOW` — the coulomb counter hitting its floor. That is ground truth reporting an impossibility, not a warning anyone issued.",
  },
  {
    id: "wearing-out-while-idle",
    title: "Nothing is happening, and it is still wearing out",
    scenario: "calendar_fade_hot.toml",
    demand: { mode: "Rest", value: 0 },
    ambient_c: 25,
    bms: null,
    // 10 000x, the top of the slider: this is the one lesson whose subject is slower
    // than a person. The multiplier changes steps per frame and never `dt`, so the
    // trajectory is bit-identical to watching it in real time for two days.
    speed_x: 10000,
    until_s: 200000,
    watch: ["plot-soh"],
    prose: [
      "A different pack: 4S2P LFP at 95 % charge, thermally coupled, aging switched on, nothing connected to it. The demand is `Rest`, so no current flows anywhere and the voltage and charge traces will be flat lines for the rest of this step.",
      "Calendar fade does not care. A cell sitting on a shelf loses capacity as `√t`, at an Arrhenius rate set by its temperature and a stress factor set by how full it is. Every coefficient behind that is a labelled placeholder in the chemistry file — this is that parameter set integrated honestly, not a claim about a real cell's shelf life.",
      "This runs to 200 000 s of simulation — about two and a half days — which is twenty seconds of watching at 10 000×. Then raise the ambient slider to 45 °C and press Run.",
    ],
    expect:
      "Two lines leaving 100 % together and never coming back: capacity down, resistance up, at exactly 1.5 points of resistance per point of capacity, because `CLAUDE.md` refuses to model one without the other. The curve bends hard and then flattens — a quarter of the way in, 0.53 points are gone; at the mark it is 1.06, twice the damage for four times the time, which is `√t` visible on screen. Then the ambient: 20 K buys 2.84 points over the next 200 000 s against the 1.06 the first leg cost, about 2.7×. Not the 3.6× two fresh packs would show at those temperatures — this pack has already aged, and `√t` means the same stress costs less the second time. Everything else on the page stays perfectly still throughout.",
  },
  {
    id: "two-legs",
    title: "Putting it back: the two legs of a charge",
    scenario: "cc_cv_charge_nmc.toml",
    // 1.5 A on a 3.0 Ah cell is 0.5 C, and 0.15 A is the 0.05 C cutoff a datasheet
    // would call "charge termination". Both are the page's, not the file's: a scenario
    // is an initial condition and never a demand program.
    demand: { mode: "CcCv", value: 1.5, v_cell: 4.2, taper: 0.15 },
    ambient_c: 25,
    bms: null,
    speed_x: 800,
    until_s: 6300,
    watch: ["plot-i", "plot-v"],
    prose: [
      "Eight steps of taking charge out; this one puts it back. The `CC-CV charge` demand mode is not a demand the engine has — it is two of them with a rule between them, which is where `CLAUDE.md` says a charge policy belongs: constant current until the pack reaches its voltage limit, then hold that voltage and let the current do what it likes.",
      "The same NMC cell as step 2, starting at 20 % instead of full. Nothing is protecting it, so the only thing that ends this charge is the current falling below the 0.15 A cutoff.",
      "The rule is checked every 10 s of *simulation* time, never once per frame — otherwise the speed multiplier would decide which step the legs change on, and the same experiment would take a different path at 1× and at 800×.",
    ],
    expect:
      "Watch the current trace, which is flat and negative — negative because positive is discharge everywhere on this page. It stays at −1.5 A for 5420 s while the terminal voltage climbs from 3.66 V to 4.20, and the cell reaches 95.3 %. Then the knee: nothing on the page moves, but the current starts falling by itself — 0.65 A at 5700 s, 0.27 A at 6000 — and it stops at 6210 s and 99.5 %. **The last leg is 13 % of the time for 5 % of the charge**, and that is the shape worth taking away: near the top, the only thing pushing current in is the gap between the cell's own open-circuit voltage and the limit, and the charging is what closes it.",
  },
  {
    id: "leg-that-is-not-there",
    title: "The same charge, on a cell with no second leg",
    scenario: "cc_cv_charge_lfp.toml",
    // 1.15 A is 0.5 C of 2.303 Ah — the same rate as the step before, so the rate is
    // not one of the differences.
    demand: { mode: "CcCv", value: 1.15, v_cell: 3.65, taper: 0.115 },
    ambient_c: 25,
    bms: null,
    speed_x: 800,
    until_s: 7000,
    watch: ["plot-v", "flags"],
    prose: [
      "The same charge at the same rate on the LFP cell from step 1, and the same controller. Wait for the knee.",
      "It does not come. The terminal voltage climbs to 3.6357 V and stops — 14.3 mV short of this cell's own 3.65 V limit — and the mode stays on constant current until the coulomb counter hits 100 % and raises `SOC_CLAMPED_HIGH` at 5769 s.",
      "The arithmetic is the plateau from step 1, read from the charging end: this file's open-circuit curve tops out at 3.60 V, its resistance is about 21 mΩ, and 0.5 C is 1.15 A — so 3.60 + 0.024 never reaches 3.65. **This cell fills up before it reaches its voltage limit**, which leaves the constant-voltage leg with nothing to do.",
    ],
    expect:
      "A voltage trace that flattens against a ceiling it never touches, and a status line that says `constant current` for the whole run. Two honest caveats. First, this is this parameter set's behaviour and not a claim about LFP cells: real ones are charged CC-CV to 3.65 V, and both numbers that decide it here — where the OCV table ends, how large R0 is — are hand-fitted values the chemistry file labels as such. Second, look at what happens *after* the flag: the charge counter stops but the current does not, because this engine models no overcharge chemistry and the energy simply goes nowhere. That is a hole in the model, left visible rather than papered over with an automatic stop. The next step is the same situation with a BMS in the way, which is what actually prevents it.",
  },
  {
    id: "what-protection-costs",
    title: "What the protection costs, and why",
    scenario: "cc_cv_charge_pack.toml",
    // The BMS toggle rebuilds the pack in-page, so this step needs the wasm backend —
    // the server builds whatever the scenario asked for and will not rewrite it.
    transport: "wasm",
    demand: { mode: "CcCv", value: 3.0, v_cell: 4.2, taper: 0.3 },
    ambient_c: 25,
    bms: true,
    speed_x: 800,
    until_s: 4100,
    watch: ["flags", "plot-cv", "pack"],
    prose: [
      "A 4S2P NMC pack with 2 % capacity and 3 % resistance scatter, protection on, passive balancing above 4.10 V per group. 3 A is 0.5 C. Everything the BMS does here, it does because the pack is being charged — a discharging pack in this repo raises none of these flags, which is why none of them have appeared in eight steps.",
      "Then run it again with the BMS unchecked, and compare where the two stop.",
    ],
    expect:
      "`BALANCING` at 3111 s as the first group crosses 4.10 V, then `OV` at 3986 s and the charge stops at **95.1 %**. With the BMS off the same pack runs on to **99.5 %**. Those 4.4 points are what protection costs, and the cell-voltage panel says why: one step before the trip the pack is at 16.775 V, only 25 mV short of the 16.80 V it is aiming for, but its eight cells are spread over 11 mV and the top one has already crossed 4.20. **They cannot all arrive together, so the pack stops when the first one does** — which is the entire argument for balancing. (The 130 mV gap you see *after* the trip is not imbalance; it is the IR drop vanishing when the current stops.) Two more things this pack will do: ask for 6 A and the BMS derates it to exactly 4.2 A — 0.7 C, its charge rating — with `OC` raised from the first step and the status line saying it was limited; or drag the ambient to −5 °C and watch the pack cool until `UT` inhibits the charge at 60.8 %, then switch the BMS off and see `PLATING_RISK` at the same instant with the charge carrying on regardless. That flag is the damage the inhibit exists to prevent.",
  },
  {
    id: "circuit-repeats-itself",
    title: "A pulse train, and a circuit that repeats itself",
    scenario: "pulse_train_ecm.toml",
    // 5.153 A is 1 C of this cell's 5.153 Ah. 60 s on / 600 s off: the long rest is the
    // point of the whole experiment, because it is what separates the part of the
    // response that vanishes with the current from the part that has to diffuse away.
    demand: { mode: "Pulse", value: 5.153, on_s: 60, off_s: 600 },
    ambient_c: 25,
    bms: null,
    // 100x, far slower than the discharge steps: the shape *within* one rest is the
    // subject here, and at 800x a 600 s rest is under a second of watching.
    speed_x: 100,
    until_s: 3300,
    // All three pulse steps compare their *first* tooth at 90 %, so none of them can
    // inherit a pack from a neighbour however it was entered.
    reload: true,
    watch: ["plot-v", "plot-i"],
    prose: [
      "A fifth demand mode, and the simplest one on the menu: `Current` for 60 s, `Rest` for 600, repeat. Unlike CC-CV it reads nothing back — the leg is a function of simulation time and nothing else, so there is no controller state a snapshot could fail to carry and nothing for a band to protect.",
      "The cell is the LG M50 from step 2's aside, at 90 % charge, running the equivalent circuit. Watch one tooth closely: the voltage drops the instant the current comes on, sags further while it flows, jumps back the instant it stops, and then climbs slowly for the rest of the ten minutes. Those are two different things — the jump is `I·R0`, which is resistance and is over immediately; the climb is the RC pairs discharging.",
    ],
    expect:
      "Five teeth, and the thing to measure is **the rebound**: how far the voltage climbs back during each rest. It is 74.8 mV on the first pulse and 74.8 mV on all four after it, identical to four decimal places — and 99.5 % of it has arrived within the first 300 s, so the second half of every rest is a flat line. That is not a property of this cell or these coefficients. It is what *linear and time-invariant* means: hand the same pulse to the same circuit and you get the same answer, every time, forever. Break the first tooth into its parts and it is 212.8 mV of sag, of which 132.8 mV returns the instant the current stops (`I·R0`, pure resistance), 74.8 mV climbs back slowly (the RC pairs), and the last 5.3 mV never returns at all — that is charge that has actually left the cell, and it is the open-circuit voltage itself stepping down. Measure the *depth* rather than the rebound and you will find teeth 4 and 5 are 9 mV deeper than teeth 1 to 3: that is not the circuit changing its mind, it is the `[ocv]` table's node at 85 % charge passing under the pulse, and it is exactly why the rebound is the number to watch. Hold onto 74.8 mV — the next two steps are both about what happens to it.",
  },
  {
    id: "particle-remembers",
    title: "The same train, on a model that knows the particle",
    scenario: "pulse_train_spm.toml",
    demand: { mode: "Pulse", value: 5.153, on_s: 60, off_s: 600 },
    ambient_c: 25,
    bms: null,
    speed_x: 100,
    until_s: 3300,
    reload: true,
    watch: ["plot-v"],
    prose: [
      "The same cell, the same 90 %, the same train. The scenario file differs from the last one in exactly one field — it adds `[pack.cell_model.Spm]` — and that field selects a model that has been in this engine since Phase 6 and that, until now, no client could reach.",
      "It does not solve a circuit. It solves lithium diffusing radially inside a particle, 20 shells deep, on parameters extracted verbatim from PyBaMM's Chen2020 rather than fitted. Pulling current out of it strips lithium from the particle *surface* faster than the inside can resupply it, and what the terminal reads is the surface, not the average.",
      "So compare the rebounds, not the depths. This file's `[r0]` and `[[rc]]` are labelled placeholders and the extracted parameter set's contact resistance is zero, so the millivolt heights of the two runs are not comparable — but how they *change* is.",
    ],
    expect:
      "The first rebound is 17.3 mV where the circuit's was 74.8. Ignore that gap; watch what it does. (The same tooth breaks down as 135.7 mV of sag: 113.9 mV back instantly, 17.3 mV climbing, and 4.5 mV that has not returned when the ten minutes are up — which for this model is *both* charge that has left and gradient still standing, and telling those two apart is the rest of this step.) Second pulse 24.3 mV, third 31.7, fourth 35.4, fifth **37.2** — it more than doubles across the run, and it has not stopped growing. Ten minutes of rest does not undo sixty seconds of diffusion, so a gradient is left standing when the next pulse arrives and the next one builds on it. **The cell remembers the earlier pulses.** The circuit could not: its 74.8 mV was the same five times because a linear system has no way to carry anything from one pulse to the next. Look at the tail of each rest, too — 8 % of the fifth rebound arrives in its final five minutes, against the circuit's 0.5 %, because what is relaxing is a concentration profile and not a capacitor. (This half is a comparison against placeholders: the RC pairs are 9 s and 72 s because someone picked round numbers. What is not picked is the particle's own timescale, which is its radius squared over a measured diffusivity.)",
  },
  {
    id: "three-times-the-current",
    title: "Three times the current, and two answers to it",
    scenario: "pulse_train_spm.toml",
    // 15.459 A is 3 C. The mark is *below* the previous step's, which is what makes
    // `reload` explicit rather than inherited: the inequality in `applyStep` would
    // reload on the way in and not on the way back, and this run has to start at 90 %
    // in both directions for its first tooth to be comparable with step 13's.
    demand: { mode: "Pulse", value: 15.459, on_s: 60, off_s: 600 },
    ambient_c: 25,
    bms: null,
    speed_x: 100,
    until_s: 1980,
    reload: true,
    watch: ["plot-v", "readouts"],
    prose: [
      "The same file, back at 90 %, with the pulse current tripled to 3 C. Three pulses is enough.",
      "The question is the one a modeller actually asks: if I know what this cell does at 1 C, do I know what it does at 3 C? For the circuit of step 12 the answer is yes, trivially and by construction — triple the current and every term triples, because every term is a resistance times a current. Its jump goes 132.8 → 397.3 mV and its slow climb 74.8 → 224.3 mV: **×2.99 and ×3.00**.",
    ],
    expect:
      "The particle gives two different answers to the same question. Its instantaneous jump goes 113.9 → 213.2 mV — **×1.87**, not ×3 — because that part is charge-transfer kinetics, and kinetics saturate: overpotential goes as the *arcsinh* of current, so asking harder buys less each time. Its slow climb goes 17.3 → 103.9 mV, which is **×6.01** — accelerating, because the open-circuit potential is a curved function of surface composition and driving the surface further out of balance costs more than proportionally. One part gentler than the circuit, one part far harsher, from the same three-fold demand. And the total sag, which is the only one of the three you can read off the plot without pausing, lands at ×2.48 — a number that looks like mild sub-linearity and conceals both effects underneath it. **No single resistance can be 1.87 and 6.01 at once**, which is the whole argument for a model with an inside. Two footnotes. This costs about 8× the circuit's arithmetic per step at 20 shells (0.90 µs against 0.11 on one cell), which is why it is not the default. And do not run this one to empty: past the charge clamp the particle model pins near 0.4 V while the circuit stops at 1.79, because the surface concentration falls off the bottom of its table — a hole in the model, left visible rather than papered over.",
  },
  {
    id: "one-step-that-got-through",
    title: "A short across the terminals, and the one step that got through",
    scenario: "external_short_30_milliohm.toml",
    demand: { mode: "Rest", value: 0 },
    ambient_c: 25,
    bms: true,
    // The first step in this path whose headline quantity *is* one step long, so the
    // step length stops being whatever the reader last typed. Every earlier step's
    // numbers assume 0.5 s too; these are the two that would be wrong by a factor.
    dt: 0.5,
    speed_x: 10,
    until_s: 90,
    watch: ["flags", "plot-i", "readouts"],
    prose: [
      "Back to the circuit model, and to a pack with a BMS. Nothing is connected to this one: the demand is `Rest` for the whole run, and for the first sixty seconds nothing whatever happens.",
      "Then the file springs a **30 milliohm short across the pack's terminals** — not across a cell, like the leak in step 5, but across the whole pack, on the load side of the main contactor. That last detail is the entire design of this experiment: the contactor is upstream of the fault, so opening it is a repair. `sim-core` puts it there deliberately.",
      "Eleven steps of protection have derated a demand. This one cannot be derated, because there is no demand — and that is what the contactor is for.",
    ],
    expect:
      "One tooth on the current plot and then nothing. The pack draws **183.84 A** for a single step with *no flag raised at all*, then `UV` and `CONTACTOR_OPEN` together, and the current is zero from there on. The pack was at 90.00 % when the short landed and it is at **89.44 %** for the rest of the run — half a percent, and 0.96 K, is the whole cost of a dead short. The step that got through is not a bug: protection decides from sensors sampled at the end of the *previous* step, which still read a resting 3.3142 V per group. The frame taken after the spike reads **1.3336 V**, well under the 1.85 V that this file calls a fault rather than an operating point, and the next step latches. **The lag is one step long, so it is yours to set.** Put `dt` up to 5 s and press **Restart**, which rebuilds this pack at t = 0 from the same file — fault and all — without touching the controls you just set, and then Run. Not Back-then-Next: that re-applies the whole step, `dt` included, and puts it back to 0.5. The spike is the same 183.84 A, because a resistive sag is instantaneous and does not care how long you look at it; it simply lasts ten times longer, and the damage is exactly proportional. 0.56 points at 0.5 s, **5.57 at 5 s**, 11.14 at 10 s, where the cell ends 19 K hotter instead of 1. Put it back to 0.5 before moving on — the pulse steps behind you count their legs in whole timesteps. Then the reset, which is two buttons, an order, and a Run. Nothing advances while the page is paused, so **Clear latched BMS fault** on its own only unlatches: press Run and the short, still connected, delivers a *second* 184 A tooth and it latches straight back. Do it the other way round — **Clear queued** first, which despite its name is a repair and removes the short that already fired, then **Clear latched BMS fault** — and the pack simply sits there at 13.16 V with nothing flowing. A latch that cleared itself when the voltage recovered would be a thermostat, not a protection device.",
  },
  {
    id: "nothing-to-clamp",
    title: "The same short, three times weaker, and nothing to clamp",
    scenario: "external_short_100_milliohm.toml",
    // The BMS toggle rebuilds the pack in-page, so the contrast at the end of this step
    // needs the wasm backend — same reason step 11 switches.
    transport: "wasm",
    demand: { mode: "Rest", value: 0 },
    ambient_c: 25,
    bms: true,
    dt: 0.5,
    speed_x: 20,
    until_s: 200,
    watch: ["flags", "plot-t", "plot-soc"],
    prose: [
      "The twin file. Same pack, same seed, same fault at the same second, and the only difference is that the short is 100 milliohms instead of 30. Three times the resistance is a third of the sag — and a third of the sag does not reach the margin that saved the pack a moment ago.",
      "So watch how long the BMS says nothing. There are two rungs on this ladder and the weaker fault slips between them: the sag is not deep enough for the hard limit, and the *soft* rung has nothing to act on either, because derating shrinks the window of current the **demand** is allowed — and a short is not a demand. Nobody asked for this current. There is no `OC`, because over-current is judged against what you requested, and you requested `Rest`.",
      "A clamp on a number the fault does not go through cannot stop the fault. Only the contactor is in the current's way.",
    ],
    expect:
      "86 amps, and **73 seconds of no flags at all**. The groups sag to 2.13 V — above `v_min` itself, so not even a soft under-voltage — while the charge trace falls off a cliff. The first thing the BMS says is `OT` at t = 133.5 s, by which point the pack is already at 51 %. It latches at **t = 156.0 s**, 96 s after the short, at **39.62 %**: against the twin's half a percent, this fault costs **fifty points**, because the rung that caught the bigger short was never in this one's way. Two details worth pausing on. The trip is a *probe* crossing 343.15 K — the two probes sit on corner cells, and the cell that is genuinely hottest is at 344.52 K when it fires, so protection is late by 1.3 K of somebody else's temperature. And the pack peaks at 344.6 K here against 299.1 K in the twin: the weaker short is the more dangerous one, which is not the intuition. Now uncheck the BMS and run it again. The trajectory is identical — the same 93.29 A on the first frame after the fault, the same 87.02 A thirty seconds later — right up to the instant it isn't, and then it simply carries on: `SOC_CLAMPED_LOW` at 235.5 s and 375 K, still climbing. Read that tail with one caveat, honestly: past the clamp the current does not stop, because this engine models no empty electrode and a cell at SOC 0 keeps sourcing at `OCV(0)` forever. It is the discharge face of the same hole step 10 shows on the charging side, and the heat after that is the model's, not the pack's.",
  },
];

/** Authored strings, so the escape is belt-and-braces; the backticks are the point. */
function proseHtml(s) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/`([^`]+)`/g, "<code>$1</code>");
}

/**
 * Outline exactly the panels this step is about, and nothing from the last one.
 *
 * Deliberately does **not** scroll the first one into view, which was tried and removed:
 * the path panel sits at the top of the column, so scrolling down to `#pack` or a plot
 * pushes the prose off screen — it makes the reader look before they have read, which is
 * backwards for a page whose whole premise is "try this, *then* watch that". The outline
 * persists, so it is waiting when they scroll. The cost used to be that on a short
 * viewport the plot-watching steps pointed at something below the fold; the plots are now
 * a two-column grid a quarter the area each, so on a wide window they are all on screen
 * with the prose. On a narrow one they fall back to the old stack and the old cost.
 */
function setWatch(ids) {
  for (const el of document.querySelectorAll(".watching")) el.classList.remove("watching");
  for (const id of ids ?? []) $(id)?.classList.add("watching");
}

function renderStep() {
  const L = LESSONS[path.i];
  $("path-title").textContent = L.title;
  $("path-where").textContent = `step ${path.i + 1} of ${LESSONS.length}`;
  $("path-prose").innerHTML = L.prose.map((p) => `<p>${proseHtml(p)}</p>`).join("");
  $("path-expect").innerHTML =
    `<span class="k">what to watch</span>${proseHtml(L.expect)}`;
  $("path-back").disabled = path.i === 0 || path.busy;
  $("path-next").disabled = path.i === LESSONS.length - 1 || path.busy;

  const t = state.facts?.sim_time_s ?? 0;
  // The transport note is appended in *every* branch rather than only when paused. The
  // switch happens on the way in, so the moment a reader most needs to be told their
  // transport changed under them is while the step is still running.
  const moved = path.switchedTransport
    ? " Switched to the in-page engine for this step: the server builds the pack its scenario asked for and will not rebuild it without a BMS."
    : "";
  // Three states, not two. A reader who takes up the invitation to pause leaves
  // `path.until` set while `state.running` goes false, and a note that still said
  // "running to…" would be contradicted by the button right next to it — the note would
  // be inviting the one action that falsifies it.
  const where = path.busy
    ? "setting up…"
    : path.until === null
      ? `paused at ${fmtTime(t)} of simulation. Nothing is advancing.`
      : state.running
        ? `running to t = ${fmtTime(path.until)}. Pause whenever you like — every control stays yours, and Back then Next re-applies this step from scratch.`
        : `paused at ${fmtTime(t)}, part-way to t = ${fmtTime(path.until)}. Press Run to carry on, or Next to move on without finishing.`;
  $("path-note").textContent = where + moved;
}

/**
 * Put the page into the state a step describes.
 *
 * Everything here goes through the controls that already exist rather than reaching past
 * them, so the sidebar and the path can never disagree about what the pack is doing. A
 * step re-applies its whole set on the way in, which is also what makes Back-then-Next a
 * repair for a reader who moved a slider mid-lesson.
 */
async function applyStep(L) {
  path.until = null;
  path.busy = true;
  path.switchedTransport = false;
  state.running = false;
  $("run").textContent = "Run";
  renderStep();
  setWatch(L.watch);

  try {
    // Transport is part of the control set, so a step that cannot run over the socket
    // switches it rather than showing the reader a lesson that quietly does nothing.
    // The note says so, in every branch, because the switch happens on the way in.
    if (L.transport === "wasm" && $("use-socket").checked) {
      $("use-socket").checked = false;
      path.switchedTransport = true;
    }

    // Reload only when there is no other way to reach the described state. Simulation
    // time does not run backwards, so a step whose mark is behind us needs a fresh pack;
    // a step ahead of us on the same scenario just keeps going, which is why lessons 2
    // to 4 are one continuous run.
    // `reload: true` is for a step whose prose describes a run **from t = 0** and which
    // therefore cannot inherit a pack. Without it the rule below is an inequality that
    // only holds in one direction: the pulse steps 12-14 are the first in this path
    // whose marks do not increase, and Back from 14 (t = 1980) into 13 (mark 3300)
    // satisfies neither clause — so 13 would arm from t = 1980 on a pack three 3 C
    // pulses into its life, while its prose talks about a first tooth at 90 %. Steps
    // 2-5 never met this because their marks ascend.
    const reload =
      !state.backend ||
      path.switchedTransport ||
      L.reload === true ||
      $("scenario").value !== L.scenario ||
      (state.facts?.sim_time_s ?? 0) > L.until_s;
    $("scenario").value = L.scenario;
    if (reload) await loadScenario();

    // `$("bms").onchange` is `() => $("reset").click()`, and a click cannot be awaited —
    // so the handler is called directly and its promise awaited. Same code path, no
    // shadow of it. Rebuilding at t = 0 is the documented behaviour of that toggle, not
    // a side effect being routed around.
    if (L.bms !== null && !$("bms").disabled && $("bms").checked !== L.bms) {
      $("bms").checked = L.bms;
      await $("reset").onclick();
    }

    $("demand-mode").value = L.demand.mode;
    $("demand-value").value = String(L.demand.value);
    // `value` is the charge current in a CC-CV step; the other two fields are only ever
    // read in that mode, so a step that does not use them leaves them where they were.
    if (L.demand.mode === "CcCv") {
      $("cccv-i").value = String(L.demand.value);
      $("cccv-v").value = String(L.demand.v_cell);
      $("cccv-taper").value = String(L.demand.taper);
    }
    // Same treatment for the train: `value` is the pulse current, and the two leg
    // lengths are its own. A step that is not a pulse step leaves them alone, so a
    // reader who has been fiddling keeps their fiddling.
    if (L.demand.mode === "Pulse") {
      $("pulse-i").value = String(L.demand.value);
      $("pulse-on").value = String(L.demand.on_s);
      $("pulse-off").value = String(L.demand.off_s);
    }
    applyDemandMode();
    // Optional, and only the steps whose subject is step-sized set it. Every other step
    // leaves the box where the reader left it — the same treatment the CC-CV and pulse
    // fields get, and for the same reason: a step should not silently undo fiddling it
    // does not depend on.
    if (L.dt !== undefined) $("dt").value = String(L.dt);
    $("ambient").value = String(L.ambient_c);
    applyEnv();
    $("speed").value = String(Math.log10(L.speed_x));
    $("speed").oninput();

    // So the readouts answer for the demand just dialled in instead of the previous one.
    // `dt = 0` does not move the pack.
    await readNow();

    // Exit can land while this was still awaiting a load. Arming a run now would leave
    // the page stepping behind a panel that is no longer on screen to explain it.
    if (!path.on) return;

    path.until = L.until_s;
    state.accumulator = 0;
    state.lastWallMs = null;
    state.running = true;
    $("run").textContent = "Pause";
  } catch (e) {
    // A step that cannot set itself up leaves the path where it is and says why, rather
    // than arming a run over a pack that is not the one the prose describes.
    showBanner(String(e.message ?? e));
  } finally {
    // Unconditional: without it, one throw anywhere above leaves `busy` set and wedges
    // Back and Next permanently — a dead path with no error on screen, which is a worse
    // failure than the one that caused it.
    path.busy = false;
    renderStep();
  }
}

/** Called from `frame` when a step reaches its mark. */
function pathArrived() {
  path.until = null;
  state.running = false;
  $("run").textContent = "Run";
  renderStep();
}

async function gotoStep(i) {
  if (path.busy) return;
  path.i = Math.max(0, Math.min(LESSONS.length - 1, i));
  await applyStep(LESSONS[path.i]);
}

$("path-start").onclick = async () => {
  path.on = true;
  $("path").className = "show";
  $("path-start").textContent = "Restart the path";
  await gotoStep(0);
};
$("path-next").onclick = () => gotoStep(path.i + 1);
$("path-back").onclick = () => gotoStep(path.i - 1);
$("path-exit").onclick = () => {
  path.on = false;
  path.until = null;
  state.running = false;
  $("run").textContent = "Run";
  $("path").className = "";
  $("path-start").textContent = "Start — 8 steps";
  setWatch([]);
};

window.addEventListener("beforeunload", () => state.backend?.close());

// The picker first, because `loadScenario` reads its value. A failure here is not fatal
// — the hand-written options in `index.html` still name scenarios this server serves —
// so it is recorded and stepped over rather than thrown out of boot, which would leave
// the page with no pack at all.
//
// Recorded, and deliberately **not** shown with `showBanner`: `loadScenario` opens with
// `clearBanner`, so a banner raised here would be wiped one line later without ever
// having been on screen. That is not hypothetical — this page shipped exactly that bug
// once, in a version check whose warning erased itself. The note beside the picker is
// rewritten on every load, so a failure parked there stays visible instead.
try {
  await loadScenarioList();
} catch (e) {
  state.scenarioListError = String(e.message ?? e);
}
await loadScenario();
