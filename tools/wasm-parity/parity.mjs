#!/usr/bin/env node
//
// parity.mjs — does the browser compute the same trajectory as the native engine, bit for
// bit?
//
// `sim-core` computes every transcendental through the `libm` crate, so the native build
// and the wasm build compile the same source for them and must agree exactly
// (`docs/plans/cross-platform-math.md`). `cargo test` never runs the wasm build, so this
// is the check: it runs the native example `wasm_parity`, then replays every one of its
// demands through `web/pkg` — the package the page loads, which is built locally and not
// committed (README, "The browser demo") — and compares every call's result as a string. The JSON is written by the same serializer on
// both sides with `float_roundtrip`, so equal strings are equal bits.
//
// It also checks the handoff: each run's first snapshot, taken natively, is restored into
// a fresh wasm engine, which must continue exactly as the native engine did.
//
// No dependencies. From the repository root:
//
//   node tools/wasm-parity/parity.mjs
//
// It builds the example in release (the first time takes a minute or two). Exit code 0
// when everything agrees; 1, with the first difference of each run that differs, when not.
// Build `web/pkg` first if it is missing. A difference after a Rust change can mean the
// package is stale — rebuild it with the README's `wasm-pack build` command and run this
// again before reading anything into it.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const { initSync, Sim } = await import(new URL("../../web/pkg/sim_wasm.js", import.meta.url));
initSync({ module: readFileSync(join(root, "web", "pkg", "sim_wasm_bg.wasm")) });

const work = mkdtempSync(join(tmpdir(), "wasm-parity-"));
const nativeFile = join(work, "native.json");
try {
  execFileSync(
    "cargo",
    ["run", "--release", "--quiet", "-p", "sim-wasm", "--example", "wasm_parity", "--", nativeFile],
    { cwd: root, stdio: ["ignore", "inherit", "inherit"] },
  );
  const { runs } = JSON.parse(readFileSync(nativeFile, "utf8"));

  const replay = (sim, ops) =>
    ops.map((op) => {
      try {
        return op.op === "snapshot"
          ? sim.snapshot()
          : sim.step_many(op.dt, op.n, JSON.stringify(op.demand), op.every);
      } catch (e) {
        return "ERR: " + (e && e.message ? e.message : String(e));
      }
    });

  // Where two result strings first differ, named as precisely as the JSON allows.
  const firstDifference = (a, b) => {
    let i = 0;
    while (i < a.length && a[i] === b[i]) i++;
    const from = Math.max(0, i - 60);
    return `at character ${i}:\n      native: …${a.slice(from, i + 40)}\n      wasm:   …${b.slice(from, i + 40)}`;
  };

  const compare = (label, native, wasm, opOffset) => {
    for (let k = 0; k < native.length; k++) {
      if (native[k] !== wasm[k]) {
        return `${label}: op ${k + opOffset} differs ${firstDifference(native[k], wasm[k] ?? "")}`;
      }
    }
    return null;
  };

  let failures = 0;
  let handoffs = 0;
  for (const run of runs) {
    const fresh = new Sim(run.scenario_toml, run.chemistry_toml);
    const whole = compare(run.name, run.lines, replay(fresh, run.ops), 0);
    fresh.free();
    if (whole) {
      failures++;
      console.log("DIFFERS  " + whole);
    }
    const k = run.ops.findIndex((op) => op.op === "snapshot");
    if (k >= 0 && k + 1 < run.ops.length) {
      const restored = new Sim(run.scenario_toml, run.chemistry_toml);
      restored.restore(run.lines[k]);
      const rest = compare(
        `${run.name} (native snapshot continued in wasm)`,
        run.lines.slice(k + 1),
        replay(restored, run.ops.slice(k + 1)),
        k + 1,
      );
      restored.free();
      handoffs++;
      if (rest) {
        failures++;
        console.log("DIFFERS  " + rest);
      }
    }
  }
  const checks = runs.length + handoffs;
  console.log(
    `${checks - failures}/${checks} checks bit-identical ` +
      `(${runs.length} runs, ${handoffs} native-snapshot handoffs) against web/pkg`,
  );
  process.exitCode = failures === 0 ? 0 : 1;
} finally {
  rmSync(work, { recursive: true, force: true });
}
