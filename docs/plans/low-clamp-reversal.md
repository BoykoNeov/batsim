# The low clamp closed: a reversal branch below empty

The last conservation defect in the engine, open since Phase 3 named it and deferred
twice since. `docs/plans/energy-hole.md` closed the charge side and left this:

> **Positive at the bottom**: the cell delivered charge it did not have. That energy is
> *fabricated*, and unlike the overcharge case nothing here corrects it.

`docs/plans/low-clamp-solve-side.md` then spiked three cell-model curves against it and
concluded that **no change to any cell model can refuse a demanded current**, because
`solve_current(Demand::Current(i), e, r)` returns `i` without reading either argument.
That document's item 1 — "a decision about `Demand::Current` with `bms: None`" — was
read as the blocker.

**It was the wrong blocker.** Refusing the current was never the only way to stop
fabricating energy. The other way is to keep delivering it at an *honest voltage*, which
is what a real cell driven past empty does: it goes into voltage reversal, the external
circuit starts paying, and the books close. That needs no change to what a demand means.

## What the three failed curves were each missing

| candidate | direction-correct | linear within a step | continuous |
| --- | --- | --- | --- |
| mode 0 — blocking resistance on discharge | yes | no (`Σ 1/R → 0`, `-inf` terminal V) | no |
| mode 1 — OCV collapses to 0 at empty | **no** (−180 A into a 3.3 V charger) | yes | no |
| mode 2 — mode 1 on the discharge branch only | yes | **no** (32-pass limit cycle) | no |
| **this** — OCV ramps down as a stored deficit grows | yes | yes | yes |

All three failures come from the same place: the collapse was a **branch**, evaluated
inside the step from the current's sign or magnitude. Making it a **state** — a deficit
that accumulates across steps and is read at the start of the next one, exactly as `soc`
is — buys all three properties at once. Within any one step the cell is still a fixed
line, so `is_linear()` stays `true`, the solve stays one pass, and nothing allocates.

## The design

`EcmState` gains a sibling to `soc`:

```rust
/// Charge withdrawn beyond empty, as a fraction of today's capacity.
/// Invariant: `> 0.0` only while `soc == 0.0`.
pub soc_deficit: f64,
```

The coulomb count runs on the *extended* position `soc − soc_deficit`; below zero it
writes the shortfall to the deficit instead of rejecting it. Open-circuit voltage below
empty is the chemistry's `[reversal]` ramp, floored:

```text
OCV_eff = max(OCV(soc) − v_per_soc · soc_deficit,  floor_v)
```

**A sibling field, not a negative `soc`.** That is the whole cost difference. `soc` never
leaves `[0, 1]`, so the `R0` lookup's bracket, aging's SOC-stress table, the BMS
estimator, plating, and `Telemetry::soc_true` are all untouched. A negative `soc` puts
every one of them in play for no physics.

**Conservation is structural, not parametric.** `OCV_eff` is a single-valued function of
the extended position, so stored energy is a state function of it and the ledger closes
for *any* `v_per_soc` and *any* `floor_v` — including a floor above `OCV(0)`, which makes
the branch inert. The parameters set how deep the hole is, not whether it exists. That is
what makes the placeholder values below safe to ship.

## What the measurement said

