# Protection chatter: a comparator that oscillated, and the band that is not the one you would size

The second of the two items `docs/plans/energy-hole.md` left open and priced. Not a
numbered phase and not a client slice: one commit against `bms.rs`, plus the
`SNAPSHOT_VERSION` bump it costs.

The defect, quoted from the document that measured it rather than paraphrased:

> **Protection chatters at the top of charge.** Pre-existing, measured at `1365808`, and
> now expensive: a 50 % duty cycle depositing 73.57 W. Hysteresis on the comparators is
> the obvious fix and is not free — it is state, so it is snapshot layout.

Everything below is measured on this box. Where a measurement contradicts the reasoning
that preceded it, the measurement is recorded as having won, because in this slice it
won twice.

## The cycle, reproduced before anything was changed

An out-of-tree probe at `M:\claud_projects\temp\chatter-probe`, path-depending on
`sim-core` so the half it shares with the engine *is* the engine, replicating
`scenario_runaway.rs`'s 3S3P LFP fixture and `scenario_protection.rs`'s 1S1P one. Printed
step by step, the shipped engine at `f1dac7f`:

```text
step 361 i= -0.0000 v_meas=3.6138 q= -0.0069 flags=(OV | OC | BALANCING)
step 362 i= -6.9104 v_meas=3.6680 q= 13.5297 flags=(SOC_CLAMPED_HIGH | OC | BALANCING)
step 363 i= -0.0000 v_meas=3.6188 q= -0.0065 flags=(OV | OC | BALANCING)
step 364 i= -6.9104 v_meas=3.6671 q= 73.6137 flags=(SOC_CLAMPED_HIGH | OC | BALANCING)
step 365 i= -0.0000 v_meas=3.6179 q= -0.0063 flags=(OV | OC | BALANCING)
step 366 i= -6.9104 v_meas=3.6663 q= 73.5965 flags=(SOC_CLAMPED_HIGH | OC | BALANCING)
```

73.6 W on every admitted step, against `v_max = 3.65`, and 2461 of 6000 steps admitted.
That is the plan's 73.57 W and its "50 % duty cycle" — the duty cycle is really 41 %,
because the tail is 141 admitting steps in 400 rather than 200.

**The 1S1P protection fixture did not chatter at all**, and chasing why found a second
defect. `scenario_protection.rs`'s over-voltage test started at `soc = 0.9`, where its
OCV is 3.52 against a `v_max` of 3.50 — so the rung tripped on step 0 and the protected
pack admitted current on **0 of 4000** steps. Every assertion in it was satisfied by a
pack that never charged: `saw_ov` on the first step, a mean tail current of exactly zero,
a final SOC of exactly the 0.9 it was handed, and a `bare_peak > protected_peak`
comparison against a pack that was never loaded.

It is fixed here rather than only noted, because it is one line: `initial_soc` drops to
0.7, below the 0.875 where that table's OCV crosses `v_max`, and the test gains a
**coverage assertion** — `charged_steps > steps / 10` — so it cannot go vacuous again.
That is the same instrument the previous commit needed for its clamp-driven properties,
and the second time in two commits it has been the thing that turned a green test into a
test.

**Transferable: a protection threshold outside the pack's own rested voltage range is
decorative** — the fixture's own comment says exactly that about `v_max` sitting above
the OCV curve, and the fixture then violated it from the other side by starting the pack
above the threshold. The band-sizing rule below is the same fact in a third form.

## The band, and the quantity it is not

The obvious thing for a release band to clear is the **load line** — the difference
between a loaded and an unloaded reading, `i·(R0 + Σ R_rc)` per cell. Measured on the
LFP fixture at its derated 2.30 A per cell: **39.9 mV**. That reasoning produced a first
draft with a 50 mV default, and it is wrong.

Sweeping the band over the same 6000-step run:

