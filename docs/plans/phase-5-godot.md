# Phase 5 — Godot adapter

**Status: in progress. Slices A, B and C have landed; D and E are planned.** This file is written before the work so the
decisions below are made once; the "learned while building" material is appended as each
slice lands, the way `phase-2-thermal-bms.md` through `phase-4-server-wasm.md` grew.

Like Phase 4, Phase 5 adds **no physics**. Everything below is embedding, lifecycle, and
presentation over an engine that is already finished for this phase's purposes. That
framing is load-bearing: see "The `SNAPSHOT_VERSION` canary, again".

Unlike every previous phase, `CLAUDE.md` gives Phase 5 **no `Exit:` line** — it names the
deliverables (`BatteryPack` node, exported chemistry/topology properties, fixed-dt
accumulator in `_physics_process`, signals) and stops. The exit criterion below is
therefore authored here rather than inherited, and the choice is argued rather than
asserted.

| exit criterion (authored here) | to be met by |
| ------------------------------ | ------------ |
| A scenario driven through the `BatteryPack` node inside a running Godot process produces a **bit-identical** trajectory to the same scenario driven by `Pack::step` in process | `sim-godot/tests/godot_gate.rs` — a Rust test that builds the cdylib, runs `godot --headless` over a GDScript driver, and compares `f64::to_bits` of every reported field |

## Slices

| slice | scope | state |
| ----- | ----- | ----- |
| A | the shared boundary rule: `Demand::check_finite` / `Env::check_finite` land in `sim-core`; `sim-server` and `sim-wasm` migrate onto them; the sim-wasm "third client" comment is amended to name a criterion instead of a count | **landed** (v9 — no bump, as designed) |
| B | `crates/sim-godot`: crate skeleton, `godot` pinned to `api-4-7`, workspace membership, and `driver.rs` — the entire pure layer (accumulator, flag edge detector, scenario→pack, batch stepping, caps), host-tested | **landed** (v9 — no bump, as designed) |
| C | the node surface: exported properties, `#[func]` methods, signals, `_physics_process` wiring, `.gdextension`, demo project skeleton | **landed** (v9 — no bump, as designed) |
| D | the exit gate: GDScript driver emitting bit patterns, the Rust integration test that compares them, the `--import` bootstrap. **Carries the exit criterion.** | **planned** |
| E | wrap-up: a watchable demo scene, README status and run instructions, adapter overhead measured separately from `Pack::step` | **planned** |

Each slice keeps `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` clean. As in Phase 4, **no slice
should bump `SNAPSHOT_VERSION`** — a deliberate tripwire, not an observation.

---

## The spike, and what it settled

A throwaway crate (`godot 0.5.4`, `features = ["api-4-7"]`, one `#[derive(GodotClass)]`
node, one pure module, host tests) was built and run against the installed
Godot 4.7 **before this document was written**, because most of the decisions below turn
on facts that cannot be reasoned out. Everything in this section is measured on this box,
not inferred.

| question | answer |
| -------- | ------ |
| Does a gdext crate host-test with no Godot process running? | **Yes.** `cargo test` compiles and runs, provided no test touches a `godot` type. |
| Does the build need a Godot binary? | **No**, with `api-4-7`. The bindings are generated from JSON bundled in `godot-codegen`. Only `api-custom` shells out to a `godot4` binary — do not use it. |
| Is `clippy --all-targets -- -D warnings` clean over gdext's generated code? | **Yes**, with no `allow`s in the spike. |
| What does it cost the gate? | **~1m45s once** to compile `godot-codegen`/`godot-core`/`godot`, then free. Warm re-clippy: 0.05 s. Touch-one-file re-clippy: 0.16 s. |
| Does the extension load headless? | **Yes** — `Initialize godot-rust (API v4.7.stable.official, runtime v4.7.stable.official, safeguards strict)`. |
| Is a one-time bootstrap needed? | **Yes.** On a tree with no `.godot/`, `godot --headless --path . --script x.gd` fails with `Identifier "BatteryPack" not declared`. `godot --headless --path . --import` writes `.godot/extension_list.cfg` and fixes it. |
| Does a *rebuilt* cdylib need a re-import? | **No.** Verified by changing a `#[func]`'s return value, `cargo build`, and re-running the same script — the new value came back. Import is a per-clone bootstrap, not a per-build step. |
| `Engine.physics_ticks_per_second` / `max_physics_steps_per_frame` / `time_scale` | **60 / 8 / 1.0** on 4.7 defaults. All three move `delta`; see the accumulator section. |
| Does `#[signal]` register introspectably? | **Yes** — `ClassDB.class_get_signal_list` returns the signal with typed args. |

Two findings were sharp enough that they change the design, and they get their own
sections: **GDScript cannot print a float without losing bits**, and **a failing
`assert()` hangs a headless run**.

---

## Decisions already made (do not re-derive)

### The `SNAPSHOT_VERSION` canary, again

Phase 4 inverted the phases 2–3 rule: *if a slice needs a bump, an adapter has leaked into
the engine.* It held for all five Phase 4 slices. It carries over unchanged.

Nothing a game engine needs should change what a pack *is*. If a slice reaches for a bump,
stop and re-read the slice — the honest fixes are "put it in the adapter" or "add a
read-only accessor", not "add a field to `Pack`".

Slice A is the one place this needs care, because it *does* touch `sim-core`. It adds two
inherent methods and an error type; it adds no field to any serialized struct. `Demand` and
`Env` are step arguments, not pack state — neither appears in a `Snapshot`. The canary
should therefore hold through slice A too, and if it does not, slice A is wrong.

