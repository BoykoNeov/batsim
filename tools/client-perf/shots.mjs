// Screenshots of the browser client's plots, for looking at what `measure.mjs` counts.
//
// Loads the page, optionally picks a scenario and a demand mode, runs it at a chosen
// speed for a while, pauses, and writes three PNGs: the six plots, the same six with the
// cursor placed on the first panel, and the whole page. The pulse-train case is the one
// that matters — see `docs/plans/client-redraw.md` for the two images this produced
// before and after the plots stopped decimating by stride.
//
// Same preconditions as measure.mjs (a server, a headless Chrome on the debugging port).
//
//   SPEED=<slider value 0..4> RUN_MS=<ms> node tools/client-perf/shots.mjs \
//        [pageUrl] [tag] [scenario file] [demand mode] [outDir] [port]
//
//   e.g. SPEED=4 RUN_MS=16000 node tools/client-perf/shots.mjs \
//        http://127.0.0.1:8080/app/ pulse cc_discharge_lgm50.toml Pulse .
import { writeFileSync } from "node:fs";

const [url = "http://127.0.0.1:8080/app/", tag = "shot", scenario = "", mode = "", outDir = ".", portArg = "9333"] =
  process.argv.slice(2);
const PORT = Number(portArg);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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
const ev = async (expr) =>
  (await send("Runtime.evaluate", { expression: expr, returnByValue: true, awaitPromise: true })).result.result?.value;

try {
  await send("Runtime.enable");
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1600, height: 1000, deviceScaleFactor: 1, mobile: false });
  while (!(await ev("!!(window.batsim && window.batsim.state.backend && window.batsim.history.t.length > 0)"))) await sleep(200);
  if (scenario) {
    await ev(`(async () => { const $ = (i) => document.getElementById(i); $("scenario").value = ${JSON.stringify(scenario)}; await $("load").onclick(); return true; })()`);
    await sleep(1500);
  }
  if (mode) {
    await ev(`(() => { const $ = (i) => document.getElementById(i); $("demand-mode").value = ${JSON.stringify(mode)}; $("demand-mode").onchange(); return true; })()`);
  }
  await ev(`(() => { const $ = (i) => document.getElementById(i); $("speed").value = ${JSON.stringify(process.env.SPEED ?? "3")}; $("speed").oninput(); $("run").click(); return true; })()`);
  await sleep(Number(process.env.RUN_MS ?? 6000));
  await ev(`document.getElementById("run").click()`);
  await sleep(400);
  console.log("samples", await ev("window.batsim.history.t.length"), "t", await ev("window.batsim.state.facts.sim_time_s"));

  const clip = async (sel) => {
    const r = JSON.parse(await ev(`JSON.stringify(document.querySelector(${JSON.stringify(sel)}).getBoundingClientRect())`));
    return { x: r.x, y: r.y, width: r.width, height: r.height, scale: 1 };
  };
  const shot = async (name, sel) => {
    const c = await clip(sel);
    const { result } = await send("Page.captureScreenshot", { format: "png", clip: c });
    writeFileSync(`${outDir}/${name}.png`, Buffer.from(result.data, "base64"));
    console.log("wrote", `${outDir}/${name}.png`);
  };
  await ev(`document.querySelector("#plots").scrollIntoView()`);
  await sleep(300);
  await shot(`${tag}-plots`, "#plots");
  // The cursor on the first panel, six tenths of the way along.
  const r = JSON.parse(await ev(`JSON.stringify(document.getElementById("plot-v").getBoundingClientRect())`));
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: r.x + r.width * 0.6, y: r.y + r.height * 0.5 });
  await sleep(300);
  await shot(`${tag}-hover`, "#plots");
  await shot(`${tag}-page`, "body");
} finally {
  await fetch(`http://127.0.0.1:${PORT}/json/close/${target.id}`).catch(() => {});
  ws.close();
}