| `v_release_band_v` | steps admitting current | peak T | rung reached |
| --- | --- | --- | --- |
| 0 (bare comparator) | 2461 / 6000 | 333.18 K | `OT` |
| 0.02 | 2461 | 333.18 K | `OT` |
| 0.04 | 2454 | 333.20 K | `OT` |
| **0.05** | **1853** | 329.26 K | — |
| 0.051 | 1670 | 326.07 K | — |
| 0.06 | 625 | 301.65 K | — |
| **0.08 (shipped)** | **447** | **298.72 K** | — |
| 0.10 | 450 | 298.72 K | — |
| 0.15 | 443 | 298.72 K | — |

A band of 40 mV — *the same order as the load line* — changes 2461 to 2454, which is
nothing. The cliff is between 0.04 and 0.06, and it is at `v_max − OCV(1.0)` =
`3.65 − 3.60` = **50 mV exactly**.

The reason is that the rung releases on the **rested** reading, and a saturated cell
rests at its own `OCV(1.0)`. While the band is inside that gap the reading drops through
the release threshold every single time the load comes off, no matter how large the band
is; past it, the rung never releases again. So the quantity to clear is the distance
between the protection threshold and the cell's own full-charge open-circuit voltage —
**a property of the chemistry file, not of the operating current**.

80 mV ships: it is where the run saturates (447 steps, and 450 at 100 mV and 443 at
150 mV are the same answer), 60 % clear of the shipped LFP's 50 mV gap. It is a default
*sized against* a shipped file rather than derived from one, and a chemistry whose
`v_max` sits further above its `OCV(1.0)` will chatter until it is given a larger band.
That is said in the field's own doc comment rather than only here.

The temperature band is **2 K and labelled a placeholder**, in the sense `CLAUDE.md`
allows. There is no measured chatter to size it against and there cannot be: a pack at
its temperature limit is on a thermal time constant of minutes, so that rung does not
oscillate at the step rate however it is written.

## What it costs

The band is also how much charge the pack declines to take, and that is not free:

* On the runaway fixture, final SOC 0.999846 (band 0) to 0.999315 (band 0.08).
* On `cc_cv_charge_pack.toml`, the guided path's protection lesson, **95.1 % to 94.9 %**
  — see below.

## The design

`rung(held, trip, release) = trip || (held && !release)`, one call per soft rung, with
the four held bits carried on `Bms` and the two bands on `ProtectionConfig`.

Two properties of that expression are load-bearing rather than incidental:

* **A zero band is the old comparator, exactly.** `trip` and `release` are mutually
  exclusive for any band `>= 0`, so at zero `!release` *is* `trip` and the whole
  expression collapses to `trip`. Not approximately — identically, which is what makes
  the claim testable (it is, below).
* **It is idempotent**, which it has to be: the pack's nonlinear solve calls
  `apply_protection` once per pass, and a rung's conditions read the sensor frame, which
  no pass mutates. Re-deciding a rung reaches the same verdict.

The hard rungs are untouched. They already latch the contactor until
`Pack::clear_bms_fault`, and a band on top of a latch is nothing.

## The four exit criteria, and the two instruments that had to change

**1. The cycle is gone, and the fixture proves it was there.** `protection_hysteresis.rs`
runs the same pack twice with one number different: the zero-band arm still admits
current on the tail steps, the shipped-band arm admits zero. The assertion is on the
*count over a window*, never on a single step — bang-bang control is defined by
alternating, so any single-step assertion is satisfied by whichever phase it lands on.
That is the same trap that made a 20-step sampling stride read "heat with no source" one
commit ago.

**2. The sizing rule is asserted, not just documented.** The fixture places both voltage
limits exactly 50 mV outside the rested OCV range, and the test asserts a 40 mV band
still chatters while a 60 mV band does not. The load-line swing on that fixture is
60 mV — so if the swing were the deciding quantity, the 40 mV arm would be the one that
worked. The mistake this design nearly shipped is pinned by a test that fails if it is
made.

**3. Nothing that does not touch a rung moved.** The out-of-tree trajectory instrument at
`M:\claud_projects\temp\phase6-baseline`, run at a **zero band** against
`after-energy-hole.txt`: of 1579 lines, **12 differ and all 12 are `## final snapshot`
hashes**. Not one telemetry value moved. Two of those hashes also grew by 103 bytes, the
JSON for the new fields on the two cases that have a BMS at all; the other ten moved
because the version field did.

