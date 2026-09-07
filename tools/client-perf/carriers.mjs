// Screenshots of the carrier diagram in each state it can draw, for looking at what the
// engine reports and the page says about it. Written for `docs/plans/carrier-diagram.md`.
//
// Each case loads a scenario, sets the controls a lesson would, runs at speed until a
// condition on the page holds (a flag, a simulation time, a cell field), pauses, and
// writes one PNG of `#carriers` plus the panel's own two text lines — so a reviewer can
// read what the diagram *claimed* beside what it drew.
//
// Same preconditions as measure.mjs: a server, a headless Chrome on the debugging port.
//
//   node tools/client-perf/carriers.mjs [pageUrl] [outDir] [port] [only-case-name]
import { writeFileSync } from "node:fs";

const [url = "http://127.0.0.1:8080/app/", outDir = ".", portArg = "9333", only = ""] = process.argv.slice(2);
const PORT = Number(portArg);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/**
 * `run`: a JS predicate on `window.batsim` evaluated in the page; the run pauses when it
 * holds or when `maxWallMs` passes. `set` runs before the run with `$` in scope.
 */
const CASES = [
  {
    name: "lfp-discharge",
    scenario: "cc_discharge_lfp.toml",
    set: `$("demand-mode").value = "Current"; $("demand-value").value = "2";`,
    speed: "2",
    until: "b.state.facts.sim_time_s >= 600",
  },
  {
    name: "nmc-cccv-charge",
    scenario: "cc_cv_charge_nmc.toml",
    set: `$("demand-mode").value = "CcCv"; $("cccv-i").value = "1.5"; $("cccv-v").value = "4.2"; $("cccv-taper").value = "0.15";`,
    speed: "2",
    until: "b.state.facts.sim_time_s >= 900",
  },
  {
    name: "nimh-overcharge",
    scenario: "nimh_overcharge.toml",
    set: `$("demand-mode").value = "Current"; $("demand-value").value = "-3";`,
    speed: "3",
    until: "b.state.latest && b.state.latest.i_rejected_a < -0.5",
    maxWallMs: 40000,
  },
  {
    name: "lfp-past-empty",
    scenario: "over_discharge_damage_lfp.toml",
    set: `$("demand-mode").value = "Current"; $("demand-value").value = "2";`,
    speed: "3",
    until: "b.state.cells && b.state.cells.cells[0].soc_deficit > 0.01",
    maxWallMs: 40000,
  },
  {
    name: "nmc-cold-plating",
    scenario: "cold_charge_nmc.toml",
    set: `$("demand-mode").value = "Current"; $("demand-value").value = "-3"; $("ambient").value = "-20"; $("ambient").oninput && $("ambient").oninput();`,
    speed: "2",
    until: "b.state.latest && b.state.latest.flags.includes('PLATING_RISK')",
    maxWallMs: 40000,
  },
  {
    name: "pack-internal-short",
    scenario: "soft_short_under_a_lying_sensor.toml",
    set: `$("demand-mode").value = "Current"; $("demand-value").value = "6";`,
    speed: "2",
    until: "b.state.cells && b.state.cells.cells[2].internal_short_conductance_s > 0",
    pin: 2,
    maxWallMs: 40000,
  },
  {
    name: "pack-contactor-open",
    scenario: "external_short_30_milliohm.toml",
    set: `$("demand-mode").value = "Rest";`,
    speed: "1",
    until: "b.state.latest && b.state.latest.flags.includes('CONTACTOR_OPEN')",
    maxWallMs: 40000,
  },
  {
    name: "pack-balancing",
    scenario: "cc_cv_charge_pack.toml",
    set: `$("demand-mode").value = "CcCv"; $("cccv-i").value = "3"; $("cccv-v").value = "4.2"; $("cccv-taper").value = "0.3";`,
    speed: "3",
    until: "b.state.latest && b.state.latest.flags.includes('BALANCING')",
    maxWallMs: 60000,
  },
  {
    name: "pba-discharge",
    scenario: "cc_discharge_pba.toml",
    set: `$("demand-mode").value = "Current"; $("demand-value").value = "0.36";`,
    speed: "3",
    until: "b.state.facts.sim_time_s >= 7200",
    maxWallMs: 40000,
  },
  {
    name: "dfn-3c",
    scenario: "cc_discharge_3c_dfn.toml",
    set: `$("demand-mode").value = "Current"; $("demand-value").value = "15.459594"; $("dt").value = "2";`,
    speed: "1.5",
    until: "b.state.facts.sim_time_s >= 400",
    maxWallMs: 60000,
  },
  {
    name: "pack-runaway",
    // The one shipped pack with a thermal network and no BMS. A hard charge at a hot
    // ambient, past full: the refused current is heat, and nothing stops it. The state
    // no shipped scenario reaches on its own, and the hottest cell is the one drawn.
    scenario: "calendar_fade_hot.toml",
    set: `$("demand-mode").value = "Current"; $("demand-value").value = "-20"; $("ambient").value = "60"; $("ambient").oninput && $("ambient").oninput();`,
    // 100x and not 1000x: the driver polls every 250 ms of wall time, and at 1000x a
    // cell goes from onset to vent between two polls.
    speed: "2",
    until: "b.state.cells && b.state.chem && Math.max(...b.state.cells.cells.map((c) => c.temp_k)) >= b.state.chem.t_onset_k",
    pinExpr: "b.state.cells.cells.map((c) => c.temp_k).indexOf(Math.max(...b.state.cells.cells.map((c) => c.temp_k)))",
    maxWallMs: 120000,
  },
  {
    name: "pack-vented",
    scenario: "calendar_fade_hot.toml",
    set: `$("demand-mode").value = "Current"; $("demand-value").value = "-20"; $("ambient").value = "60"; $("ambient").oninput && $("ambient").oninput();`,
    speed: "3",
    until: "b.state.latest && b.state.latest.flags.includes('VENTED')",
    pinExpr: "b.state.cells.cells.findIndex((c) => c.vented)",
    maxWallMs: 150000,
  },
];

