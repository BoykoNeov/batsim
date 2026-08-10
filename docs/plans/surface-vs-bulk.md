# Surface vs bulk: the gradient an equivalent circuit cannot have

The item `docs/plans/spm-scenario.md` closes on, quoted rather than paraphrased:

> **Surface-vs-bulk stoichiometry on the wire.** The thing no equivalent circuit can show
> at all — the reader would *see* the gradient rather than infer it from a rebound. It is
> a `Telemetry`/`CellView` change, therefore `API_VERSION` and `WASM_API_VERSION`
> together, a `web/pkg` rebuild, and the out-of-tree trajectory instrument enumerates its
> 17 telemetry fields by name so an 18th is invisible to it. A slice of its own.

Not a numbered phase. Phases 6 and 7 built two porous-electrode models; every client can
select them and none can see the one quantity that makes them different from a circuit.

Three of that paragraph's five clauses are wrong, and the spike below is what says so.

## What is missing, stated as source

`CellView` carries ten fields. `overpotential_v` is the *voltage* consequence of a
concentration gradient, and it is model-neutral by design — an ECM answers it with
`Σ V_rc`. Nothing on the wire carries the gradient itself, so a reader watching a 3C
discharge end at 61.3 % SOC has to take "the surface is empty even though the bulk is not"
on the prose's word.

`CellModel::surface_*` does not exist. `spm::c_surface` and `dfn`'s per-node equivalent are
computed inside every voltage evaluation and thrown away.

## What the spike measured

Worktree `M:\claud_projects\temp\surface-spike` at `d68e806`, probe
`M:\claud_projects\temp\surface-probe`, full record in that directory's `FINDINGS.md`.
Four results, two of which were blocking and one of which changes the headline.

### 1. A surface *level* leaves [0, 1]; a *gap* never does

Surface stoichiometry mapped onto the SOC window reaches **1.244718** on a 3C DFN
overcharge (SPM: 1.161385). `CLAUDE.md` states `SOC ∈ [0, 1]` as an invariant, so a field
named `soc_surface` would break it — and clamping would erase exactly the signal.

A discharge cannot find this: on every discharge run, at 1C and 3C, on both models, the
level stays inside [0, 1]. Measuring only the guided path's 3C discharge would have
produced the wrong answer with full confidence.

**Worse, the level's gap against `CellView::soc` is contaminated by a clamp.** After the
3C overcharge, at rest, `soc` reads exactly `1.000000` while the relaxed, uniform surface
reads `1.186667` — the entire standing `+0.186667` is the clamp on `soc`, not a gradient.

**The difference of two unclamped x-means dissolves all of it**, and relaxes to exactly
zero in every case measured, including the one the clamp broke:

| t_rest \[s\] | DFN 3C discharge | DFN 3C charged past full |
| --- | --- | --- |
| 0 | 0.058047297 | −0.058051592 |
| 300 | 0.000004908 | −0.000004866 |
| 7200 | **0.000000000** | **0.000000000** |

So the field is a **gap**, not a level; it is not a state of charge, so the `[0, 1]`
invariant never applies to it, and the naming fork closes.

### 2. The reduction over x must be the mean

DFN 3C, gap to the x-mean bulk at rest:

| t_rest \[s\] | mean over x | min over x |
| --- | --- | --- |
| 0 | 0.058047 | 0.319394 |
| 300 | 0.000002 | 0.221236 |
| 7200 | 0.000000 | **0.028918** |

`dfn::raw_soc` reduces the bulk with `bulk_stoich`, an x-**mean**. Differencing an
x-extreme against it differences two different spatial reductions, and nothing moves solid
lithium between x positions except the small reaction currents a relaxing electrolyte
drives — so the extreme carries a standing offset that two hours of rest does not remove.
"Rest it and watch them converge" only works for the mean.

### 3. The **positive** electrode is where the gradient lives, by ~6× — and it moves the headline

| run | gap, negative | gap, **positive** |
| --- | --- | --- |
| DFN 3C, at its 464 s cut-off | 0.058047 | **0.331415** |
| SPM 3C, at its 1060 s cut-off | 0.058052 | **0.371807** |
| DFN 1C, at its 3484 s cut-off | 0.019351 | **0.126673** |

`dfn.rs:219` already said the positive is the electrode that saturates and ends a hard
discharge. This puts a number on it. A negative-only field — the obvious choice, because
`soc` itself is defined from the negative — would understate the headline by 5.7× and name
the wrong electrode as the reason a 3C discharge stops. **Both electrodes ship.**

### 4. The gap does not separate SPM from DFN, and that is a result

At each model's own 3C cut-off, on the same 15.459594 A, the negative gaps agree to **5
parts in 10⁵** (0.058047 vs 0.058052). Not a coincidence: the radial gradient is set
quasi-statically by the current, and the DFN's 1C gap is ≈ ⅓ of its 3C one — linear in
current.

