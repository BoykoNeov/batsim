# Do the browser and the server compute the same trajectory? — H5, measured

> **Status: landed 2026-10-06.** Measured first: the native and wasm builds parted in the
> last bit on 33 of 54 runs — every field within about 10⁻¹⁴ of its value, every discrete
> field (RNG state, counters, queues, flags) identical, all 184 flag transitions on the same
> frame. The owner chose "the reuse first, then `libm`": each ECM cell now takes the pack's
> RC decay instead of recomputing it (13–15 % off the step, bit for bit), and every
> transcendental in `sim-core` goes through `crate::math` on the `libm` crate. The native
> build moved onto the wasm build's bits; the browser did not move; no test moved; no
> snapshot bump. `node tools/wasm-parity/parity.mjs` checks it: 108 of 108. ROADMAP H5.

## What was believed

`CLAUDE.md` promises same-binary determinism and declines to promise bit-exactness across
platforms, because `exp`, `ln` and `powf` come from the platform's maths library. ROADMAP
H5 added that nobody had measured what that means for the two binaries this project
actually ships twice: the browser's wasm build and the native server (and the native Godot
adapter). Its approach — route every transcendental through the `libm` crate, "which is
what wasm32 already compiles against" — rested on an assumption it did not check, and it
predicted that **"the goldens move by ULPs and need re-pinning"**.

## How it was measured