### The third-client question, and why the answer is four checks in `sim-core`

`crates/sim-wasm/src/engine.rs` says, verbatim:

> **if a third client ever needs these, the fix is to lift `Limits`/`StepCommand`/`Frame`
> into a `sim-protocol` crate**, not to add a third copy. Two clients did not justify the
> churn to slice C's work; three would.

`sim-godot` is that third client, and the commitment has to be met or amended — leaving it
to read as silently violated is the one outcome that is not acceptable. The resolution
here is to **amend it, and to fix the part of it that was actually right**.

The argument for amending: Godot is not the client that comment anticipated.

- **No `StepCommand`.** That type exists because a command arrives as JSON over a socket
  and must be parsed and rejected as a unit. GDScript calls a `#[func]` with typed
  arguments. There is no message, so there is nothing to parse and nothing to deny unknown
  fields on. Its `#[serde(default = "one")]` and `deny_unknown_fields` are *wire-contract
  decisions belonging to the server*; moving them to a shared crate would make one client's
  policy look like a workspace rule.
- **No `Frame`.** `Frame` exists because a batch reply is an array of samples handed to a
  plotting client. The Godot node exposes the current reading as properties and announces
  changes as signals — the idiom the engine's users expect, and the one `CLAUDE.md` names.
  There is no frame array, so there is no frame shape.
- **No decimation, so no `frame_count`.** Decimation exists to keep a reply from becoming a
  million samples. The node reports the *latest* state, once per batch. `k` has no meaning
  where the sample count is always one.
- **The caps do not unify either.** All three clients cap steps per call, but for three
  different reasons: the server bounds how long a session lock is held, `sim-wasm` bounds
  main-thread occupancy in a tab, `sim-godot` bounds a frame budget. Three similar
  constants with three rationales are not one shared constant, and collapsing them would
  put a number in a shared crate that no client could then justify changing.

What *is* genuinely one rule, in all three clients and for one reason: **a `f64` that
reached the boundary without passing through a JSON parser may be non-finite, and the
engine's own types should say so.** `sim-wasm` documented this precisely — over the
server's socket the check is unreachable, because JSON has no literal for `NaN`; across the
wasm boundary JS hands over a raw `f64` and `Number.NaN` arrives intact. GDScript hands
over a raw `f64` too, and `NAN` and `INF` are both spellable in it.

So slice A puts the checks where the rule actually lives:

```rust
// sim-core
impl Demand { pub fn check_finite(self) -> Result<(), NonFinite>; }
impl Env    { pub fn check_finite(self) -> Result<(), NonFinite>; }
```

This is a statement about what the engine's own types accept, not about any protocol, so it
does not violate the purity rule — no I/O, no state, no dependency. `sim-server` maps
`NonFinite` into `ApiError { code: OutOfRange }`, `sim-wasm` into
`EngineError::OutOfRange`, `sim-godot` into its own error. Their *messages* stop being
three hand-copied strings that can drift, which was the only real defect in the status quo.

The sim-wasm comment then gets rewritten to name a **criterion instead of a count**:
lift when a client needs the *shapes*, not when the third client arrives. That is a better
artifact than the original, and it is honest about what happened rather than quietly
deleted.

`Limits`, `StepCommand`, and `Frame` stay exactly where they are, with two consumers each.

### Two entry points, and an honest determinism claim for each

Phase 4's exit gate could assert bit-identical because both legs were driven by explicit
step counts. **The `_physics_process` path cannot make that claim**, and inheriting Phase
4's phrasing would be a lie:

> **True:** same scenario + same seed + same *total step count* + same demand sequence ⇒
> bit-identical trajectory.
> **False:** same scenario, run twice in a real Godot window for the same wall-clock
> duration ⇒ bit-identical trajectory.

The second is false because the number of steps consumed depends on how many frames the
machine delivered and how much time each carried. That is not a defect — it is what
`CLAUDE.md` principle 3 asks for. The frame rate does not define the *timestep*; it defines
only *how many* fixed timesteps get consumed. Each step is still exactly `fixed_dt`.

The consequence for the crate's shape, and it is not cosmetic: **the node needs an explicit
`step_batch(n_steps)` alongside the accumulator, and the exit gate drives the explicit
path.** A gate driven through `_physics_process` would be asserting bit-identity on a
quantity that legitimately varies, and would flake on a loaded machine while looking like a
physics bug. Slice C ships both; slice D uses only the explicit one.

### The accumulator, and the three Godot knobs that move `delta`

`_physics_process(delta)` on Godot 4.7 defaults to a fixed 1/60 s, so it is tempting to
step once per call and skip the accumulator entirely. Three measured facts say no:

- `Engine.physics_ticks_per_second` (default **60**) is settable by the game, so `delta`
  is not a constant this crate may assume.
- `Engine.time_scale` (default **1.0**) scales `delta`. A slow-motion or fast-forward game
  would otherwise silently change the physics timestep — precisely the thing `CLAUDE.md`
  forbids.
- `Engine.max_physics_steps_per_frame` (default **8**) means Godot itself will *drop*
  physics ticks under load rather than let them pile up.

So the node owns a `pending_s` remainder, adds `delta`, consumes `floor(pending_s /
fixed_dt)` whole steps, and carries what is left. `fixed_dt` is an exported property and is
the *only* thing that sets a step's size.