This falsifies a purpose that was inferred rather than stated. `spm-scenario.md`'s claim is
**ECM-vs-porous** ("the thing no equivalent circuit can show *at all*"), and 14 of the 18
guided-path steps are ECM, so the contrast the field was priced for is the one that holds.
What it does not do is sharpen steps 15/16's SPM-vs-DFN comparison — that difference lives
in the **spread across x** (DFN 0.406 neg / 1.167 pos at the 3C cut-off, against the SPM's
structural 0.000000), which this slice defers with a price below.

## Part A — `sim-core`: two fields and one accessor

`CellView` gains two fields, both `Option<f64>`, in units of state of charge (a fraction of
that electrode's usable stoichiometry window, the same scale `soc` uses):

```rust
/// Bulk minus surface stoichiometry on the negative electrode, discharge-positive.
pub surface_gap_neg: Option<f64>,
/// The same on the positive electrode.
pub surface_gap_pos: Option<f64>,
```

* **Discharge-positive**, matching `overpotential_v`, of which this is the concentration
  half. `bulk − surface`, so a discharge reads positive.
* **`None` for `Ecm1Rc`/`Ecm2Rc`.** Not `0.0`: that is precisely the `v_rc_sum → 0.0` trap
  the `overpotential_v` rename was paid to remove — indistinguishable, to a plotting
  client, from a real measurement of a relaxed cell. An ECM has no surface at all.
* **Both sides unclamped**, per measurement 1. Neither `soc`'s clamp nor `raw_soc`'s
  absence of one is visible in a difference of two quantities on the same scale.
* Fed by a new `CellModel::surface_gap(&self, chem, eff_r0_factor, eff_capacity_ah) ->
  Option<(f64, f64)>`, dispatching exactly as `overpotential_v` does, onto new
  `spm::surface_gap` and `dfn::surface_gap`.

Both are **pure functions of stored state** — the post-step profile plus the converged
`DfnState::u` (or `SpmState::i_last`). No `dt` enters. That is what keeps
`SNAPSHOT_VERSION` at 13, and it is the same argument `overpotential_v` makes.

`Telemetry` gains **nothing**. The only DFN configuration this repo ships is 1S1P (a DFN
pack is ~18 ms/step, per `dfn-scenario.md`), so `cell(0, 0)` *is* the pack for every
scenario this field targets, and a pack-level twin would be a second name for the same
number.

## Part B — adapters, and the version constants parting for the third time

`spm-scenario.md` priced this as "`API_VERSION` and `WASM_API_VERSION` together." That is
wrong, and each constant's own doc says so — the mistake Phase 6 recorded, the
`ui-bms-view` slice paid, and the energy hole paid again:

* **`sim_server::API_VERSION` stays at 2.** Its rule is explicit: "Adding a field or an
  error code does not bump it." These are added fields. Precedent: Phase 6 slice B's
  `"spm":null` and the energy hole's `i_rejected_a`, both exempted by the same clause.
* **`sim_wasm::WASM_API_VERSION` moves 4 → 5**, with `web/app.js`'s `WASM_API_MIN`. Not for
  a rename — for the reason its own v3 and v4 paragraphs give: `web/pkg` is a build
  artifact loaded separately from the JS that calls it, and against a v4 bundle the page's
  new read is `undefined` inside a draw call.
* **`sim_core::SNAPSHOT_VERSION` stays at 13.** No stored state changes.

Read a constant's doc before paying a planned bump. Third time.

## Part C — the page

* One `METRICS` entry per electrode, so the pack grid can colour by the gap. The ramp
  normalises on a range, and `null` is not a number — but `CellModelConfig` is *pack*-level,
  so the field is null for every cell or for none. That makes it a **metric availability**
  question, not a per-cell null path: omit both entries from the selector when
  `cell(0,0).surface_gap_neg` is null, and say why in the empty state rather than showing a
  metric that renders as a flat grid.
* Two lines in the cell tooltip, printed in points of SOC (`×100`), 2 dp — the panel's SOC
  readout at 1 dp is too coarse for a 0.058 gap, which is `dfn-scenario.md`'s
  "the panel's precision is not the engine's" recurring.
* `web/pkg` rebuilt. Any Rust change requires it; this one is load-bearing.

## Part D — one lesson, and it must be a `Pulse` step

A new guided-path step, inserted after the current 16 (renumbering 17 and 18 to 18 and 19 —
the step that wedges the CDP harness becomes **19**, and it already wedged at `HEAD~1`, so
nothing here creates it). Verified safe to insert there: every absolute `step N` reference in
`web/app.js`'s prose points at 1–15, so none of them shifts.

The lesson is the rebound `spm-scenario.md` had the reader *infer*: discharge to the
cut-off, rest, watch the gap collapse and the voltage come back with it.

**That is two demand phases, and a step record carries exactly one `demand`.** Written as
two steps it would break: the rest step's whole claim is about state inherited from its
predecessor, and `applyStep`'s reload rule reloads on `$("scenario").value !== L.scenario` —
so arriving by **Back** from step 19 (a different scenario) rebuilds a fresh pack at t = 0
and the step's own number is zero. That is `spm-scenario.md`'s recorded inequality — a
reload rule that only holds one direction — recurring in a new place.

**So the step uses `Pulse`**, the demand mode the SPM slice added, with a single long leg:
`{ mode: "Pulse", value: 15.459594, on_s: 1060, off_s: 600 }`, `reload: true`, mark at the
end of the rest leg. One step, one demand record, both phases, and correct arriving from
either direction — `Pulse`'s leg is a function of `sim_time_s`, so it needs no inherited
state at all.

The SPM arm, not the DFN, because measurement 4 says the gap is the same in both and the
SPM step is 200× cheaper.

**Every number below is the spike's prediction, taken from the native API at `dt = 2`, and
is to be replaced by the panel reading before this step ships.** Writing "what you will
see" from reasoning has been wrong three times in this repo's own record, and once in a
neighbouring shape (a status line reasoning about a frame from the previous demand). The
predictions: positive-electrode gap **0.372** at the cut-off, **0.000** by the end of a
600 s rest; negative **0.058** → **0.000**.

## Part C addendum — the headline needs a surface that paints without interaction

On a 1S1P pack the grid is **one square**, a lesson cannot instruct "hover cell (0,0)", and
a hover cannot be reliably CDP-driven under `--headless=new`, where rAF does not fire and
one screenshot is one frame. So the metric entry and the tooltip are right for multi-cell
packs and **cannot carry this lesson**.

The gap therefore also gets a `#readouts` line — which is where the precision lives anyway,
the panel's SOC readout being 1 dp against a 0.058 effect.

## Verification

* `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all`. Note `cargo test --workspace` **stops at the first failing binary**, so
  a perturbation table built on it understates coverage — run the perturbations against the
  final tree and per-binary, which is `dfn-aging-gap`'s lesson.
* **The trajectory instrument.** `cell_bits` gains two entries, `unwrap_or(f64::NAN)` on
  `soc_bms`'s precedent. Predicted diff shape, so a wall of moved lines is not read as a
  regression: **every** case's per-cell block changes length, and every ECM case carries the
  same NaN bits and therefore discriminates nothing. The discrimination comes from the two
  DFN cases and the SPM ones — and the instrument's DFN conditions (2 A ≈ 0.4C from 85 %)
  were measured to give gap neg **0.007498**, pos **0.038603**, so the anchor pins a live
  value rather than a structural zero. New anchor after this slice.
* **Perturbations**, each expected to be caught by a named test: swap the two electrodes'
  gaps; drop the negative's sign; clamp either side to [0, 1]; reduce the DFN's surface over
  x as an extreme rather than a mean; return `Some(0.0)` from the ECM arm.
* The page driven end to end under headless Chrome + CDP, per
  `protection-escalation.md`'s recipe, with the lesson's own instruction executed rather
  than reasoned about.

## Deferred, with a price

* **The x-spread** (`max − min` over x). It is what actually separates a DFN from an SPM —
  0.406 (neg) and 1.167 (pos) at the 3C cut-off against a structural zero — and it is the
  more dramatic number. Deferred because it is DFN-only, the only shipped DFN
  configuration is 1S1P, and it does **not** relax on a viewable timescale (the negative's
  is still 0.0589 after two hours where the gap is 0 by 600 s). It cannot carry the "rest
  and watch it converge" lesson this slice is built on. Its own slice.
* **The unclamped surface level** (1.244718 on a 3C overcharge). `SOC_CLAMPED_HIGH` already
  tells a reader the window was passed; the level would say how far. But the quantity a
  reader would actually act on there is the unclamped *bulk*, which is a different field
  and a different argument.

## Versions

| constant | before | after | why |
| --- | --- | --- | --- |
| `sim_core::SNAPSHOT_VERSION` | 13 | **13** | derived from stored state; nothing stored changes |
| `sim_server::API_VERSION` | 2 | **2** | added fields, explicitly exempt by its own rule |
| `sim_wasm::WASM_API_VERSION` | 4 | **5** | `web/pkg` is loaded separately from the JS that reads it |

## Exit criterion

`CellView` reports a surface-vs-bulk gap on both electrodes for both porous models and
`None` for the equivalent circuit; the pack grid can colour by it and hides it rather than
faking it on an ECM pack; and a reader who runs the new step sees a 3C SPM cell stop at
a positive-electrode gap the spike predicts at **0.372** and watches it reach **0.000**
while the voltage comes back — with both figures **replaced by the panel's own reading**
before this criterion is called met, and the whole path walked forward and back.

The two predicted numbers are the criterion's only unmeasured content, and they are marked
as such deliberately: `dfn-scenario.md` qualified an unmet clause on its own criterion
rather than two hundred lines above it, and this does the same for an unread one.