All out of tree under `W:\temp\claude\h5\`; nothing in the repository changed to take
these readings.

1. **Function by function.** One small crate, built natively (MSVC) and with
   `wasm-pack --target nodejs`, evaluated every transcendental the engine calls on
   200 000 inputs each, over a broad range and over the ranges the engine actually feeds
   it (RC decay arguments, Arrhenius exponents, electrolyte concentrations, the
   Box-Muller unit draw, Butler-Volmer arguments, depth-of-discharge powers). Inputs were
   drawn identically on both sides. Three results per input: native `std`, native `libm`
   crate (0.2.16), wasm `std`.
2. **Whole trajectories.** One demand script (JSON), replayed by both sides through
   `sim_wasm::engine::SimEngine` — natively through the rlib, and under Node through the
   same `Sim` the page uses, built fresh from HEAD. 54 runs: every shipped scenario under
   a 1 C discharge, a rest and a C/2 charge, every non-`Dfn` scenario under a power draw
   and a voltage hold, and a year of hour-long steps plus a discharge on
   `calendar_fade_hot`. One output line per call (the frames, or the snapshot JSON), so a
   line diff finds the first call where the two part. `float_roundtrip` makes the JSON
   comparison bit-exact.
3. **Controls.** Native against itself, wasm against itself, and snapshot → restore →
   continue on the same side.
4. **The handoff.** Snapshot after the first call on one side, restore on the other,
   continue, and compare against the side that took the snapshot.

## What was measured

### The functions

Native `std` and wasm `std` disagree on **up to 21 % of inputs** (1–21 % in the engine's own ranges; 0.01 % for `ln` over the whole positive range) for `exp`, `ln`, `powf`,
`sinh`, `cosh`, `asinh`, `exp_m1`, `sin` and `cos`, always by 1 or 2 ULP (`cosh`, `sinh`
and `exp_m1` reach 2). `sqrt` and `powi` agree everywhere, as they must: `sqrt` is
correctly rounded on both, and `powi` lowers to multiplies.

**The native `libm` crate agrees with wasm `std` on every input of every case.** So the
roadmap's assumption holds — at `libm` 0.2.16 and this toolchain (rustc 1.98.0); wasm's
`std` takes its maths from a copy vendored into `compiler-builtins`, and that is a version
relationship, not a guarantee. A test that pins it is part of any build (below).

### The trajectories, as shipped

| comparison | runs bit-identical |
| --- | --- |
| native vs native (control) | 54 / 54 |
| wasm vs wasm (control) | 54 / 54 |
| snapshot → restore → continue, same side | 54 / 54 each |
| **native vs wasm** | **21 / 54** |
| wasm snapshot continued natively, vs wasm continuing | 28 / 54 |

Where they part, they part early — the first differing frame is at 2 s on the `Dfn` and
within a few hundred seconds on most others; the year-long fast-forward agrees for its
first 33 million seconds — and they stay close. A walk over **every leaf** of every frame
and every snapshot (`deep_compare.py`, out of tree) found:

* **Every non-float field identical** on all 54 runs: the RNG state, the counters, the
  fault queue, the BMS's discrete state, and the flags. No run took a different number of
  RNG draws.
* **Every float within about 10⁻¹⁴ of its own value**, with the largest relative gaps on
  state measured in big units — `Dfn` concentrations (2.3 × 10⁻¹⁰ mol/m³ of ~5 × 10⁴),
  SOC (2 × 10⁻¹⁵), terminal voltage (9 × 10⁻¹⁴ V, on the `Spm` power draw), temperature
  (6 × 10⁻¹⁴ K). The fields that read as large *relative* gaps are quantities sitting at
  zero, where the gap is absolute noise: a voltage hold's settled current (2.9 × 10⁻¹² A),
  the heat at the instant the current crosses zero (6.5 × 10⁻¹² W), a rested RC
  overpotential (3.5 × 10⁻¹⁸ V), the `Dfn`'s potentials near their reference.
* **The flags were exercised, and agree.** 40 of the 54 runs cross a threshold —
  184 flag transitions in all, including under-voltage, over-current and
  over-temperature trips, the contactor opening, plating risk, balancing, both SOC clamps
  and the operating window — and every transition lands on the same reported frame on
  both sides. Not exercised: an over-voltage trip, venting, runaway.

The two builds agree to about fourteen significant figures and then go their own ways in
the last bit or two; nothing in this sample amplifies the difference.

Twenty-one runs agree completely because the inputs they happen to feed the functions
land where the two libraries round alike (79–99 % of inputs, depending on the function) — the bare LFP and NMC curves
among them, which is why the guided path has never shown a difference.

What the sample cannot rule out: a threshold — a protection trip, a cut-off, the client's
CC-CV switch, a seeded-RNG draw compared against a probability — that a trajectory crosses
within 10⁻¹⁴ of its edge. In 54 runs that never happened. It is possible, it is rare, and
when it happens the two stories diverge by a whole step.

### The trajectories, with every transcendental routed through `libm`

A scratch copy put a `math` module in `sim-core` — a trait giving `f64` nine `p_*` methods,
each calling `libm` — and changed the 24 calls (on 21 lines) in `aging`, `dfn`, `ecm`, `noise`,
`plating`, `runaway` and `spm` to use it. `sqrt` and `powi` were left alone.

| comparison | runs bit-identical |
| --- | --- |
| **native vs wasm** | **54 / 54** |
| wasm (routed) vs wasm (as shipped) | 54 / 54 — the browser does not move at all |
| wasm snapshot continued natively, vs wasm continuing | 54 / 54 |
| native snapshot continued in wasm, vs native continuing | 54 / 54 |
| native (routed) vs native (as shipped) | 21 / 54 — native moves, by the ULPs above |

### What it moves in the test suite

**Nothing.** `cargo test --workspace --no-fail-fast` on the routed copy: 725 passed, 2
ignored, 1 failed — and the failure (`sim-server` `rest`,
`a_scenario_can_inline_its_chemistry_and_survive_the_round_trip`) also fails on an
untouched copy of HEAD checked out the same way: it compares chemistry text with a
checked-out file that a fresh worktree gets with CRLF line endings. It is about the
checkout, not the maths.

The roadmap's prediction that the goldens would need re-pinning was **wrong**. Every
golden carries a tolerance, and every bit-exact test compares the engine with itself
(replay, snapshot round-trip, hint-versus-search), so a uniform change of library moves
none of them.

### What it costs

The step-loop driver from `ocv-segment-hint.md` (100S10P, pinned to the quietest physical
core, three arms alternating, six rounds of four seconds) on a box held at 100 % by other
work: the quietest core's two logical processors were both saturated. That is a state the
instrument was never validated in — it reproduced to ~1 % at 25–40 % load, and a ratio read
on a loaded box was once wrong by 3× (`ocv-segment-hint.md`). The absolute figures are
inflated about 1.8×; the ratios below are this reading, not a settled number. The
third arm, **null**, is the routed copy with every `p_*` method calling `std` instead:
the same code shape, so it separates the library's cost from layout. Its fingerprint is
bit-identical to HEAD's.

| case | HEAD | null | `libm` | `libm` / HEAD |
| --- | --- | --- | --- | --- |
| `current` | 67.97 µs | 67.53 µs | 71.65 µs | 1.054 |
| `full` | 77.32 µs | 77.60 µs | 82.14 µs | 1.062 |

What this reading does establish is *where* a cost comes from: the null arm sits on HEAD,
so whatever the gap is, it is the library and not the layout. What it does not establish
is how much headroom is left under the 50 µs budget — **a quiet-box reading is owed before
that question is weighed**, and no projection is made from this one. The `Dfn`
(`cc_discharge_3c_dfn`, five alternating runs per arm through the trajectory harness)
read 1.24 s against 1.29 s median, about 4 %, inside this box's noise. The `Spm` runs
were too short to time. The browser pays nothing; it was already on this library.

**Where the cost is, and how to take it back.** The ECM pack computes the RC decay
`exp(−dt/τ)` once per step for the split (`ecm::rc_decays`), and then each cell computed
it again in `rc_update` — 1000 exponentials a step at 100S10P, and `libm`'s `exp` is a few
nanoseconds slower than the MSVC runtime's. The two compute `τ` as the same product in the
same order, so handing the pack's decay to the cell is bit-identical whichever library is
in use. **Built the same day** — see the next section.

## The RC-decay reuse — landed 2026-10-06

The owner chose "the reuse first, then `libm`". `CellModel::advance` and
`ecm::advance_cell` take the pack's decays (`&[f64; MAX_RC_PAIRS]`), memoised in the
advance loop on `soh_resistance` exactly as the split's are; `ecm::rc_update_decayed`
applies one, and shares its blend with `rc_update` so the two cannot drift. Its branch is
`rc_update`'s own test rather than a reliance on the slot's `1.0`, because `v + r·i·0` is
not `v` when `v` is `−0.0` or `r·i` is not finite. A `debug_assert` checks every supplied
decay against a fresh `exp` to the bit.

**Bit-identical:** all 54 trajectory runs of the harness above, native before against
native after; the step-loop fingerprints of `current` and `full`; the workspace suite,
726 passed, 0 failed.

**Perturbation** — the memo fed `soh_resistance = 1.0` instead of the cell's (the way
this goes wrong: an aged pack whose cells' RC pairs stop growing):

| build | what reddens |
| --- | --- |
| debug | 30 `sim-core` tests, all through the new `debug_assert` ("RC decay supplied for a different tau or dt") |
| release | 1: `aging_grows_the_rc_resistance_of_an_ecm_cell`, on its values |

**What it bought.** Four arms on the step loop, twelve rounds, alternating, on a box at
45–72 % load with the pinned core shared (load 125 on the pair) — rounds swung by up to
1.7×, so the per-arm **minimum** is quoted, the reading load can only add to:

| case | HEAD | reuse | `libm` | reuse + `libm` |
| --- | --- | --- | --- | --- |
| `current` | 35.55 µs | 30.19 µs | 36.53 µs | 30.03 µs |
| `full` | 41.86 µs | 36.39 µs | 42.41 µs | 37.45 µs |

The reuse takes 13–15 % off the step; `libm` on top of it gives back 0–3 %; the pair
together is 10–16 % under HEAD. Fingerprints: reuse = HEAD, reuse + `libm` = `libm`. A
quiet-box reading of the absolute figures is still owed, but the sign of the decision no
longer depends on it: with the reuse in, `libm` costs less than the reuse returned.

## What was built — `libm`, landed 2026-10-06

1. **`libm` as the fourth runtime dependency of `sim-core`**, through the workspace table
   and pinned exactly (`=0.2.16`): it is `no_std` with no dependencies of its own, and a
   version bump is a numerical change. `CLAUDE.md`'s dependency line says so.
2. **`src/math.rs`**: nine free functions (`exp`, `exp_m1`, `ln`, `powf`, `sinh`, `cosh`,
   `asinh`, `sin`, `cos`), and the 24 calls rewritten to them — including the
   `debug_assert` in `rc_update_decayed`, which would otherwise compare a `libm` decay
   against a `std` one. `sqrt` and `powi` stay on `f64` and the module says why.
3. **A lint**: `crates/sim-core/clippy.toml` lists the nine methods, and the nine more that
   would bypass the module if anyone reached for them (`tanh`, `log10`, …), under
   `disallowed-methods`. It is confirmed to fire on a primitive method in this clippy:
   on its first run it stopped on the `std` calls in the crate's own tests. Those tests compute their
   expected values with `std` on purpose — an independent reference, compared within a
   tolerance — so the eleven files allow the lint at the top, with that reason.
4. **No pin test for "`libm` equals wasm's `std`".** The plan above wanted one; it guards a
   link the build removes. With the engine calling `libm` explicitly, both targets compile
   the same `libm` source, and what wasm's own `std` vendors no longer enters.
5. **A parity tool, not a `cargo test`**: `cargo test` cannot run the wasm build, and
   committed trajectory hashes would add no parity coverage while every physics slice
   re-pinned them. `crates/sim-wasm/examples/wasm_parity.rs` runs the 54 runs natively
   (frames thinned to about fifty a call; the snapshot after every phase carries the whole
   state), and `tools/wasm-parity/parity.mjs` replays them through the built `web/pkg` and
   compares every result string, then restores each run's first native snapshot into a
   fresh wasm engine and compares the rest. It builds the example itself; one command.
6. `CLAUDE.md`'s determinism rules and the README's determinism section now say what was
   measured — native Windows and wasm, bit for bit, snapshot handoff included; the native
   Godot build is the same route — and that other targets are expected and unmeasured.

**Results.** Workspace suite: 726 passed, 0 failed, nothing re-pinned. The Godot exit gate
(`sim-godot` `godot_gate`, `--ignored`, Godot 4.7 on this machine), which drives the native
engine through the `BatteryPack` node outside `cargo test`: 2 passed — it compares the node
with the in-process engine of the same build, so it pins no number a library could move. `web/pkg` rebuilt
(`WASM_API_VERSION` 8 and `SNAPSHOT_VERSION` 23, both unchanged). Parity: **108 / 108**
against the rebuilt package — and 108 / 108 against the package built the day before,
which is the measurement's own claim seen from the other side: the browser's bits never
moved.

**Perturbation:**

| break | what catches it |
| --- | --- |
| `runaway.rs`: `math::exp(exponent)` put back as `exponent.exp()` | clippy: "use of a disallowed method `f64::exp`" (and the now-unused import) |
| `dfn.rs`: one `math::sinh` put back as `.sinh()` (bypassing the lint, as an `allow` would) | `parity.mjs`: 106 / 108, exit code 1 — `cc_discharge_3c_dfn__cycle` from its first call, and its handoff; the only run that reaches that site |
| (clean tree) | `parity.mjs`: 108 / 108, exit code 0 |

The exit codes were read from `node` run directly, not through `start /wait`, which hides
them.

**Cost, as shipped.** The step-loop driver built at 5c8bc6a fingerprints identically to
the reuse + `libm` prototype (`current` `ca3722d6189b1860`, `full` `2e9f8086c5f49d79` over
three steps), so the free-function routing computes what the timed trait did. Twelve
alternating rounds against the engine before this note, on a box at 100 % (the pinned
pair at 199) — yet tight this time, medians within 3 % of minimums:

| case | before (min) | 5c8bc6a (min) | ratio of minimums | median of paired ratios |
| --- | --- | --- | --- | --- |
| `current` | 35.58 µs | 30.06 µs | 0.845 | 0.843 |
| `full` | 41.23 µs | 35.79 µs | 0.868 | 0.859 |

## Still open

* A Linux or macOS native build was not measured. The transcendentals no longer depend on
  the platform, so what is left to differ is the compiler's code generation; expected to
  agree, not claimed.
* The out-of-tree trajectory instrument (`ANCHORS.md`) could now come in, as a parity
  check like this one rather than a committed baseline.
* A quiet-box reading of the step's absolute cost is still owed (ROADMAP H9).