That is the cross-build evidence for the collapse-to-bare-comparator claim above, and it
is evidence no in-tree test could give: every bit comparison inside this repo is between
two runs of one build.

**4. The gate could not see the change, and now can.** At the *shipped* band the same
instrument was still byte-identical in telemetry — because **none of its twelve cases
drives a soft protection rung to its limit**. Two have a BMS; neither approaches `v_max`,
`v_min`, `t_max_k` or `t_charge_min_k`. A green sweep over that is the same hole slice D
found for the DFN and the energy hole found for the high clamp, one commit apart, and
the answer is the same: the instrument gains a case. `lfp_2s2p_overcharge_protected` is
the existing overcharge case with protection in front of it, and it discriminates —
**19 lines** differ between a zero band and the shipped one, all in that case.

## The perturbation table, and the test that was measuring `Clone`

`docs/plans/phase-7-dfn.md`'s rule — tabulate which tests actually catch the
perturbation rather than assuming the suite does — run over four perturbations with
`--no-fail-fast`:

| perturbation | caught by |
| --- | --- |
| `rung` ignores the band (the bare comparator is back) | `a_release_band_is_what_stops_the_top_of_charge_chattering`, `the_band_that_matters_is_the_gap_to_the_rested_voltage`, `every_soft_rung_carries_its_band`, `a_held_rung_survives_a_snapshot_round_trip`, `scenario_runaway::the_same_abuse_through_a_bms_never_gets_warm` |
| `rung` never releases | `a_release_band_is_what_stops_the_top_of_charge_chattering`, `the_band_that_matters_is_the_gap_to_the_rested_voltage` |
| the band is wired into `OV` only | `every_soft_rung_carries_its_band` |
| **the held rungs are not snapshot state** (`#[serde(skip)]`) | **nothing, first time round** |

The fourth is the one worth recording. `a_held_rung_survives_a_snapshot_round_trip` was
written specifically for it, and passed anyway — because **`Snapshot` holds a `Pack` by
value**, so `Pack::restore(&p.snapshot())` is a `Clone` and no `serde` attribute on any
field is exercised by it. The test asserted a property of `Clone` while claiming one
about serialization, and nothing but the perturbation said so. It now round-trips through
`bincode` for real, and the perturbation fails it.

Transferable, and it is the second commit running to pay a version of it: **a test whose
subject is serialization must serialize.** The generalisation from the previous commit —
*a conservation test that draws every term from the same reported quantities is checking
arithmetic, not physics* — has the same shape: the assertion was one level away from the
mechanism it named, and only forcing the mechanism to break revealed the gap.

`never-releases` is caught by two tests and not by `every_soft_rung_carries_its_band`,
which is correct rather than a hole: that test asserts rungs *hold*, and a rung that never
releases holds harder. What catches it is the zero-band **control** inside the chatter
test — "this arm is supposed to be chattering" — which is the second time in two commits
that a control, not an assertion, did the discriminating.

## The snapshot bump, whose usual argument does not apply

`SNAPSHOT_VERSION` 11 to 12, and it is a *semantic* bump: a v11 pack ran with a band of
zero, and the new default is not zero, so restoring one here would silently continue a
different trajectory. The held bits are the harmless half — they default to "nothing
held", which is exactly what a v11 pack was.

The **structural** argument that v10 and v11 both used does not extend to it, and
`snapshot_version.rs` says so rather than inheriting the sentence. Those bumps could
claim a stale blob stayed deserializable whatever it contained. This one cannot: `bincode`
writes struct fields positionally with no framing — the same fact the test relies on to
find the version tag — so `#[serde(default)]` buys nothing there, and a v11 blob carrying
`bms: Some(..)` is six values short of what v12 reads. That blob fails at
*deserialization*, which is the pre-v10 situation. It is only for `bms: None`, where none
of the new fields is emitted, that the bytes stay valid and the version field is what
refuses them — which is the fixture that file uses, and now the whole of what it proves.

Under the self-describing JSON the server uses, the defaults do apply and the wider claim
holds. Two formats, two answers, and the test pins the one it actually runs.

## Adapters: neither version constant moves, and this time that was the easy call