The accumulator needs its own cap — `max_steps_per_frame`, exported — for the same reason
Godot has one: without it, a single long frame (a level load, a breakpoint) hands the node
a huge `delta` and the pack does thousands of steps inside one frame, which stalls the game
and looks like a hang. With it, sim time falls behind wall time under sustained load.

**That must be visible, not silent.** Sim time quietly dilating is indistinguishable from a
physics bug to whoever is looking at the plot. The node emits a `falling_behind` signal
carrying the backlog in seconds when the cap binds, and clamps `pending_s` so the backlog
cannot grow without bound. Whether to *drop* the backlog or *repay* it is a policy the game
should choose; the exported knob and the default are an open question below.

### The exit gate: bit patterns, because GDScript cannot print a float

Measured in the spike, on a value taken from this repo's own `float_roundtrip` finding:

```text
x                    = 0.7995885912375074      (the f64 the engine produced)
str(x)               = 0.79958859123751        (what GDScript prints — 14 sig figs)
float(str(x)) == x   = false                   <-- bits lost
PackedFloat64Array([x]).to_byte_array().hex_encode()
                     = 74d533d03a96e93f
hex_decode().to_float64_array()[0] == x
                     = true                    <-- exact
```

This is the same class of failure as Phase 4's `serde_json` `float_roundtrip` finding, and
it would defeat the gate in the same silent way: a decimal-text handoff makes the
comparison pass on values that differ, or fail on values that do not, with no signal about
which. **The GDScript leg writes `PackedFloat64Array(...).to_byte_array().hex_encode()`;
the Rust leg compares `f64::to_bits`.** No decimal float crosses that boundary in either
direction.

Note the ordering trap this also avoids: `hex_encode` is little-endian byte order of the
IEEE-754 bits, so the Rust side must decode accordingly rather than parse the hex as one
big-endian `u64`. The gate's own round-trip test pins that.

### A failing `assert()` hangs a headless run — so the gate is a Rust test, not a script

The spike ran four failure modes through `godot --headless --script`:

| failure | exit code |
| ------- | --------- |
| `quit(1)` | **1** |
| script parse error | **1** |
| missing script | **1** |
| **failing `assert()`** | **hangs** (killed at 60 s) |
| **runtime script error** (`null.method()`) | **hangs** (killed at 180 s) |

The last two are the dangerous ones: `_initialize` is abandoned mid-function, `quit()` is
never reached, and the headless `SceneTree` runs forever. In an unattended gate that is not
a failure, it is a stall.

Three rules fall out, and slice D is built on them:

1. **No `assert()` in the GDScript leg.** Every path reaches an explicit `quit(code)`.
2. **The gate is a Rust `#[test]`**, which shells out to Godot with a timeout, checks the
   exit code, and does the comparison in Rust where `f64::to_bits` and real assertion
   semantics live. GDScript's job is reduced to *emit numbers on stdout and quit* — the
   least it can be trusted with.
3. **A timeout is mandatory even so**, because rule 1 cannot cover a runtime error in code
   that has not been written yet.

### The gate cannot live in `cargo test --workspace`, and that is the `wasm-pack` shape

The root `Cargo.toml` already carries this exact carve-out, and the plan reuses its
framing rather than inventing new language:

> What is *not* in the gates is `wasm-pack build` — that needs a toolchain the Rust test
> run has no business invoking, and its output is an uncommitted build artifact.

Substitute "a Godot 4.7 binary and a built cdylib" for "wasm-pack" and the sentence is
unchanged. So:

- `crates/sim-godot` **is** a workspace member. Its pure `driver.rs` is compiled and
  host-tested by `cargo test --workspace`, and clippied by the normal gate. The spike
  proves this costs ~1m45s once and nothing thereafter. It cannot rot.
- `tests/godot_gate.rs` is `#[ignore]`d with a reason string naming what it needs. It is
  still *compiled* by the default gate — so it cannot rot either — and runs under
  `cargo test -p sim-godot -- --ignored`. The README and this plan carry that command.

### Signals are edge-triggered, coalesced per batch, and hold their previous state in the adapter

`EventFlags` is a bitmask that is recomputed every step. Emitting a signal whenever a flag
is *set* would emit at 60 Hz for the entire duration of a condition — a signal storm that
makes the feature useless. Rules:

- **Rising edges only.** `protection_tripped` fires on the step where the flag goes
  0→1, not on every step it is 1. A falling-edge signal (`protection_cleared`) is cheap
  once the previous mask is already held, and is worth having for the same reason.
- **Previous flags are adapter state, not engine state.** Putting a `prev_flags` field on
  `Pack` would change the snapshot layout and trip the canary for a purely presentational
  need. It lives on the node. A consequence to accept and document: a snapshot restore
  does not restore the edge detector, so the first step after a restore can re-announce a
  condition that was already active. The alternative is engine pollution; this is the right
  trade and it is written down rather than discovered.
- **Coalesce per batch, never per step.** A `step_batch(10_000)` fast-forward that emitted
  per step would emit thousands of signals into a game's main loop. The batch ORs the
  transitions it saw and emits once at the end — the same reasoning as Phase 4's
  decimation, which the plan there called a throughput requirement rather than an
  optimization. Cost: within one batch, the *ordering* of two different events is lost.
  That is acceptable for a fast-forward and wrong for real-time, and real-time batches are
  a handful of steps, so it is acceptable there too.