const target = await (await fetch(`http://127.0.0.1:${PORT}/json/new?${url}`, { method: "PUT" })).json();
await fetch(`http://127.0.0.1:${PORT}/json/activate/${target.id}`);
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map();
ws.onmessage = (ev) => {
  const m = JSON.parse(ev.data);
  if (m.id) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  }
};
const send = (method, params = {}) =>
  new Promise((res) => {
    const i = ++id;
    pending.set(i, res);
    ws.send(JSON.stringify({ id: i, method, params }));
  });
const ev = async (expr) => {
  const r = await send("Runtime.evaluate", { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.result.exceptionDetails) throw new Error(JSON.stringify(r.result.exceptionDetails.exception?.description ?? r.result.exceptionDetails));
  return r.result.result?.value;
};

const summary = [];
try {
  await send("Runtime.enable");
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1600, height: 1200, deviceScaleFactor: 1, mobile: false });
  while (!(await ev("!!(window.batsim && window.batsim.state.backend && window.batsim.history.t.length > 0)"))) await sleep(200);
  console.log("page up:", await ev("document.getElementById('versions').textContent"));

  for (const c of CASES) {
    if (only && c.name !== only) continue;
    console.log(`== ${c.name}`);
    await ev(`(async () => { const $ = (i) => document.getElementById(i); $("scenario").value = ${JSON.stringify(c.scenario)}; await $("load").onclick(); return true; })()`);
    await sleep(1200);
    // Every case starts from the page's defaults for the two controls a case may move and
    // a later one would otherwise inherit: the ambient slider and the step length.
    await ev(`(() => { const $ = (i) => document.getElementById(i); $("ambient").value = "25"; $("ambient").oninput && $("ambient").oninput(); $("dt").value = "0.5"; ${c.set} $("demand-mode").onchange && $("demand-mode").onchange(); return true; })()`);
    await ev(`(() => { const $ = (i) => document.getElementById(i); $("speed").value = ${JSON.stringify(c.speed)}; $("speed").oninput && $("speed").oninput(); return true; })()`);
    await ev(`document.getElementById("run").click()`);
    const t0 = Date.now();
    let held = false;
    while (Date.now() - t0 < (c.maxWallMs ?? 30000)) {
      await sleep(250);
      if (await ev(`(() => { const b = window.batsim; try { return !!(${c.until}); } catch (e) { return false; } })()`)) {
        held = true;
        break;
      }
      if (!(await ev("window.batsim.state.running"))) break; // the page stopped itself (an error)
    }
    if (await ev("window.batsim.state.running")) await ev(`document.getElementById("run").click()`);
    const pin = c.pinExpr ? await ev(`(() => { const b = window.batsim; return ${c.pinExpr}; })()`) : c.pin;
    if (pin !== undefined && pin >= 0) {
      await ev(`(() => { const tiles = document.querySelectorAll("#pack-grid .celltile"); tiles[${pin}].click(); return true; })()`);
    }
    // Let the cell sampler and one paint go by.
    await sleep(700);
    await ev("window.batsim.draw()");
    const banner = await ev("document.getElementById('banner').textContent");
    const head = await ev("document.getElementById('carriers-cell').textContent");
    const note = await ev("document.getElementById('carriers-note').textContent");
    const flags = await ev("window.batsim.state.latest ? window.batsim.state.latest.flags : ''");
    const t = await ev("window.batsim.state.facts.sim_time_s");
    const drawMs = await ev("window.batsim.view.drawMs");
    // The diagram's own paint, apart from the six plots `draw` also repaints: the median
    // of twenty, so one GC pause does not stand for the panel.
    const diagramMs = await ev(
      "(() => { const t = []; for (let k = 0; k < 20; k += 1) { const a = performance.now(); window.batsim.drawCarriers(); t.push(performance.now() - a); } t.sort((x, y) => x - y); return t[10]; })()",
    );
    await ev(`document.getElementById("carriers").scrollIntoView()`);
    await sleep(200);
    const r = JSON.parse(await ev(`JSON.stringify(document.getElementById("carriers").getBoundingClientRect())`));
    const { result } = await send("Page.captureScreenshot", {
      format: "png",
      clip: { x: r.x, y: r.y, width: r.width, height: r.height, scale: 1 },
    });
    const file = `${outDir}/carriers-${c.name}.png`;
    writeFileSync(file, Buffer.from(result.data, "base64"));
    // The drawn cell's own record, so a reviewer can check the picture against the numbers.
    const cell = await ev(
      "(() => { const b = window.batsim; const g = b.state.cells; if (!g) return null; const i = b.grid ? (b.grid.pinned ?? 0) : 0; return JSON.stringify(g.cells[i]); })()",
    );
    const row = { name: c.name, held, t, flags, head, note, drawMs, diagramMs, banner, file, cell };
    summary.push(row);
    console.log(JSON.stringify(row, null, 1));
  }
} finally {
  writeFileSync(`${outDir}/carriers-summary.json`, JSON.stringify(summary, null, 2));
  await fetch(`http://127.0.0.1:${PORT}/json/close/${target.id}`).catch(() => {});
  ws.close();
}