* **`sim_server::API_VERSION` stays at 2.** Its rule exempts additions, and two fields on
  `ProtectionConfig` are an addition. A v11 snapshot posted to a v12 server is now
  refused — but that is `SNAPSHOT_VERSION`'s job, and that constant's doc already says so
  ("Two numbers, two jobs").
* **`sim_wasm::WASM_API_VERSION` stays at 4.** Its rule is *scope* rather than
  additivity, and the two previous bumps both fired because the page **read** something
  new. Nothing here crosses that way: the page never constructs a `ProtectionConfig` —
  protection reaches it only inside scenario TOML that the server or the wasm parses —
  so there is no field for an old bundle to hand back as `undefined`.
* **`sim-godot` is unchanged**, on the same terms as last commit: its surface is a curated
  set of accessors and this adds no telemetry.

## The prose this commit falsified

Measured, not inferred, and the instrument for it was a **port of the page's own CC-CV
controller** into an out-of-tree probe — validated by reproducing the documented
pre-change numbers exactly with the band set back to zero.

* **`web/app.js`, the `what-protection-costs` step.** "`OV` at 3986 s and the charge stops
  at **95.1 %** … those 4.4 points are what protection costs" becomes **94.9 %** and
  **4.6 points**. The trip itself is unmoved at 3986 s and 94.879 %; what moved is where
  the run *ends*, because the client checks its completion test once per 10 s decision
  window, and the old chatter kept admitting current for two more windows after the trip
  — 4010 s and 95.143 % against 3990 s and 94.877 %.
* **`scenarios/cc_cv_charge_pack.toml`** and **`docs/plans/cc-cv.md`** carry the same two
  numbers; the scenario's comment is corrected in place and the plan is amended with a
  pointer here, in the house style the previous commit used.
* **Not falsified, and that was checked rather than assumed:** every *first-occurrence*
  number in that lesson — `BALANCING` 3111 s, `OV` 3986 s, `UT` 1494 s at 60.8 %,
  `PLATING_RISK` at the same instant, the 4.200 A derate — because a release band cannot
  move a first trip. Nor the step-before-the-trip reading (16.775 V, 11.0 mV of spread),
  which the change does not reach.
* **`scenario_runaway.rs` has moved twice now and its name has been wrong both times.**
  It settled 0.8 K above ambient originally; the energy hole took it to `t_max_k` and it
  was renamed to `..._is_stopped_at_its_temperature_limit`; it is back to **298.72 K,
  0.57 K above ambient**, stopped by `OV`, and the name is the original one again. The
  intermediate name was accurate about the engine and wrong about what the scenario was
  for — which is the argument for naming a test after the claim rather than the symptom.

  What it deliberately does **not** assert is `SOC_CLAMPED_HIGH`: whether the pack stops
  just short of the clamp or just past it moves with the band (absent at 0.08, present at
  0.10, absent at 0.15), so either claim would pin a knife edge.

## Still open

* **The low clamp still fabricates energy**, and the fix priced in
  `docs/plans/energy-hole.md` was measured in this slice and does not work. See
  `docs/plans/low-clamp-solve-side.md`.
* **Balancing has the same shape of comparator** — `bleed_conductances` closes the switch
  on `v > v_threshold_v` with no band, so a group near the threshold switches its bleed
  resistor on and off at the step rate. It is far cheaper than the protection cycle was
  (a 33 Ω bleed at 3.6 V is 0.4 W against 73.6 W) and it is out of scope here, but it is
  the same defect and it is now the only one left in this file.

  **Closed — see `docs/plans/balancing-chatter.md`.** `SNAPSHOT_VERSION` 12 → 13. The
  band it needed is sized by a **different rule than this document's**: not
  `v_max − OCV(1.0)` but the bleed's own load line `I_bleed · (R0 + Σ R_rc)`, because
  opening a bleed switch returns the reading to wherever the group sits rather than to a
  rested voltage the cell cannot exceed. That makes it scale with parallel count and with
  `R_bleed`, so unlike this one it is not a property of the chemistry file. The open
  question this file also left — whether `snapshot.rs`'s replay tests share the `Clone`
  blind spot — was answered there in passing: **they do not.**