- **`soc_changed` needs an exported epsilon.** SOC changes every step, so an unconditioned
  signal is a per-frame signal. It fires when `|soc − last_announced| >= soc_signal_epsilon`,
  default to be chosen in slice C.
- **`EventFlags` crosses as an `i64` bitmask, plus a human-readable `String`.** Not the
  bitflags type — that is not a Godot type and cannot be. Note this deliberately differs
  from the socket, where `EventFlags` crosses as a `" | "`-joined name string; a game
  wants to mask-test cheaply, a browser wanted to print. Both are provided so neither
  client has to parse the other's choice.

### The node takes scenario *text*, not a path

`sim-core` does no file I/O and `sim-data`'s `load_chemistry_file` does. Neither is what a
Godot node should call: `res://` paths are not filesystem paths once a game is exported
into a `.pck`, so a node that took a path would work in the editor and fail in a shipped
build — the worst possible split.

The node takes scenario TOML and chemistry TOML as **strings**, exactly as `sim-wasm` takes
them from JS. GDScript reads them with `FileAccess.get_file_as_string("res://...")`, which
works identically in the editor and in an export. `SimEngine::chemistry_id_of` already
exists for the "which chemistry do I need to load first" step and the same helper is
exposed here.

### Where the demo project lives, and the `res://../target` tension

The Godot project needs `project.godot`, a `.gdextension`, and scripts committed to the
repo. The `.gdextension` must point at the built cdylib, which lives in `/target` — and
`CLAUDE.md` says the repo tree must never hold build artifacts.

There is no conflict, but it deserves one explicit sentence because it looks like one: the
`.gdextension` is **source that references a path inside `/target`**, not an artifact
itself. `/target` is already gitignored. What must be *added* to `.gitignore` is Godot's
own editor cache:

```
# Godot's editor/import cache for the demo project. Regenerated by
# `godot --headless --path godot --import`; see docs/plans/phase-5-godot.md.
/godot/.godot
```

Decided: the project lives at **`godot/`** at the repo root, sibling to `web/` — which is
the precedent Phase 4 set for a client that is not a Rust crate. `.uid` files that Godot
generates beside scripts *are* committed; they are stable identifiers, not cache.

The `.gdextension` lists all six platform/profile paths (win/linux/mac × debug/release)
even though only one is exercised here, because a half-filled table is a trap for whoever
first runs this on another OS.

### Compile-time cost, stated once

Adding `godot` to the workspace adds **~1m45s to a cold build** (`godot-codegen` →
`godot-core` → `godot`), measured on this box. Warm builds are unaffected: 0.05 s for a
no-op clippy, 0.16 s after touching a file in the crate.

That is a real cost and it is paid by everyone who clones the repo, including someone who
only cares about `sim-core`. It is accepted for the same reason `sim-wasm`'s membership was
accepted: a crate outside the gates is a crate that rots, and this one embeds an engine
whose determinism guarantees are the product. The alternative — excluding it — trades a
one-time two minutes for a permanently unverified adapter.

`api-4-7` is **pinned**, not left to the default feature level. Two reasons: the default
tracks whatever gdext considers current, so a `cargo update` could silently change which
Godot versions the built extension will load into; and `compatibility_minimum` in the
`.gdextension` must match what the crate was built against, which is only knowable if it
is pinned.

---

## Slice detail

### A — the shared boundary rule

Touches `sim-core`, `sim-server`, `sim-wasm`; adds no crate and no `godot` dependency.

- `sim-core`: `NonFinite` error (which field, what value), `Demand::check_finite`,
  `Env::check_finite`. Unit tests covering every arm, including `Env::t_coolant: Some(NaN)`,
  which is the arm most likely to be missed.
- `sim-server`: `protocol::check_demand` / `check_env` become thin maps into `ApiError`.
  **The existing `ErrorCode::OutOfRange` wire behaviour must not change** — the message text
  may change, the code may not. `validate_rejects_every_non_finite_field` is the regression
  gate and passes untouched.
- `sim-wasm`: same, into `EngineError::OutOfRange`.
- `sim-wasm/src/engine.rs`'s module comment is rewritten per the section above.

Exit: `cargo test --workspace` passes with no test *modified* — only added. If an existing
assertion had to change, the migration changed behaviour and is wrong.

### B — `sim-godot` and its pure driver

