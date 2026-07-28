#!/usr/bin/env node
//
// experiment.mjs — run a full battery experiment against `sim-server` over WebSocket.
//
// This is the "external script" Phase 4's exit criterion is about, in a form you can
// copy. It needs **no dependencies**: Node 22+ ships `fetch` and `WebSocket` as
// globals, so there is nothing to install and nothing to build.
//
//   1. start the server:   cargo run -p sim-server
//   2. run this script:    node examples/experiment.mjs
//
// It writes `experiment.csv` (one row per reported step) and
// `experiment-snapshot.json` (the pack, mid-run), and prints a summary.
//
// ---------------------------------------------------------------------------
// What the experiment is
// ---------------------------------------------------------------------------
// The shipped scenario `soft_short_under_a_lying_sensor.toml` is a 4S2P LFP pack
// with a thermal network and a full BMS. At t = 600 s a 5-ohm internal short
// appears on one cell and, in the same instant, a +120 mV offset lands on that
// group's voltage sensor — so the fault is real and the instrument that would show
// it is lying. The BMS never trips. Ground truth and the BMS estimate diverge, and
// watching that gap open is the point.
//
// The run is three legs, at a constant 2.9 A discharge:
//
//   leg 1   t =    0 →  300 s   healthy; the pack before anything goes wrong
//   ~~~~~   snapshot over REST, written to disk, posted back, run resumed
//   leg 2   t =  300 → 1800 s   the fault fires at 600 s, i.e. *after* the restore
//   leg 3   t = 1800 → 3600 s   same fault, colder room (SetEnv to 5 degC)
//
// The split is deliberately before the fault: the scenario's faults are queued at
// t = 600 s and fire in leg 2, so a restored pack that had silently dropped its
// pending queue would produce a visibly different — and much duller — experiment.
//
// ---------------------------------------------------------------------------
// Three things about the protocol that are worth knowing before you read the code
// ---------------------------------------------------------------------------
// * **`dt` is always yours.** Every stepping command carries an explicit `dt` and a
//   step count. The server never derives a timestep from when your message arrived,
//   so network jitter cannot enter the trajectory and a 10 000-step fast-forward is
//   one message rather than 10 000. That is why there is no `POST /step`.
//
// * **Commands and events are externally tagged**: `{"Step": {...}}`, `{"Current": 2.9}`,
//   and a variant that carries nothing is a bare string — `"Ping"`, `"Rest"`, `"Pong"`.
//   `eventKind` below is the whole of what a client needs for that.
//
// * **`flags` is a string, not a bitmask.** `"OV | PLATING_RISK"`, or `""` for none.
//   Split on `" | "` and treat the empty string as the empty set rather than as a
//   flag whose name happens to be empty.
//
// Sign convention throughout: **positive current = discharge**.

// The HTTP/WebSocket contract this script was written against. The server reports
// its own at `GET /` and in the hello frame; a mismatch is a warning rather than a
// refusal, because a bump means "a client *may* break", not "this one has".
const EXPECTED_API_VERSION = 1;

const DEFAULTS = {
  base: "http://127.0.0.1:8080",
  scenario: "soft_short_under_a_lying_sensor.toml",
  out: "experiment.csv",
  snapshotOut: "experiment-snapshot.json",
};

const USAGE = `\
usage: node examples/experiment.mjs [options]

  --base URL        server to talk to (default ${DEFAULTS.base})
  --scenario NAME   scenario served at <base>/scenarios/NAME
                    (default ${DEFAULTS.scenario})
  --file PATH       read the scenario from a local file instead of the server
  --out PATH        CSV output (default ${DEFAULTS.out})
  --snapshot PATH   snapshot output (default ${DEFAULTS.snapshotOut})
  --keep            do not DELETE the session on the way out, so you can poke at
                    it with curl afterwards
`;

import { readFile, writeFile } from "node:fs/promises";

