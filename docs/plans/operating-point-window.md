# The client naming a current did not know where it was going either

`docs/plans/power-operating-point.md` shipped `EventFlags::POWER_OUT_OF_WINDOW` and, in the
same breath, declined to let a current demand raise it. The reason it gave:

> **with a current demand the client chose the operating point and knows it; with a power
> demand the engine chose it and the client cannot predict it.**

Half of that is true. The client did choose the *current*. It did not choose, and cannot
predict, the **voltage** — which is the quantity the window is about. `Current(40.0)` says
nothing whatever about where the terminal lands: that depends on the resistance table, the
state of charge, the accumulated overpotential, and how long the cell has been under load.
A client that knew where it was going would not need a simulator.

The deferral named its own price — "it would move flags on existing hard-discharge
trajectories rather than only on power demands, so it needs its own argument and its own
blast-radius measurement". This is that argument and that measurement.

## The flag is renamed, because its name was a claim about demands

`POWER_OUT_OF_WINDOW` → **`OPERATING_POINT_OUT_OF_WINDOW`**, same bit (`1 << 13`). The name
now says what the predicate tests rather than which demand happened to ask it, which is the
whole content of this slice.

## The predicate moves from the series sum to the group

Not cosmetic, and not a consequence of the widening as such — a consequence of *where* the
widening puts the flag. The shipped predicate tested the pack terminal against
`series × [v_min, v_max]`, which cannot see imbalance: one group at 2.4 V and another at
3.4 V sum to a terminal that divides back to a perfectly in-window 2.9 V.

That blind spot was nearly harmless while only `Demand::Power` asked, because an
engine-chosen operating point misses the window by orders of magnitude rather than by a
scatter's worth — 162 kV on a 4.2 V cell is not a number any averaging rescues. An ordinary
current demand is the opposite case: it lands *near* the edge, on a pack whose cells
disagree, which is the regime a pack layer exists to represent at all.

So the predicate is now each parallel group's own node voltage against `[v_min, v_max]`,
evaluated in the per-cell loop on the same `v_node = E_g − i_g·R_g` that the current split
already computes. On a 1S pack the two readings are the same number. It also costs less
than what it replaces: the two `series`-long sums are gone and the check rides a loop that
was already running.

**Unmeasured, deliberately.** `docs/plans/pack-step-perf.md` has `Pack::step` marginal
against its < 50 µs budget, so a change that touches every step deserves the question. It
is not benched here because the change is a strict removal — two sums deleted, one
comparison added inside an existing loop — and the standing rule is that test and bench
runs go at below-normal priority, which makes a marginal budget unmeasurable anyway. The
claim made here is "strictly less arithmetic than the shipped version", not "perf-neutral,
measured".

## What asks, and what does not

| demand | raises it | why |
| ------ | --------- | --- |
| `Power` | yes | unchanged; the engine picks the point |
| `Current` | **yes, new** | the client picked the current, not the voltage |
| `Voltage` | no | clamped into this same window before the solve; cannot leave it |
| `Rest` | **no, deliberately** | see below — this one is not hygiene |

## `Rest` is excluded, and the exclusion is load-bearing

An open-circuit pack below `v_min` is a reversed cell, and `SOC_CLAMPED_LOW` is raised for
precisely that state — its own doc already explains that the open-circuit voltage is falling
toward `reversal.floor_v` and that the terminal has gone negative. A second flag on one
condition is the overload this crate has paid for elsewhere (`SOLVE_UNCONVERGED` carries two
meanings and says so).

That is the argument. **The measurement is what settles it**, and it was taken by building
the wider version and running the client's own lessons through it. Two steps of the guided
path teach the external-short ladder on packs whose demand is `Rest` for the whole run —
the lesson's own words are "there is no `OC`, because over-current is judged against what
you requested, and you requested `Rest`":

| step | scenario | with `Rest` asking | prose at risk |
| ---- | -------- | ------------------ | ------------- |
| `one-step-that-got-through` | `external_short_30_milliohm` | **raised at 60.5 s**, group at 1.3336 V | *"draws 183.84 A for a single step with no flag raised at all"* |
| `nothing-to-clamp` | `external_short_100_milliohm` | never raised | *"73 seconds of no flags at all"* |

The first is the one that decides it: the flag lands on **exactly the step the sentence is
about**, at exactly the 1.3336 V the sentence quotes. The second was never in danger — that
short sags the groups to 2.13 V, above LFP's 2.0 V floor, which is what the lesson says and
why the fault slips between the BMS's two rungs.

So excluding `Rest` is what keeps a shipped lesson true, and the reasoning about flag
overload and the client evidence point the same way.

## Blast radius

**The shipped form**: `cargo test --workspace --no-fail-fast`, complete capture — **67 test
binaries, 0 failures**. No golden moved, no scenario assertion moved, no path claim moved.
Clippy clean at `-D warnings`, `cargo fmt` clean.

