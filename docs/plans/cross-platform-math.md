# Do the browser and the server compute the same trajectory? — H5, measured

> **Status: measured 2026-10-06; nothing built.** They do not, by the last bit, on 33 of
> 54 runs — but every field of every frame and snapshot agrees to about 10⁻¹⁴ of its
> value, every discrete field (RNG state, counters, queues, flags) agrees exactly, and all
> 184 flag transitions land on the same frame. Routing the engine's nine transcendental
> functions through the pure-Rust `libm` crate makes them agree on all 54, makes a
> snapshot taken in either one continue bit for bit in the other, and moves **no** test.
> Its cost to the native step read 5–6 % on a fully loaded box; a quiet-box reading is
> owed before anyone weighs it against the budget. Whether to
> take it is the owner's decision: it adds a dependency to `sim-core`, whose list
> `CLAUDE.md` closes, and it changes a sentence of the design contract. ROADMAP H5.

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
`exp(−dt/τ)` once per step for the split (`ecm::rc_decays`), and then each cell computes
it again in `rc_update` — 1000 exponentials a step at 100S10P, and `libm`'s `exp` is a few
nanoseconds slower than the MSVC runtime's. The two compute `τ` as the same product in the
same order, so handing the pack's decay to the cell is bit-identical whichever library is
in use. That is a separate perf slice; it would probably return most of the library's cost.

## What a build would be

If the owner takes it:

1. `libm` as a fourth runtime dependency of `sim-core` (it is `no_std`, has no
   dependencies of its own, and is what the wasm target already runs), the `CLAUDE.md`
   dependency line amended to say so.
2. A `math` module and the 24 calls, as in the scratch copy. Free functions or the
   trait — the trait keeps each call site's shape.
3. A lint so a bare `.exp()` cannot creep back: `clippy::disallowed_methods` in
   `clippy.toml` naming the nine `f64` methods for `sim-core`.
4. **A test that pins the assumption**: the function sweep above, as a wasm-vs-native check
   that runs when someone rebuilds `web/pkg`, or at least a committed table of
   `libm` outputs on a few hundred inputs that the native test compares against — if a
   future toolchain's vendored copy drifts from the crate, this is what notices.
5. A trajectory test: a handful of the 54 runs' final snapshots committed as hashes, so
   the trajectory instrument the Phase 6 note declined can come in.
6. `CLAUDE.md`'s determinism section rewritten carefully: bit-exact between the native
   Windows build and the wasm build, *measured*; still not promised against a Linux
   server's glibc or any other target, which this did not measure — though with `libm`
   doing all the transcendentals, the remaining differences would be the compiler's, not
   the library's.

Not needed: a snapshot bump (the layout does not change), re-pinned goldens (none moved).

## Still open

* The owner's decision on the dependency and the contract sentence.
* The RC-decay reuse above, worth doing either way.
* A Linux native build was not measured.