// ---------------------------------------------------------------------------
// The experiment's shape, in one place so the numbers can be read together.
// ---------------------------------------------------------------------------
const DT_S = 0.5;
const DEMAND = { Current: 2.9 };
// Report every 20th step: 40 samples per simulated minute is far more than a plot
// needs, and decimation drops *reports*, never steps — the trajectory is identical
// either way. This is what keeps a long run from being gigabytes of JSON.
const REPORT_EVERY = 20;
const LEG_1_STEPS = 600; //     0 →  300 s, healthy, and snapshotted at the end
const LEG_2_STEPS = 3_000; //  300 → 1800 s, the fault fires at 600 s, after the restore
const LEG_3_STEPS = 3_600; // 1800 → 3600 s, the same fault in a cold room
const COLD_ROOM_K = 278.15; // 5 degC

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------
function parseArgs(argv) {
  const args = { ...DEFAULTS, file: null, keep: false };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    const value = () => {
      const v = argv[++i];
      if (v === undefined) fail(`${flag} needs a value\n\n${USAGE}`);
      return v;
    };
    switch (flag) {
      case "--base": args.base = value().replace(/\/+$/, ""); break;
      case "--scenario": args.scenario = value(); break;
      case "--file": args.file = value(); break;
      case "--out": args.out = value(); break;
      case "--snapshot": args.snapshotOut = value(); break;
      case "--keep": args.keep = true; break;
      case "-h": case "--help": process.stdout.write(USAGE); process.exit(0); break;
      default: fail(`unknown argument ${JSON.stringify(flag)}\n\n${USAGE}`);
    }
  }
  return args;
}

function fail(message) {
  process.stderr.write(`experiment.mjs: ${message}\n`);
  process.exit(1);
}

// ---------------------------------------------------------------------------
// REST
// ---------------------------------------------------------------------------

/** `fetch`, with the server's error body surfaced instead of a bare status code. */
async function request(base, method, path, { body, contentType } = {}) {
  const url = `${base}${path}`;
  let response;
  try {
    response = await fetch(url, {
      method,
      body,
      headers: contentType ? { "content-type": contentType } : undefined,
    });
  } catch (cause) {
    // The overwhelmingly likely cause, said plainly. A stack trace about
    // ECONNREFUSED does not tell anyone to start the server.
    fail(
      `could not reach ${url}\n` +
        `  ${cause.message}\n` +
        `  is the server running? start it with:  cargo run -p sim-server`,
    );
  }
  const text = await response.text();
  if (!response.ok) {
    fail(`${method} ${path} -> HTTP ${response.status}\n  ${text}`);
  }
  return text;
}

async function requestJson(base, method, path, options) {
  return JSON.parse(await request(base, method, path, options));
}

// ---------------------------------------------------------------------------
// The WebSocket client
// ---------------------------------------------------------------------------

/**
 * Every event is one of `{"Name": {...}}` or a bare `"Name"` for a variant that
 * carries nothing. This collapses both to the name.
 */
function eventKind(event) {
  return typeof event === "string" ? event : Object.keys(event)[0];
}

/** The payload of `{"Name": payload}`, or `null` for a bare `"Name"`. */
function eventBody(event) {
  return typeof event === "string" ? null : event[eventKind(event)];
}

/**
 * A queue over the socket, so the experiment below can read as a sequence of
 * awaits rather than as a state machine of callbacks.
 *
 * Events arrive strictly in order and the server never reorders commands, so a
 * plain FIFO with pending waiters is the whole client.
 */
class Session {
  #pending = []; // resolvers waiting for an event
  #queued = []; // events that arrived with nobody waiting
  #closed = null;

  constructor(socket) {
    this.socket = socket;
    socket.addEventListener("message", (message) => {
      this.#deliver({ event: JSON.parse(message.data) });
    });
    socket.addEventListener("close", (close) => {
      this.#closed = `socket closed (code ${close.code}${close.reason ? `: ${close.reason}` : ""})`;
      this.#deliver({ error: this.#closed });
    });
    socket.addEventListener("error", () => {
      this.#deliver({ error: this.#closed ?? "socket error" });
    });
  }

  static async attach(base, id) {
    const url = `${base.replace(/^http/, "ws")}/sessions/${id}/ws`;
    const socket = new WebSocket(url);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", () => reject(new Error(`could not open ${url}`)), {
        once: true,
      });
    });
    return new Session(socket);
  }

  #deliver(item) {
    const waiter = this.#pending.shift();
    if (waiter) waiter(item);
    else this.#queued.push(item);
  }

  /** The next event, with a protocol `Error` turned into a thrown one. */
  async next() {
    const item = this.#queued.shift() ?? (await new Promise((r) => this.#pending.push(r)));
    if (item.error) throw new Error(item.error);
    if (eventKind(item.event) === "Error") {
      const { code, message } = eventBody(item.event);
      throw new Error(`server refused the command [${code}]: ${message}`);
    }
    return item.event;
  }

  /** The next event, asserted to be of a particular kind. */
  async expect(kind) {
    const event = await this.next();
    if (eventKind(event) !== kind) {
      throw new Error(`expected ${kind}, got ${JSON.stringify(event).slice(0, 200)}`);
    }
    return eventBody(event);
  }

  send(command) {
    this.socket.send(JSON.stringify(command));
  }

  close() {
    this.socket.close();
  }
}