That green is not vacuous, and it was checked rather than assumed. On the shipped
`over_discharge_damage_lfp` scenario at its own `Current(2.0)` and 0.5 s step:

| | when | terminal |
| --- | ---- | -------- |
| `OPERATING_POINT_OUT_OF_WINDOW` first raised | **199.0 s** | 1.9953 V |
| `SOC_CLAMPED_LOW` | 207.5 s | 1.9306 V |

The window report **leads the existing clamp flag by 8.5 seconds** on a trajectory that
ships today — it says "this cell is under its floor" while the charge readout still shows
charge left. So the flag fires in real trajectories, and no existing assertion cared.

## What it falsified: a lesson no check protects

The guided path's `protection-off` step drives `Current(40)` into an unprotected pack and
said, of the result:

> With it gone the pack does not merely fail to stop: **it fails to say anything.**
>
> …the only flag you will see is `SOC_CLAMPED_LOW`

Both sentences are now false. The step carries **no claim**, so the value checker is
structurally blind to it — this is the "fourteen unclaimed steps" gap
`docs/plans/path-accounting.md` deferred, meeting a change that walks straight through it.
Found by reading for capability rather than by grepping for a literal.

The repair is not a patch, because the lesson is *better* with the flag in it. Its subject
is the contrast between a pack that is watched and one that is not, and the accurate version
of that contrast is not "nothing is reported" but "nothing *acts* — the engine still knows".
Measured on an in-tree reproduction of the step (same scenario, `build_pack_with_bms(false)`,
same `dt = 0.5`, same 25 °C ambient with no coolant, which is exactly what `app.js` sends):

* `OPERATING_POINT_OUT_OF_WINDOW` at **335.0 s**, weakest group **1.9731 V**, pack still at
  3.7 %;
* `SOC_CLAMPED_LOW` at **345.0 s** — which is the number the step's own prose already
  quotes, and is the check that the reproduction is faithful.

### Two numbers in that step were already wrong, and this is how they were found

Rewriting the sentence next to them meant measuring them, and they do not survive: the step
claimed **−2.2619 V** at the mark with every cell between −0.57 and −0.56 V, and measures
**−2.6355 V** with cells between −0.671 and −0.651. The cause is attributable rather than
mysterious — this scenario is one of the few with `[pack.aging]` **on**, so the
over-discharge damage of `docs/plans/reversal-damage.md` and the RC resistance growth of
`docs/plans/rc-resistance-growth.md` both bill against it, and both landed after these
numbers were written. The timing numbers (345.0 s) were untouched by either, which is why
nothing looked wrong.

Corrected in place, along with the `past empty` figure, which is now given as the range the
pack grid actually shows (**23.5 to 27.8 points** across the eight cells) rather than as a
single number whose aggregation was not recoverable from the prose.

**The general point is worth more than the two numbers.** A step with no claim is a step
where a physics change is free to silently invalidate the teaching, and two separate slices
did exactly that here before this one arrived.

## Versions

Read from each constant's own doc, per the standing rule that these have parted.

* `SNAPSHOT_VERSION` (17): unmoved. Flags are recomputed fresh every step and are not part
  of the snapshot.
* `API_VERSION` (2) and `WASM_API_VERSION` (6): unmoved, and this needed more care than the
  previous slice's addition did, because a **rename** is a stronger move than an addition.
  Flags cross the wire as a `" | "`-joined name string, so the text a client receives
  changes. What these constants version is method names and JSON field names, and a flag
  name is neither — but the real question is whether any client *matches* on the old name,
  and every list was read rather than assumed:
  * `web/app.js` — `parseFlags` keeps every name it is given and renders each as a chip;
    the two hardcoded lists are `PROTECTION_FLAGS` (`UT`, `OT`, `OV`, `UV`, `OC`,
    `CONTACTOR_OPEN`) and `SEVERE` (`THERMAL_RUNAWAY`, `VENTED`, `CONTACTOR_OPEN`,
    `PLATING_RISK`). Neither named the old flag.
  * `crates/sim-godot` — its protection set is built from `EventFlags` constants in Rust, so
    it is renamed by the compiler or not at all.
  * No test pins the full set of flag spellings; `wire_json.rs` names three (`OV`, `UV`,
    `THERMAL_RUNAWAY`) and round-trips `EventFlags::all()` by value, so nothing there sees
    the change either.

  So the rename breaks no client, and the only cost is the chip text a reader sees.
  `web/pkg` is a local build artefact and is not committed; a page must be rebuilt against
  the new engine to show the new name, as after any engine change.

## Verification

Thirteen tests in `crates/sim-core/tests/power_operating_point.rs` — the nine that shipped
with the flag, plus four for what this slice changed — on the same fixture whose every
threshold is derived rather than measured: at 50 % SOC the source is 3.20 V behind 0.02 Ω,
so the in-window current band is exactly ±15 A.