- Crate skeleton, `crate-type = ["cdylib", "rlib"]` (the `rlib` is what makes `driver`
  reachable from `tests/`, exactly as `sim-wasm`'s manifest explains), `godot` pinned to
  `api-4-7`, added to the workspace members list with a comment saying why it is in and
  what is out.
- `driver.rs`: the whole pure layer, **no `godot` type in any signature**, mirroring the
  `SimEngine`/`Sim` split and for the identical reason — the gates run on the host, where a
  `godot` type is either absent or a stub.
  - `Accumulator { pending_s }` with `take(delta, fixed_dt, max) -> u32`.
  - `FlagEdges { prev }` producing rising/falling masks.
  - `PackDriver`: scenario text → pack, `step_batch`, latest telemetry, snapshot/restore
    strings, `schedule_fault`, `restart(bms)`, cells.
  - `MAX_STEPS_PER_CALL` with a comment stating its *Godot-specific* rationale (frame
    budget), not a cross-reference to the other two clients' constants.
- Host tests for every one of the above. The accumulator's remainder-carry and cap, and
  the edge detector's rising/falling/no-change cases, are pure arithmetic and should be
  tested hard here because slice D cannot see inside them.

### C — the node surface

- `#[derive(GodotClass)] BatteryPack`, `base=Node`. The shell forwards; it decides nothing.
- Exported properties: `scenario_toml`, `chemistry_toml`, `fixed_dt`, `max_steps_per_frame`,
  `soc_signal_epsilon`, `auto_step` (whether `_physics_process` runs at all), `bms_enabled`.
- `#[func]`s: `step_batch`, `set_demand`, `read_telemetry`, `snapshot_json`,
  `restore_json`, `schedule_fault_json`, `clear_faults`, `clear_bms_fault`, `restart`,
  `cells_json`, `chemistry_id_of`.
- Signals: `protection_tripped`, `protection_cleared`, `thermal_runaway_started`,
  `vented`, `contactor_opened`, `soc_changed`, `falling_behind`. Payloads are `i64` /
  `float` / `String` only.
- `_physics_process` wiring per the accumulator section, gated on `auto_step`.
- `godot/` project skeleton + `.gdextension` + `.gitignore` entry.
- Exit: a hand-run headless smoke script instantiates the node, sets a scenario, steps, and
  prints telemetry. Not the gate — the gate is slice D — but the first proof the surface
  is callable from GDScript at all.

### D — the exit gate (carries the exit criterion)

- `godot/gate.gd`: builds a pack from a committed scenario, runs a fixed step schedule
  through `step_batch`, prints one hex line per reported field, `quit(0)`. No `assert`.
- `crates/sim-godot/tests/godot_gate.rs`, `#[ignore]`d with a reason naming its two
  requirements: runs `cargo build -p sim-godot`, bootstraps with `--import` if `.godot/` is
  absent, runs `godot --headless --path godot --script gate.gd` under a timeout, parses the
  hex, and compares against an in-process `Pack::step` run field by field on `to_bits`.
- The gate must be **built to fail** before it is trusted — the discipline Phase 4 applied
  to its float test twice. Perturb one field by one ULP in the Rust leg and confirm the
  comparison reports it, naming the field and the step.
- A separate small test pins the hex encoding's byte order, so a silent endianness change
  cannot make the gate compare zeros to zeros.

### E — wrap-up

- A demo scene: the pack running under the accumulator with signals wired to visible
  output, so the phase's deliverable is *watchable* and not only assertable.
- README: status table row, the `--import` bootstrap, the two run commands (demo, gate),
  and the pinned-`api-4-7` note.
- **Adapter overhead measured, and attributed correctly.** Phase 5 adds no physics, so
  `Pack::step` must not move; memory says it sits at ~42–54 µs against a <50 µs budget with
  a box that has missed its fast state four sessions running, so the measurement is
  reported as a **ratio against a same-session baseline**, never as an absolute, and the
  node's per-step overhead is reported as its own number rather than folded into the
  engine's.

---

## Learned while building — slice A (the shared boundary rule)

### The canary held, and this was the slice that could have broken it

Phase 5's canary says a `SNAPSHOT_VERSION` bump means an adapter has leaked into the
engine. Slice A is the only slice in the phase that edits `sim-core` at all, so it was the
one place the rule was under real load rather than being trivially satisfied.

It held: `SNAPSHOT_VERSION` is still 9. What went in was one error struct and two inherent
methods; no serialized struct gained a field. The plan predicted the reason — `Demand` and
`Env` are step *arguments*, and neither appears in a `Snapshot` — and that prediction is
what made the edit safe to make in the engine rather than in three adapters.

### "No existing test was modified" is a stronger gate than "the tests pass"

The slice's exit criterion was deliberately not *`cargo test --workspace` is green*. A
behaviour-preserving refactor can be green while having quietly changed something a test
was updated to accommodate, and the update is invisible in a summary line.

So the gate was a claim about the **diff**, checkable mechanically:

```text
$ git diff --name-only | grep -i test
NONE — no existing test modified
```

The only change under any `tests/` tree is one new file. That proves behaviour was
preserved rather than merely that nothing obviously broke. It is the same instinct as slice
E of Phase 4 — "'nothing changed on the step path' is a claim about a diff, read the diff"
— applied a slice earlier, and it is cheap enough to be worth making a habit.

### The message texts *did* change, and that was checked before it was relied on

Consolidating the checks changes what a user reads:

```text
before (sim-server):  t_ambient must be finite, got NaN
before (sim-wasm):    t_ambient must be finite, got NaN
after  (all three):   env.t_ambient [K] must be finite, got NaN
before (demand):      the demand carries a non-finite value (NaN)
after  (demand):      the demand's current [A] must be finite, got NaN
```

That is only safe if nothing asserts on the text. It was checked rather than assumed —
`sim-server/tests/ws.rs` asserts `error.code() == ErrorCode::OutOfRange`, and
`sim-wasm/tests/engine.rs` asserts `matches!(err, EngineError::OutOfRange(_))`. Both are
assertions about the *taxonomy*, which is the part that is a contract, and neither touches
the string. `ErrorCode::OutOfRange` is a wire contract and did not move.

The new text is also better in a way the repo already had a rule for: `CLAUDE.md` requires
units on every numeric field, and the old messages carried none. `env.t_ambient [K]` and
`the demand's current [A]` do. The old demand message did not even name *which* field was
bad, because the copied version threw the variant away before formatting.

### The `t_coolant` arm was built to fail, and it failed alone

The plan claimed `t_coolant` is the arm a hand-written check misses, "because it is the
only one behind an `Option`". That is an assertion about the test suite as much as about
the code, so it was verified the way Phase 4 verified its float guard — by breaking the
thing on purpose:

```text
$ # Env::check_finite's t_coolant match replaced with `Ok(())`
$ cargo test -p sim-core --test boundary
---- an_env_rejects_a_non_finite_coolant_behind_the_option stdout ----
panicked at crates\sim-core\tests\boundary.rs:95:10:
t_coolant = Some(NaN) was accepted: ()

test result: FAILED. 8 passed; 1 failed
```

**8 passed, 1 failed** is the part worth recording. It says the coverage is not
accidental — no other test in the file incidentally exercises that arm, so if that one test
had not been written, the whole suite would have gone green on a check that silently
accepted `Some(NaN)`. That is precisely the hole the three hand-copied versions were at
risk of, and it is now closed in one place instead of three.

### What is deliberately not here

- **No `sim-protocol` crate.** The argument is in "The third-client question" above and the
  amended comment now lives in `sim-wasm/src/engine.rs`. `Limits`, `StepCommand`, and
  `Frame` were not touched.
- **No `dt` check.** `dt` is not a field of `Demand` or `Env` — it is a separate argument
  to `step`, and its rule is different (finite **and** `>= 0`, where a demand may legitimately
  be negative). Each adapter still owns that one, and correctly so: `sim-godot` will want
  `dt > 0` rather than `>= 0`, because a zero-length step in a `_physics_process` loop is a
  frame that did nothing rather than a deliberate telemetry read.
- **No `godot` dependency anywhere yet.** Slice A adds no crate; that is slice B.

## Learned while building — slice B (`sim-godot` and its pure driver)

### The canary held, and slice B cost `sim-core` nothing at all

`SNAPSHOT_VERSION` is still 9. Slice B added a crate, a `godot` dependency, an
accumulator, an edge detector and a driver, and touched `sim-core` **zero times**. That is
the canary working as designed rather than merely not firing: the one thing the adapter
wanted from the engine that it did not already have — the finiteness rule — was taken in
slice A, deliberately and in isolation.

### The plan predicted a first reading was needed; a real Godot process showed what the alternative looks like

The plan argued that a node's readings are *properties*, that a property has no spelling
for "no reading yet", and that a default `0.0` would be drawn by a plot as a real
measurement of an empty pack. So [`PackDriver::new`] takes one zero-length step.

Driven from GDScript, that is the difference between these two:

```text
primed v_terminal -> 3.6          # LFP OCV at SOC 1.0, straight off the chemistry table
primed sim_time_s -> 0.0          # and the clock has not moved
```

`3.6` is the last entry of `lfp_26650_generic.toml`'s `[ocv] volts` table. It is a
measurement of the pack, at t = 0, before anything ran. Without priming it would have been
`0.0` V — indistinguishable, to a plotting client, from a pack that had been shorted.

### The priming claim was checked rather than argued, and the check is the snapshot

"A zero-length step is free" is exactly the kind of claim that is true until it is not, so
it is a test rather than a sentence. The test compares **snapshots**, not telemetry, and
that choice is the whole point: a snapshot carries the RNG's word position, so a
byte-identical snapshot proves the zero-length step drew nothing. It runs on
`soft_short_under_a_lying_sensor.toml` — a fixture with faults *and* a noisy sensor —
precisely so there is an RNG in play to draw from.

```text
200 steps, preceded by a dt = 0 step   ->  snapshot JSON
200 steps, not preceded by one         ->  snapshot JSON
identical: true
```

If a zero-length step ever stops being free, that test fails and the constructor has to
change rather than the doc comment.

### Slice A's move worked twice, which is the argument that it was the right move

Slice A put the finiteness rule on `Demand`/`Env` because it was a statement about those
types rather than about a protocol. Slice B hit the identical shape a second time and
applied the identical fix: `sim-wasm`'s private `build` — "build this scenario with or
without its BMS, and count the sensor faults that had nowhere to land" — is a statement
about a `Scenario`, so it is now `sim_data::Scenario::build_pack_with_bms` and `sim-wasm`
calls it.

That is the criterion from the amended comment doing real work. Neither move needed a
`sim-protocol` crate, because neither was about a wire.

### A hand-written test fixture tested the wrong failure, and the first run caught it

`a_restore_refuses_a_differently_shaped_pack` needs a 9S3P pack to restore into a 1S1P
node. The first version hand-wrote the `[pack]` table:

```text
a 9S3P pack builds: Data(Toml(TomlError { message: "missing field `seed`", ... }))
```

`PackConfig` requires `seed`, so the fixture never built a pack at all. Had the assertion
been `is_err()` rather than a match on `TopologyMismatch`, this test would have **passed
while proving nothing** — a parse failure wearing a topology check's name.

The fix is a rule worth keeping: derive fixtures from the committed scenario files by
transformation rather than writing new ones by hand. The test now reshapes
`cc_discharge_lfp.toml` and asserts the reshape actually happened, so a future required
field on `PackConfig` cannot quietly turn it back into a parse test.

### The accumulator's real property is conservation, not "steps == round(delta/dt)"

The interesting test is not that 0.025 s of a 0.01 s step gives 2. It is that over 700
uneven frames, **everything fed in is either consumed or still pending** — nothing
evaporates — and the pending remainder never exceeds one step. That is what makes sim time
track wall time, and it is the property a naive `round(delta / dt)` fails while passing
every single-frame test.

Two smaller decisions the tests pin:

- **A hostile `delta` cannot poison the accumulator.** A `NaN` frame delta contributes
  nothing rather than being added; otherwise one bad frame makes `pending_s` `NaN` forever
  and every subsequent frame does nothing, which presents as a hung simulation rather than
  as a dropped frame. Godot should never produce one — this is cheap insurance against the
  failure being unrecoverable rather than transient.
- **The two caps are genuinely different knobs**, and there is a test that says so.
  `MAX_STEPS_PER_CALL` refuses an absurd explicit batch; `max_steps_per_frame` is an
  *argument*, small on purpose, and a capped frame is a normal event that reports itself.
  Collapsing them would either make fast-forward impossible or let one long frame stall a
  game.

### Two gdext papercuts, neither interesting but both costing a compile

- `bitflags` types do not implement `Default`, so `#[derive(Default)]` on anything holding
  an `EventFlags` fails. Both defaults are hand-written, which is arguably better: "no
  edges" has to mean *empty*, and that is now stated rather than derived.
- `GString: From<&String>` and `From<&str>` exist; `From<String>` does not. Every
  conversion takes a reference.

### What running it found that reading it could not

The shell was driven from a real headless Godot process before this slice was committed,
because a slice that ships fourteen `#[func]`s and never loads once is claiming something
unverified. Everything worked first time, which is worth recording as much as a failure
would be:

```text
chemistry_id_of -> lfp_26650_generic
load_scenario -> true
primed v_terminal -> 3.6         sim_time_s -> 0.0
step_batch(1.0, 100, 2A) -> true
after sim_time_s -> 100.0        v_terminal -> 3.27904127761807   soc_true -> 0.97588159871621
v_bits -> 5ad44cfe793b0a40
bad demand -> false  err='demand is not one of {"Current": A}, … : unknown variant `current`,
                          expected one of `Current`, `Power`, `Voltage`, `Rest` at line 1 column 10'
```

Three things that had been assertions in a plan document became observations here: the
extension registers and its `#[func]`s are callable; the priming reading is a real
measurement; and **the bit-exact interchange slice D depends on works on a value produced
by this crate**, not only on a constant typed into a spike.

The error message is worth its line too — the engine's own serde error survives the whole
way out to a GDScript console and names both what was wrong and what was expected.

### What is deliberately not here

- **No `godot/` project, no `.gdextension`, no signals, no `_physics_process`.** All
  slice C. The load test above ran through the throwaway spike project outside the repo
  tree, deliberately, so slice B commits no Godot project files.
- **No exported properties.** The `#[func]` surface here is what slice B needed to prove
  the path end to end; slice C is where the node gets an editor-facing shape.
- **No `Advance` consumption in the shell.** `BatteryPack::advance` already scopes its
  borrow so the edges can be emitted after it ends — the structure is in place, the
  emission is slice C.

## Learned while building — slice C (the node surface)

### The canary held, and slice C also cost `sim-core` nothing

Still v9. Slice C added twelve exported properties, nine signals, thirty-odd `#[func]`s
and a whole Godot project without touching the engine once — which is what the canary is
for. The one thing that *would* have tripped it, storing the previous flag mask so edges
survive a restore, was refused on exactly those grounds; see below.

### The accumulator would have shipped with no evidence it was connected

This is the slice's most useful finding and it came from asking what the exit gate cannot
see. Slice D drives `step_batch`, because that is the only path whose trajectory is
reproducible enough to assert bit-identity on. The accumulator is therefore **invisible to
the exit criterion** — and it is the deliverable `CLAUDE.md` actually names for Phase 5.

Left alone, it would have had thorough unit tests on its arithmetic in
`crates/sim-godot/tests/driver.rs` and zero evidence that `_physics_process` reaches it.
So `godot/smoke.gd` runs 30 real physics frames and checks that simulated time advanced by
a **whole number of `fixed_dt` steps**, that the carried remainder is under one step, and
that `soc_changed` actually fired.

Note what it deliberately does *not* assert: an exact step count. That depends on frame
timing, which is precisely why the gate does not drive this path — asserting it would
produce a test that fails on a loaded machine while looking like a physics bug.

The check was built to fail before it was trusted. Setting `auto_step = false` — the
one-line version of "the accumulator is not wired up" — produces:

```text
SMOKE FAIL: after 30 physics frames the accumulator advanced nothing
SMOKE FAIL: 30 physics frames of 2 A discharge emitted no soc_changed
EXIT=1
```

Both assertions fire, and the exit code is 1. Without that check the same breakage exits 0.

### Signals: three rules, each of which prevents a specific uselessness

- **Rising edges only**, because `EventFlags` is recomputed every step. A signal on "flag
  is set" fires for the entire duration of a condition — 60 Hz of `protection_tripped`
  while a cell sits over-voltage. Only transitions are reported.
- **Coalesced per batch**, because a `step_batch(10_000)` fast-forward emitting per step
  would push thousands of signals into a game's main loop. The cost is that ordering
  within a batch is lost; a flag that rose *and* fell inside one batch reports both, which
  is a less complete story than ordering but a much more complete one than silence.
- **`soc_changed` needs an epsilon**, because SOC changes every single step. Default
  0.001 — 0.1 % of full charge, which at 1 C is a signal roughly every 3.6 s of simulated
  time instead of every frame.

`PROTECTION` deliberately excludes `SOC_CLAMPED_HIGH`/`SOC_CLAMPED_LOW`: those mean the
engine truncated an over-charge attempt, which is a modelling clamp rather than a
protection device acting. They still reach a listener through `flags_changed`, which is
why that general signal exists at all — no flag the engine can raise should be unreachable
just because it did not earn a named signal.

### The edge detector's state is adapter state, and a restore re-announces

Storing `prev_flags` on `Pack` would make edges survive a snapshot. It would also change
the serialized layout and bump `SNAPSHOT_VERSION` for a purely presentational need — the
exact leak the canary exists to catch.

So the previous mask lives on the node, and the documented consequence is that
[`restore_json`] and `restart` reset it: **conditions already active in a snapshot are
announced again on the next step.** A game hears "protection tripped" for a trip that
happened before the save. That is the failure a listener can cope with; the alternative is
engine pollution. Written down here rather than discovered by whoever first loads a save.

`last_announced_soc` is reset alongside it, for a sharper reason: a restored pack's SOC has
nothing to do with the old one's, so an epsilon gate still measuring against the previous
run's reading would either fire spuriously or stay silent through a large real change.

### The borrow discipline had to be structural, and it is invisible until it is not

A GDScript handler on `protection_tripped` may call straight back into the node. If that
happens while a `&mut` borrow of the driver is live, `godot-cell` panics **at runtime**
with nothing at compile time to catch it — and only once a scene actually connects a
handler, which is a slice later than the code that causes it.

Every mutating method therefore has the same shape, and it is worth reading as one:

```rust
let outcome = {
    let Some(driver) = self.driver.as_mut() else { … };
    driver.step_batch(dt, n_steps, demand)
};                      // <- the borrow ends here, and only here
match outcome {
    Ok(advance) => { …; self.announce(advance); true }   // safe to re-enter
    Err(error)  => self.fail(&error),
}
```

`Advance` is `Copy`, which is what lets the block return owned data rather than something
still borrowing the driver. That is not an accident of the type — it is why it is `Copy`.

### `NAN` as the "no coolant" sentinel, and why it cannot collide

GDScript has no `Option`, so `set_env(t_ambient, t_coolant)` needs some way to say "no
coolant" — which is *not* the same as a coolant at 0 K. A second boolean argument is easy
to pass wrong.

`NAN` works as a sentinel precisely because slice A made it illegal everywhere else:
`Env::check_finite` rejects a non-finite `t_coolant`, so the sentinel can never collide
with a value someone meant. A sentinel that is a legal value would be a bug; this one is
unreachable by construction.

### `res://` is not a filesystem path, and that decided the whole loading API

The node takes scenario and chemistry **text**, never paths. `res://` resolves inside a
`.pck` once a game is exported, so a node that took a path would work in the editor and
fail in a shipped build — the worst possible split, because it passes every test anyone
runs during development.

GDScript reads the files with `FileAccess.get_file_as_string()` and hands over strings.
The same choice `sim-wasm` made for JS, for a different but equally forcing reason.

### Two gdext papercuts, and one Godot project fact

- `emit` takes a `GString` argument **by reference** (`AsArg`), not by value; the
  `i64` arguments beside it are by value. The error is a `ByValue`/`ByRef` type mismatch
  several macro layers deep and does not name the fix.
- Exported enums work through `#[derive(GodotConvert, Var, Export)]` with
  `#[godot(via = GString)]`, which is what makes `backlog_policy` a readable dropdown in
  the inspector rather than an integer nobody can interpret.
- `project.godot` deliberately does **not** set `run/main_scene` yet. Naming a scene that
  does not exist would make every headless run print a load error, and the demo scene is
  slice E.

### What is deliberately not here

- **No demo scene**, and therefore no `run/main_scene`. Slice E.
- **No exit gate.** `smoke.gd` is not it and says so in its own header: it asks "is this
  wired up", not "is the trajectory bit-identical". Slice D.
- **No `Telemetry` → `Dictionary` method.** The open question below is now half-answered —
  the typed getters were enough for everything slice C and the smoke script needed — but
  the demo scene is the first real consumer, so the decision waits for it.

## Open questions

### Resolved in slice C

- **Backlog policy when `max_steps_per_frame` binds.** Resolved as planned: an exported
  `BacklogPolicy` enum, defaulting to `Drop`. A game should stay responsive and pay for a
  stall in simulated seconds rather than in a cascade of capped frames. `Repay` is one
  inspector click away for anything that needs sim time to track wall time. The part that
  is *not* configurable is the announcement — `falling_behind` fires under **both**
  policies, because silent dilation is the failure mode that reads as a physics bug.
- **`soc_signal_epsilon` default.** `0.001`, i.e. 0.1 % of full charge. At a 1 C rate that
  is a signal roughly every 3.6 s of simulated time rather than every frame; the smoke
  script confirms a 2 A discharge emits within 30 physics frames at a tiny epsilon, so the
  gate is doing work rather than being inert.
- **Whether `restore_json` should refuse a topology mismatch.** Yes, and the Godot wrinkle
  turned out to strengthen the case rather than complicate it: the node's `series()` /
  `parallel()` accessors would otherwise describe a pack it no longer holds. Same rule as
  `sim-wasm`, tested in `tests/driver.rs`.

### Still open

- **Does the node need a `Telemetry`→`Dictionary` conversion, or are typed getters enough?**
  A `Dictionary` is one allocation per read and forty string keys; typed getters are forty
  `#[func]`s. Slice C shipped getters for the fields a game polls every frame and needed
  nothing more, but the demo scene in slice E is the first real consumer and is the honest
  place to decide. Likely both, with the `Dictionary` for scripts.