// ---------------------------------------------------------------------------
// Stepping
// ---------------------------------------------------------------------------

/**
 * Frames a batch of `n` steps decimated by `k` will produce: every `k`-th step,
 * **plus always the last one** — which is why this is a ceiling and not a plain
 * division, and why your final sample is the true end state rather than wherever
 * the modulus happened to land.
 */
const frameCount = (n, k) => Math.ceil(n / k);

/**
 * Advance the pack and collect the reported frames.
 *
 * The caps are checked here against the ones the *server* reported in its hello
 * frame, rather than against numbers copied into this file. They are configurable
 * per server, so a hardcoded copy would be wrong for someone else's deployment —
 * and being told "size your batches" beats discovering the limit by being rejected.
 */
async function step(session, limits, { nSteps, demand, env, dt = DT_S, reportEvery = REPORT_EVERY }) {
  if (nSteps > limits.max_steps_per_command) {
    throw new Error(
      `${nSteps} steps exceeds this server's cap of ${limits.max_steps_per_command}; ` +
        `split the run across messages (the trajectory is identical either way)`,
    );
  }
  const frames = frameCount(nSteps, reportEvery);
  if (frames > limits.max_frames_per_reply) {
    throw new Error(
      `${frames} frames exceeds this server's cap of ${limits.max_frames_per_reply}; ` +
        `raise report_every_n_steps (currently ${reportEvery})`,
    );
  }

  session.send({
    Step: {
      dt,
      n_steps: nSteps,
      demand,
      // `null` uses the session's standing environment. A value here would override
      // it for this batch only, without persisting — see `SetEnv` below.
      env: env ?? null,
      report_every_n_steps: reportEvery,
    },
  });

  const collected = [];
  for (;;) {
    const event = await session.next();
    const kind = eventKind(event);
    if (kind === "Telemetry") {
      collected.push(eventBody(event));
    } else if (kind === "BatchComplete") {
      const done = eventBody(event);
      // `BatchComplete` is the barrier: every frame this batch will produce has
      // already arrived when it lands. A batch delivers all of its frames or the
      // session errors — a live view may drop frames, a result may not.
      if (collected.length !== done.reported) {
        throw new Error(`batch reported ${done.reported} frames but ${collected.length} arrived`);
      }
      return { frames: collected, simTimeS: done.sim_time_s };
    } else {
      throw new Error(`unexpected ${kind} during a batch`);
    }
  }
}

/**
 * Read telemetry **without advancing**: `dt = 0` is deliberately legal, and one
 * zero-length step does not mutate any state. It is how a client answers "what is
 * the pack doing right now" on connect, or straight after a restore.
 *
 * Note the frame it returns is not a copy of the previous one: telemetry is
 * computed from start-of-step state, so a rate like `q_gen_w` here describes the
 * pack *now* rather than what it was doing during the last real step.
 */