Spike at `M:\claud_projects\temp\lowclamp2` — a worktree at `7103306` with the candidate
applied, and a probe crate path-depending on it, built from **identical source** against
the patched tree and against the shipped one. The chemical term comes from a state
function (a fine trapezoid over the OCV curve, evaluated only at the run's two endpoints),
never from the step loop — because `energy-hole.md` already measured that `properties.rs`'s
energy balance takes its chemical term from *current* and is therefore an algebraic
identity that goes green on unfixed code.

1S1P LFP, isothermal, no BMS, from `soc = 0.05` at 40 A for 120 s — 4800 As offered
against 415 As stored.

| dt \[s\] | imbalance, shipped | imbalance, unfloored ramp | imbalance, floored ramp |
| --- | --- | --- | --- |
| 0.25 | **−8762.8 J** | +272.5 J | **+18.1 J** |
| 0.5 | −8754.8 | +545.3 | +36.4 |
| 1 | −8739.1 | +1091.6 | +71.5 |
| 2 | −8707.4 | +2184.5 | +141.7 |
| 10 | −8426.6 | +10954.9 | +539.2 |
| 60 | −7591.6 | +69603.6 | +2174.2 |

Two different things are visible in those columns and only one of them is a defect.

**The shipped column does not converge.** Halving the step does not shrink −8763 J,
because it is not discretisation error — it is energy the model made. That is the hole.

**Both candidate columns converge, linearly in `dt`.** 272.5 → 545.3 → 1091.6 is exactly
proportional; so is 18.1 → 36.4 → 71.5. First-order quadrature error on a voltage that
moves within the step, and it goes to zero with the step. The ledger closes.

**The floor is what makes the constant tolerable, and it was the advisor's call before it
was a measurement.** Unfloored, the ramp is unbounded: this run reaches **−52.16 V** on a
single LFP cell in two minutes, and at `dt = 60` its error is *larger than the hole it
replaces*. Floored at 0 V the same run ends at **−1.28 V** and the worst error in the
table is a quarter of the shipped hole at the shipped hole's own worst step size.

**The state trajectory itself is dt-independent** — `x_end = −0.528841631` in every row of
both candidate columns, because coulomb counting under a current demand is exact. What
moves with `dt` is only the energy *integral*, and it moves in the shipped engine too
(3973 J → 3506 J, 12 %).

**Nothing inside the window moves.** The control — `soc = 0.5`, 4 A, 60 × 0.5 s, never
clamps — is bit-identical between the two trees to 17 significant digits:
`elec 3.79784598626338720e2`, `heat 1.18964729044101230e1`. Reproduced in the final tree
before the exit criterion is called met, not argued from "the new term is zero until
`soc` hits 0".

## The heat variant, killed before it was built

The other way to book the reversal work is as heat rather than as negative storage:
`heat = |OCV_eff|·i + i²R`. It conserves too, and it is **forbidden**. That term is
unbounded in the same way the unfloored ramp is — hundreds of watts into a single *empty*
cell within a couple of minutes — it feeds the thermal network, and it reaches
`t_onset_k`. "Over-drain a pack and it vents" would then be reachable from every
drive-flat scenario in the repo, as a modelling artifact wearing emergent physics'
clothes. `CLAUDE.md` forbids exactly that. The negative-storage booking makes the
recharge return the energy instead, which is optimistic (real over-discharge is not
recoverable) and is named as deferred below rather than smuggled in.

## Parameters

A new `[reversal]` section, required rather than optional, because a chemistry that
cannot say how its cells reverse should say so loudly rather than silently inherit a
number from code. Every value is a labelled placeholder under `CLAUDE.md`'s provenance
rule:

```toml
[reversal]
# Placeholder — order-of-magnitude only, TODO fit. Sized so the cell collapses from its
# empty-endpoint OCV to the floor over 2 % of capacity.
v_per_soc = 100.0
floor_v   = 0.0
```

`v_per_soc` is `OCV(0) / 0.02` per file: 100.0 (LFP, `OCV(0) = 2.0`), 150.0
(NMC 18650, 3.0), 125.0 (NMC 21700, 2.5). The floor is 0 V everywhere — a real reversed
cell continues to roughly −1 to −2 V on a copper-dissolution plateau, and 0 V is the
conservative placeholder.

**Why not derive the slope from the OCV table's own bottom segment?** It was considered
and rejected. It needs no new parameter and is even C1-continuous at the joint, but the
bottom segment is a deliberately steep table *endpoint* rather than a reversal
measurement, it differs between chemistries for unrelated reasons, and it degenerates on
a flat first segment — where the slope is zero, the ramp is flat and the hole comes
straight back. A guard against that would be an unlabeled constant in code, which is what
the TOML section exists to avoid.

## Versions

| constant | before | after | why |
| --- | --- | --- | --- |
| `sim_core::SNAPSHOT_VERSION` | 13 | **14** | `EcmState` gains a stored field, and it is state rather than cache |
| `sim_server::API_VERSION` | 2 | **2** | `CellView` gains a field; additions are exempt by its own rule |
| `sim_wasm::WASM_API_VERSION` | 5 | **6** | `web/pkg` is loaded separately from the JS that reads it |

Each constant's own doc gets read individually rather than bumped as a set — the pair has
now parted four times (`ui-bms-view`, `surface-vs-bulk`, `dfn-scenario`, here).

**The wasm bump was not in the draft**, and the reason it is here is worth recording
because it started as a scoped-out item (see the deferred list below, as first written).
Leaving `soc_deficit` off `CellView` looked like scope discipline until the test pass
reached `properties.rs`: `charge_conserved_through_a_soc_clamp` reads ground-truth stored
charge through `CellView::soc`, which is clamped, so past empty the pack's true charge
stopped being **observable through the public API at all** and the property's low arm went
from exact to unwritable. The choice was a new dev-dependency to read the field out of a
serialized snapshot, or the field itself. The field is the honest one — a client watching
a pack go into reversal wants exactly this number — and it costs one adapter constant.
`WASM_API_MIN` in `web/app.js` is deliberately **not** moved with it: no page reads the
field yet, so nothing there has anything to refuse.

## Exit criterion — met

*(Drafted before the work; every clause below was checked, and the measurements are in
"What the build found".)*

A cell driven past empty and charged back to where it started shows **zero net energy
imbalance** — no state-function integral in the assertion, because a closed cycle in state
space needs none — where the shipped engine fabricates kilojoules over the same cycle;
`i_rejected_a` is exactly zero at the bottom of the window and `SOC_CLAMPED_LOW` now means
what the porous-electrode models already mean by it; the solve still takes one pass past
empty; and the in-window control reproduces the two 17-digit figures above **measured in
the final tree**.

---

# What the build found

Everything above the versions table is the plan as drafted against the spike. This is what
changed when it was written into the tree.

## The tripwire fired, exactly as its author designed it

`properties.rs`'s `the_bottom_of_the_window_fabricates_exactly_what_it_reports` carried
this:

> **This test fails if the fabrication is fixed properly**, and that is intended: a
> solve-side fix (an empty cell that stops sourcing) makes both sides zero and the
> `fabricated > 0.0` coverage assertion is where it announces itself, rather than a golden
> shifting by an amount nobody can attribute.

It announced itself on exactly that assertion. The prediction was wrong about the
*mechanism* — the fix is not solve-side, because no cell model can refuse a demanded
current — and right about everything that mattered: a defect being fixed showed up as a
named assertion failing rather than as a number drifting. **A test written to fail when a
known defect is repaired is worth more than a comment saying the defect exists.**

## The property that quietly depended on `soc` being the whole truth

`charge_conserved_through_a_soc_clamp` weighs `∫(S·i_actual − i_rejected) dt` against
ground-truth stored charge, and it read that charge as `Σ soc · capacity`. With the
deficit carried outside `soc` that sum stops moving at the bottom of the window, so the
property's low arm did not merely become inexact — it became **unwritable**, because the
quantity it needs was no longer observable through the public API. That is what put
`CellView::soc_deficit` back in scope after the plan had deferred it. Recorded because the
failure did not look like a missing accessor: it looked like a conservation test failing
on a fix that conserves.

## Thirty-four files whose line endings changed, found by two failures that named neither

The `[reversal]` section is required, so 34 fixture constructors and TOML strings needed
it. Inserting them with a Python script rewrote those files in text mode, which on Windows
turns every `\n` into `\r\n` — and this repo stores LF. Nothing about the diff looked
wrong (`git diff --stat` showed 4 added lines per file, because git's autocrlf hid it).

What surfaced it was two unrelated-looking failures: `sim-godot`'s bundled-asset test,
which byte-compares the demo scene's chemistry against the canonical one, and a
`sim-server` REST test that inlines a chemistry into a scenario as a TOML multi-line
literal and asserts the round trip is byte-for-byte — the TOML parser normalises `\r\n` to
`\n` inside such a string, so the echo came back shorter than what went in. **Two
byte-comparison tests, in two crates, neither about line endings, and both of them right.**
The check that found it was comparing each modified file's endings against `git show
HEAD:<path>` rather than reading the diff.

## Three test assumptions that were wrong, and what each one was hiding

* **Landing exactly on empty raises no flag.** `coulomb_step` clamps on `raw < 0.0`, so a
  step sized to hit `0.0` exactly has not passed anything. The test asserting
  `SOC_CLAMPED_LOW` there was wrong, and inverting it made the test *better*: that state —
  empty, deficit still zero — is the last instant at which the reversal branch and the
  rejected collapse-at-empty candidate are distinguishable, which is precisely what the
  test is for.
* **"Past the floor the voltage stops moving" is not bit-equality.** It failed at
  1.2e-7 V, and that residue is not the branch: at 300 s the RC overpotential is 15 time
  constants into a relaxation toward 0.4 V, which is `0.4·e⁻¹⁵` — the number to seven
  digits. An assertion about one mechanism was measuring another.
* **The cycle's energy residue has a derivable size, and deriving it was cheaper than
  tuning a tolerance.** Each step closes *exactly* against the OCV at the state it started
  from, so a loop's residue is the rectangle rule's error going round it. Summed across
  the ramp it telescopes to `S · OCV(0) · I · dt` — independent of the reversal slope, of
  the capacity, and of how many steps it took. Measured at 1.5× that, so the test's factor
  is margin over a derivation rather than a number chosen to make a case pass. The first
  attempt used 1 % of the heat, which is a quantity with no relationship to the error at
  all.

## The exit criterion's last clause, measured

The probe was re-pointed at the finished tree and run again. Both halves hold:

* **The in-window control is bit-identical to the pre-fix engine** —
  `elec 3.79784598626338720e2`, `heat 1.18964729044101230e1`, all 17 digits, against the
  run taken at `7103306` before a line was written. Measured, not argued from "the new
  term is zero until `soc` hits 0".
* **The shipped implementation reproduces the spike digit for digit** — 18.0967 J at
  `dt = 0.25` through 2174.2277 J at `dt = 60`, and `−1.279008 V` at the floor, every
  figure equal to the worktree candidate's. The code that landed is the code that was
  measured, which is not something a rewrite between spike and slice can be assumed to
  preserve.

**The first run of that check reported an imbalance of −107 kJ, and the engine was
innocent.** The probe exists in two copies — one per tree — and the baseline copy had been
taken *before* the floor was added, so its reference curve integrated an unbounded ramp
while the engine it was weighing had a floored one. The tell was that the discrepancy was
exactly `cap_as · ∫(2 − 100|x|)` over the run's depth, i.e. precisely the area the floor
removes; the engine's own `v_end` had been reading the floored `−1.279 V` the whole time.
**When a fix and its instrument are edited in the same session, an instrument that was
copied earlier is a stale fork** — and a scripted edit that silently matches nothing is
how it stays stale. Both patches now assert their target string is present before writing.

## What the perturbations caught, and the one that surprised

Four changes applied to the finished tree one at a time, each reverted from saved bytes
rather than with `git checkout`, and read by **exit code** — a control that changes only a
comment is included because a harness that reports RED for everything proves nothing.

| perturbation | expect | got | caught by |
| --- | --- | --- | --- |
| a comment word (control) | green | green | — |
| the floor removed | red | red | the cycle **and** the floor test |
| the ramp made inert (`v_per_soc` forced to 0) | red | red | **the floor test only** |
| the shortfall rejected again instead of carried | red | red | three tests, incl. the cycle |

**The third row is the informative one.** A flattened ramp puts the cell back to sourcing
at `OCV(0)` forever — the original defect's headline symptom — and the *energy* tests do
not notice. They are right not to: with the deficit still carried, `OCV_eff` is still a
single-valued function of the extended position, just a constant one, so the cycle still
closes. That is the "conservation is structural, not parametric" claim from the design
section, arriving as a measurement and as a warning: **the energy tests do not guard the
ramp, and a reader who assumes they do would delete the floor test as redundant.** What
the ramp is guarded by is the voltage it produces, which is what
`the_reversal_floor_bounds_the_voltage` reads.

## The version bump that can no longer prove what its predecessors proved

`snapshot_version.rs` exists to separate "the version field rejected this blob" from
"deserialization rejected it", and v10 through v13 could each name a pack whose old bytes
were byte-identical under the new build, so the version check was demonstrably the
decider. **v14 cannot, and the file now says so.** The `[reversal]` section is required
and carries no `#[serde(default)]` — the only default available would be an unlabeled
physical constant — and the chemistry sits inside every snapshot whatever cell model the
pack runs, so no v13 blob parses here at all. The retagged-bytes pair still proves the
check is wired and consults the outer tag; it no longer proves a real stale snapshot meets
it. Stating the narrower claim is the point: the next bump inherits a test whose docstring
says which of the two things it is doing.

## Deferred, with a price

* **Over-discharge is recoverable at 100 %.** A scenario can pump a pack below empty and
  back for free. The honest coupling is to `soh_capacity` / `soh_resistance` — real
  reversal dissolves the anode current collector — and that is an aging-model change, its
  own slice and its own constants.
* **No client shows the deficit yet.** `CellView::soc_deficit` is on the wire (see the
  versions table above for why that stopped being deferrable), but `web/app.js` neither
  reads it nor offers it as a pack-grid metric, and the guided path has no step about
  reversal. That is a UI slice, and it is now cheap: the data is already there.
* **The high clamp keeps its hard cap**, deliberately. A symmetric branch there would
  delete the refused-charge heat term `energy-hole.md` shipped, which on an ECM is now
  *the* runaway path.
* **Fitting the reversal curve.** Two placeholders per chemistry, both labelled.
