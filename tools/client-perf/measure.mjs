// What the browser client costs to keep on screen — measured, not estimated.
//
// Drives the page in a headless Chrome over the DevTools protocol and reports, for three
// states of the page, what share of the main thread was busy and which functions held it:
//
//   paused       a full history (the page's MAX_SAMPLES cap) and nothing moving
//   running_1x   the ordinary pedagogy case, one simulated second per wall second
//   running_max  the fast-forward case, 10 000x
//
// plus the cost of one full `draw()` and how many draws a second the paused page takes.
// `docs/plans/client-redraw.md` records what this found and why the numbers are read from
// a CPU profile rather than from `Performance.getMetrics` (its ScriptDuration did not track
// what the profile showed).
//
// Needs, in this order:
//   1. a server:   cargo run --release -p sim-server            (http://127.0.0.1:8080)
//   2. a Chrome:   chrome.exe --headless --remote-debugging-port=9333 ^
//                    --user-data-dir=<some scratch dir> --no-first-run about:blank
//      Launch it yourself and record its PID: spawning it from Node under Git Bash on this
//      machine exited silently, and a name-matched kill is never the way to stop it.
//   3. node tools/client-perf/measure.mjs <label> [pageUrl] [port]
//
// The page cooperates through `window.batsim` (see the foot of web/app.js); nothing else
// on the page is touched. Output is one JSON object on stdout; set RESULTS_FILE to a path
// outside the repository to have it appended there too.
import { appendFileSync } from "node:fs";

const [label = "unlabelled", url = "http://127.0.0.1:8080/app/", portArg = "9333"] = process.argv.slice(2);
const PORT = Number(portArg);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    ws.onmessage = (ev) => {
      const m = JSON.parse(ev.data);
      if (m.id === undefined) return;
      const p = this.pending.get(m.id);
      this.pending.delete(m.id);
      if (m.error) p.reject(new Error(JSON.stringify(m.error)));
      else p.resolve(m.result);
    };
  }
  static async connect(wsUrl) {
    const ws = new WebSocket(wsUrl);
    await new Promise((res, rej) => {
      ws.onopen = res;
      ws.onerror = rej;
    });
    return new Cdp(ws);
  }
  send(method, params = {}) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }
  async eval(expr) {
    const r = await this.send("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(`page threw: ${JSON.stringify(r.exceptionDetails).slice(0, 500)}`);
    return r.result.value;
  }
}

/** Share of a window the main thread spent in anything but `(idle)`, and the top self-time frames. */
async function busy(cdp, ms) {
  await cdp.send("Profiler.enable");
  await cdp.send("Profiler.setSamplingInterval", { interval: 200 });
  await cdp.send("Profiler.start");
  await sleep(ms);
  const { profile } = await cdp.send("Profiler.stop");
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const counts = new Map();
  for (const s of profile.samples) counts.set(s, (counts.get(s) ?? 0) + 1);
  const self = new Map();
  let idle = 0;
  for (const [nid, c] of counts) {
    const name = byId.get(nid).callFrame.functionName || "(anon)";
    if (name === "(idle)") idle += c;
    else self.set(name, (self.get(name) ?? 0) + c);
  }
  const total = profile.samples.length;
  const top = [...self.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 4)
    .map(([k, c]) => `${k} ${((100 * c) / total).toFixed(1)}%`);
  return { busy_pct: +((100 * (total - idle)) / total).toFixed(1), top };
}

async function waitFor(cdp, expr, timeoutMs, what) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    if (await cdp.eval(expr)) return;
    await sleep(200);
  }
  throw new Error(`timeout waiting for ${what}`);
}

const setSpeedAndRun = (exp) =>
  `(() => { const $ = (i) => document.getElementById(i); $("speed").value = "${exp}"; $("speed").oninput(); $("run").click(); return true; })()`;
const pause = `document.getElementById("run").click()`;

const version = await (await fetch(`http://127.0.0.1:${PORT}/json/version`)).json();
const summary = { label, at: new Date().toISOString(), chrome: version.Browser, url };
const target = await (await fetch(`http://127.0.0.1:${PORT}/json/new?${url}`, { method: "PUT" })).json();
try {
  // A target that is not the foreground one gets no animation frames, and the page's
  // whole loop hangs off requestAnimationFrame.
  await fetch(`http://127.0.0.1:${PORT}/json/activate/${target.id}`);
  const cdp = await Cdp.connect(target.webSocketDebuggerUrl);
  await cdp.send("Runtime.enable");
  await waitFor(cdp, "!!(window.batsim && window.batsim.state.backend && window.batsim.history.t.length > 0)", 30000, "page boot");
  summary.versions = await cdp.eval("document.getElementById('versions').textContent");

  // Fill the history to its cap at 10 000x. The cap trims the oldest tenth, so "full"
  // is nine tenths of MAX_SAMPLES or more.
  await cdp.eval(setSpeedAndRun(4));
  const tFill = Date.now();
  await waitFor(cdp, "window.batsim.history.t.length >= 175000", 120000, "a full history");
  summary.fill_wall_s = (Date.now() - tFill) / 1000;
  await cdp.eval(pause);
  await sleep(600);
  summary.samples = await cdp.eval("window.batsim.history.t.length");
  summary.sim_time_s = await cdp.eval("window.batsim.state.facts.sim_time_s");

  summary.paused = await busy(cdp, 5000);
  summary.paused.draws_per_s = await cdp.eval(`new Promise((res) => { let n = 0, last = null; const t0 = performance.now();
    const tick = () => { const v = window.batsim.view ? window.batsim.view.drawnAtMs : performance.now(); if (v !== last) { n++; last = v; }
      if (performance.now() - t0 < 2000) requestAnimationFrame(tick); else res(n / 2); }; tick(); })`);
  summary.draw_ms = await cdp.eval(`(() => { const b = window.batsim; const N = 20; const t0 = performance.now();
    for (let i = 0; i < N; i++) { if (b.invalidate) b.invalidate(); b.draw(); } return (performance.now() - t0) / N; })()`);

  await cdp.eval(setSpeedAndRun(0));
  await sleep(1500);
  summary.running_1x = await busy(cdp, 5000);
  await cdp.eval(pause);

  await cdp.eval(setSpeedAndRun(4));
  await sleep(1500);
  const s0 = await cdp.eval("window.batsim.state.facts.sim_time_s");
  summary.running_max = await busy(cdp, 5000);
  const s1 = await cdp.eval("window.batsim.state.facts.sim_time_s");
  summary.running_max.sim_s_per_wall_s = (s1 - s0) / 5;
  await cdp.eval(pause);
  summary.banner = await cdp.eval("document.getElementById('banner').textContent");
} finally {
  await fetch(`http://127.0.0.1:${PORT}/json/close/${target.id}`).catch(() => {});
}
console.log(JSON.stringify(summary, null, 2));
if (process.env.RESULTS_FILE) appendFileSync(process.env.RESULTS_FILE, `${JSON.stringify(summary)}\n`);