async function readWithoutAdvancing(session, limits) {
  const { frames } = await step(session, limits, {
    dt: 0,
    nSteps: 1,
    demand: "Rest",
    reportEvery: 1,
  });
  return frames[0];
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------
const CSV_COLUMNS = [
  ["leg", (f, leg) => leg],
  ["sim_time_s", (f) => f.sim_time_s],
  ["v_terminal_v", (f) => f.telemetry.v_terminal],
  ["i_actual_a", (f) => f.telemetry.i_actual],
  ["soc_true", (f) => f.telemetry.soc_true],
  ["soc_bms", (f) => f.telemetry.soc_bms ?? ""],
  ["v_cell_min_v", (f) => f.telemetry.v_cell_min],
  ["v_cell_max_v", (f) => f.telemetry.v_cell_max],
  ["t_min_k", (f) => f.telemetry.t_min],
  ["t_max_k", (f) => f.telemetry.t_max],
  ["soh_capacity", (f) => f.telemetry.soh_capacity],
  ["soh_resistance", (f) => f.telemetry.soh_resistance],
  ["q_gen_w", (f) => f.telemetry.q_gen_w],
  ["i_internal_short_a", (f) => f.telemetry.i_internal_short_a],
  ["i_balancing_a", (f) => f.telemetry.i_balancing_a],
  ["flags", (f) => f.telemetry.flags],
];

function toCsv(rows) {
  const lines = [CSV_COLUMNS.map(([name]) => name).join(",")];
  for (const { frame, leg } of rows) {
    lines.push(CSV_COLUMNS.map(([, read]) => read(frame, leg)).join(","));
  }
  return `${lines.join("\n")}\n`;
}

/** `"OV | UV"` -> `["OV", "UV"]`; `""` -> `[]`, which is the case worth getting right. */
const parseFlags = (flags) => (flags === "" ? [] : flags.split(" | "));

const degC = (kelvin) => (kelvin - 273.15).toFixed(2);
const pct = (fraction) => (fraction * 100).toFixed(1);

// ---------------------------------------------------------------------------
// The experiment
// ---------------------------------------------------------------------------
async function main() {
  if (typeof WebSocket === "undefined" || typeof fetch === "undefined") {
    fail(
      `this script needs Node 22 or newer for its built-in WebSocket and fetch ` +
        `(running ${process.version})`,
    );
  }
  const args = parseArgs(process.argv.slice(2));

  // ---- who are we talking to -------------------------------------------------
  const root = await requestJson(args.base, "GET", "/");
  console.log(`server            ${args.base}`);
  console.log(`api_version       ${root.api_version}  (versions this HTTP/WS contract)`);
  console.log(`snapshot_version  ${root.snapshot_version}  (versions the engine's pack layout)`);
  if (root.api_version !== EXPECTED_API_VERSION) {
    console.warn(
      `warning: this script was written against api_version ${EXPECTED_API_VERSION}; ` +
        `the server speaks ${root.api_version}, so a field it reads may have moved`,
    );
  }

  // ---- the scenario ----------------------------------------------------------
  // Fetched from the server by default: it serves `scenarios/` as text for exactly
  // this reason (the browser page has no filesystem either), so the script needs
  // nothing but a URL. `--file` reads one off disk instead.
  const scenarioToml = args.file
    ? await readFile(args.file, "utf8")
    : await request(args.base, "GET", `/scenarios/${args.scenario}`);
  console.log(`scenario          ${args.file ?? `${args.base}/scenarios/${args.scenario}`}`);

  // ---- create the session ----------------------------------------------------
  // TOML is the on-disk format, so it is the body format too. `application/json`
  // with the Scenario struct works identically.
  const created = await requestJson(args.base, "POST", "/sessions", {
    body: scenarioToml,
    contentType: "application/toml",
  });
  const id = created.id;
  console.log(`session           ${id}  (${created.pack.series}S${created.pack.parallel}P)`);

  const rows = [];
  let session;
  try {
    // ---- attach ---------------------------------------------------------------
    session = await Session.attach(args.base, id);
    const hello = await session.expect("Hello");
    if (hello.role !== "writer") {
      throw new Error(
        `attached as ${hello.role}: something else already holds this session's writer ` +
          `slot. A session has one writer; later sockets observe.`,
      );
    }
    const limits = hello.limits;
    console.log(
      `role              ${hello.role}  ` +
        `(caps: ${limits.max_steps_per_command} steps, ${limits.max_frames_per_reply} frames/reply)`,
    );
    console.log(
      `standing env      ${degC(hello.env.t_ambient)} degC ambient, ` +
        `${hello.env.t_coolant === null ? "no coolant" : `${degC(hello.env.t_coolant)} degC coolant`}`,
    );

    // ---- t = 0, without advancing ---------------------------------------------
    const initial = await readWithoutAdvancing(session, limits);
    rows.push({ frame: initial, leg: 0 });
    console.log(
      `\nt = 0 s           ${initial.telemetry.v_terminal.toFixed(3)} V, ` +
        `SOC ${pct(initial.telemetry.soc_true)} % true / ` +
        `${pct(initial.telemetry.soc_bms)} % as the BMS estimates it`,
    );

    // ---- leg 1: healthy --------------------------------------------------------
    const leg1 = await step(session, limits, { nSteps: LEG_1_STEPS, demand: DEMAND });
    rows.push(...leg1.frames.map((frame) => ({ frame, leg: 1 })));
    console.log(`leg 1 done        t = ${leg1.simTimeS} s, ${leg1.frames.length} frames`);

    // ---- snapshot over REST, through a file, and back --------------------------
    // Exact: the server's serde_json carries the `float_roundtrip` feature, without
    // which a parsed float can come back one ULP off the one that was written and a
    // resumed run drifts silently from the one it continues.
    const snapshotText = await request(args.base, "GET", `/sessions/${id}/snapshot`);
    await writeFile(args.snapshotOut, snapshotText);
    const restored = await requestJson(args.base, "POST", `/sessions/${id}/snapshot`, {
      body: await readFile(args.snapshotOut, "utf8"),
      contentType: "application/json",
    });
    console.log(
      `snapshot          ${(snapshotText.length / 1024).toFixed(1)} KiB -> ${args.snapshotOut}, ` +
        `restored at t = ${restored.pack.sim_time_s} s`,
    );
    // A restore replaces the pack in place, so this same socket keeps stepping the
    // session it was already attached to — and a restored session has no telemetry
    // until something asks for it, which is what the zero-length step is for.
    //
    // Printed rather than written to the CSV: it shares its timestamp with the last
    // frame of leg 1 and is taken at `Rest`, so a plot would draw a spike where the
    // pack merely stopped being asked for current. The size of that step *is* the
    // I·R drop the load was costing — 2.9 A through the pack's resistance.
    const resumed = await readWithoutAdvancing(session, limits);
    console.log(
      `resumed           ${resumed.telemetry.v_terminal.toFixed(3)} V at rest, against ` +
        `${leg1.frames.at(-1).telemetry.v_terminal.toFixed(3)} V under 2.9 A a moment earlier`,
    );

    // ---- leg 2: the short is drawing -------------------------------------------
    const leg2 = await step(session, limits, { nSteps: LEG_2_STEPS, demand: DEMAND });
    rows.push(...leg2.frames.map((frame) => ({ frame, leg: 2 })));
    console.log(`leg 2 done        t = ${leg2.simTimeS} s, ${leg2.frames.length} frames`);

    // ---- leg 3: same fault, cold room ------------------------------------------
    // `SetEnv` replaces the session's *standing* environment, so every later batch
    // that omits its own `env` uses it. (A `Step` may carry an `env` instead, which
    // overrides for that batch only and does not persist.)
    session.send({ SetEnv: { env: { t_ambient: COLD_ROOM_K, t_coolant: null } } });
    await session.expect("EnvSet");
    console.log(`ambient           -> ${degC(COLD_ROOM_K)} degC`);

    const leg3 = await step(session, limits, { nSteps: LEG_3_STEPS, demand: DEMAND });
    rows.push(...leg3.frames.map((frame) => ({ frame, leg: 3 })));
    console.log(`leg 3 done        t = ${leg3.simTimeS} s, ${leg3.frames.length} frames`);

    // ---- what the ground truth says, which the socket alone cannot tell you ------
    // Telemetry is a pack-level summary. The per-cell view is ground truth — every
    // cell's real state, not what the BMS can sense — and it is a REST call because
    // it is a snapshot of a moment rather than a stream.
    const cells = await requestJson(args.base, "GET", `/sessions/${id}/cells`);
    const socs = cells.cells.map((c) => c.soc);
    console.log(
      `\ncell SOC spread   ${pct(Math.min(...socs))} % .. ${pct(Math.max(...socs))} % ` +
        `across ${cells.cells.length} cells`,
    );
  } finally {
    session?.close();
    if (!args.keep) {
      await request(args.base, "DELETE", `/sessions/${id}`);
    } else {
      console.log(`\nsession ${id} left alive: curl ${args.base}/sessions/${id}`);
    }
  }

  // ---- the summary -----------------------------------------------------------
  await writeFile(args.out, toCsv(rows));

  const last = rows.at(-1).frame.telemetry;
  const flagsSeen = new Set(rows.flatMap(({ frame }) => parseFlags(frame.telemetry.flags)));
  const shorted = rows.filter(({ frame }) => frame.telemetry.i_internal_short_a > 0);

  console.log(`
--- after ${rows.at(-1).frame.sim_time_s} s of simulation -------------------------------
  terminal voltage   ${last.v_terminal.toFixed(3)} V
  cell voltages      ${last.v_cell_min.toFixed(4)} .. ${last.v_cell_max.toFixed(4)} V  ` +
    `(spread ${((last.v_cell_max - last.v_cell_min) * 1000).toFixed(1)} mV)
  SOC, ground truth  ${pct(last.soc_true)} %
  SOC, BMS estimate  ${pct(last.soc_bms)} %   <- the gap is the lesson
  temperature        ${degC(last.t_min)} .. ${degC(last.t_max)} degC
  internal short     ${last.i_internal_short_a.toFixed(3)} A, first *reported* at ` +
    `t = ${shorted.length ? shorted[0].frame.sim_time_s : "never"} s
                     (it fires at 600 s; every ${REPORT_EVERY}th step is reported, so the
                      sample lands up to ${REPORT_EVERY * DT_S} s late — decimation costs
                      resolution, never accuracy)
  flags raised       ${flagsSeen.size ? [...flagsSeen].join(", ") : "none, the whole way"}

  ${rows.length} rows -> ${args.out}
`);

  if (flagsSeen.size === 0) {
    console.log(
      `The BMS never raised a flag. It was reading a sensor that had been offset by\n` +
        `exactly enough to hide the fault — which is the scenario's whole point, and\n` +
        `why "no flags" is not the same as "no problem".\n`,
    );
  }
}

main().catch((error) => fail(error.message));