The test that had to be **inverted** rather than added is the important one.
`the_same_operating_point_from_a_current_demand_is_not_flagged` was the codified form of the
decision being reversed; it is now `the_same_operating_point_flags_from_either_demand`, and
it is strictly stronger than either half on its own, because it reads the current off a
power probe and hands it straight back. The two demands therefore solve the *identical*
point and differ only in what was asked, so nothing about the demand can enter the predicate
without it going red.

`a_rest_demand_never_raises_it_even_below_the_window` carries a guard that is worth naming:
after asserting the rest step is silent, it asserts that the **same pack** under a current
demand does flag. Without it the test would pass on a pack that was simply in window, which
is the failure mode an exclusion test is prone to.

### Perturbations

Four, each launched through `subprocess` with `BELOW_NORMAL_PRIORITY_CLASS` rather than
`start /belownormal`, because the standing rule here is that `start` is **exit-code-blind**
— proven twice in this repo. Every row below is a real exit code.

| perturbation | exit | red |
| ------------ | ---- | --- |
| Restore the aggregate predicate (series sum) | 101 | **1** — `the_predicate_sees_a_group_the_series_sum_averages_away` |
| Narrow back to `Power` only | 101 | 5 |
| Neuter the raise entirely | 101 | 11 of 13 |
| Swap to the NaN-blind spelling `v < lo \|\| v > hi` | 101 | **2** — both non-finite tests, and nothing else |

The first and last rows are the ones that make specific decisions load-bearing rather than
decorative: exactly one test can see the per-group/aggregate difference, and exactly the two
non-finite tests can see how the comparison is spelled. The eleven-of-thirteen row leaves
the same two survivors the previous slice's perturbation left — `a_voltage_demand_can_never
_raise_it` and `the_in_window_band_is_where_the_arithmetic_puts_it`, both of which assert
the flag stays *down* and so are inertness guards rather than flag guards.

A fifth perturbation was run as a **complete workspace suite** rather than on one binary,
because its subject was the client rather than the engine: including `Demand::Rest` reddens
**one test in 67 binaries** — the one written for it — while falsifying a shipped lesson
sentence at exactly the step that sentence is about. That gap between "nothing goes red" and
"the teaching is now wrong" is the finding this slice would most like to be remembered for.

### Suite

`cargo test --workspace --no-fail-fast`: **67 test binaries, 0 failures**, complete capture.
`cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo fmt --all` clean.
No golden CSV moved and none could — the predicate reports, it never moves a current.

One suite run was lost to a self-inflicted collision worth recording: `wasm-pack build` was
launched while `cargo test --workspace` was still running, and the two cargo invocations
fought over the same `target/`. It surfaces as a `link.exe` failure carrying a "the Visual
Studio build tools may need to be repaired" note — a broken-toolchain message for a plain
concurrency problem.

## Deferred, with a price

* **An open contactor makes a `Current` demand behave like a `Rest` one, and the question
  is still asked.** With the contactor open `i_g` is zero, so `v_node` is the group's
  open-circuit voltage — exactly the quantity `Rest` was excluded over. A pack sitting
  open-circuit below `v_min` under a standing current demand therefore keeps raising this,
  where the same pack under `Rest` would not. Nothing in the suite or the shipped scenarios
  is in that state (a BMS trips under-voltage on the *loaded* voltage, which recovers above
  the floor once the contactor opens), and where it is reachable at all `SOC_CLAMPED_LOW` is
  there too. Recorded rather than fixed: the honest repair is a predicate that asks whether
  current is *flowing* rather than what was asked, which is a different design and wants its
  own measurement.
* **Nothing says *which* group left the window.** The flag is one bit for a pack that may
  have one group under its floor and eleven fine. A client that wants to know reads
  `Pack::cell`, which is ground truth and always available; a client watching only telemetry
  cannot tell a single weak group from a pack-wide collapse. That is the same trade
  `SOLVE_UNCONVERGED` makes and it is made here for the same reason — a new `Telemetry`
  field is a wider change than a flag bit.
* **`Rest` reports nothing even when a rested pack is genuinely out of window for a reason
  that is not reversal** — a chemistry whose `OCV(0)` sits below its own `v_min`, say. No
  shipped chemistry does, and `SOC_CLAMPED_LOW` covers the case that exists today, but the
  exclusion is by demand rather than by cause.
* **The fourteen unclaimed lesson steps are the real gap**, and this slice is the second
  piece of evidence for it rather than an argument about it. Two prior slices left stale
  numbers in `protection-off` and nothing noticed; this one would have falsified two
  sentences in `one-step-that-got-through` and nothing would have noticed. See
  `docs/plans/path-accounting.md`, which deferred exactly this and priced it.
