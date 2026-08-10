# Balancing chatter: the last bandless comparator, and a band sized by a different rule

`docs/plans/protection-chatter.md` closed with one item still open:

> **Balancing has the same shape of comparator** — `bleed_conductances` closes the switch
> on `v > v_threshold_v` with no band, so a group near the threshold switches its bleed
> resistor on and off at the step rate. It is far cheaper than the protection cycle was
> (a 33 Ω bleed at 3.6 V is 0.4 W against 73.6 W) and it is out of scope here, but it is
> the same defect and it is now the only one left in this file.

This is that slice. `SNAPSHOT_VERSION` **12 → 13**.

## What the chatter actually is, measured before anything was written

A bleed switch is unlike a protection rung in one way that turns out to decide the whole
design: **closing it is itself what re-opens it.** The bleed current flows through the
group's own resistance, so the sensed group voltage drops the moment the switch closes,
and a bare comparator sees a reading back under its threshold on the very next frame.

Two fixtures, each run over the same 6000 s of simulated time at two different `dt`s:

| case | `dt` = 1 s | `dt` = 0.1 s |
| --- | --- | --- |
| **park** — charged at 0.05 A against a 0.104 A bleed | 5999 flips / 6000 steps, 50.0 % duty | 59999 / 60000, 50.0 % duty |
| **drain**, charge phase | 21 flips | 191 flips |
| **drain**, rest phase | 427 flips | 4271 flips |

**A flip count that scales with the sampling rate is the definition of numerical
chatter.** Ten times the samples, ten times the flips, same physics — that ratio is the
instrument this whole slice is built on, and it is the one number that does not have to
be re-tuned when the shipped band changes. The `park` trace shows the cycle bare: the
frame reads `3.441027719` open, `3.438912287` closed, and back, forever.

`park` is not a contrived fixture. A charge current below the bleed current is the
ordinary end-of-CV condition — precisely when a passive balancer is doing its job.

## The sizing rule, which is *not* protection's

The band must clear the reading's response to the switch's own action. That sentence
covers both comparators, and it evaluates to a different quantity in each:

* **Protection**: `v_max − OCV(1.0)`. Tripping removes the *external* load entirely, so
  the reading falls back to a rested voltage the cell cannot exceed. A property of the
  chemistry file alone.
* **Balancing**: `I_bleed · (R0 + Σ R_rc)`, the bleed's **own load line**. Opening the
  switch returns the reading to wherever the group actually sits; there is no rested
  voltage pinning it.

And the *settled* load line, not the instantaneous `I_bleed · R0` — 4.2 mV rather than
2.1 mV on the 1P fixture — because hysteresis lengthens the closed dwell into exactly the
regime where the RC pairs finish developing. The fix moves the system into the regime
that needs the larger number.

### The sweep, which predicted the cliff rather than fitting it

Four `(parallel, R_bleed)` combinations on one unchanged chemistry, band swept 0–80 mV,
reading the `dt` = 1 s vs `dt` = 0.1 s flip ratio:

| parallel | `R_bleed` | predicted `I_bleed · (R0 + Σ R_rc)` | measured cliff |
| --- | --- | --- | --- |
| 1 | 33 Ω | 3.1 mV | between 2 and 4 mV |
| 4 | 33 Ω | 0.78 mV | between 0 and 2 mV |
| 10 | 33 Ω | 0.31 mV | between 0 and 2 mV |
| 1 | 5 Ω | 20.6 mV | between 10 and 20 mV |

The ratio reads **10.00 while chattering and exactly 1.00 once the band clears the load
line**, on every row. The prediction was written down before the sweep ran and the sweep
did not adjust it.

Above the cliff the residual 1–3 flips are **not** a failure. With a band the switch has
to drain the group until OCV carries the reading through `threshold − band`, then the
charger carries it back: a relaxation oscillation whose period is charge transfer, ~2800 s
at 10 mV on the park fixture, matching the traversal time of 10 mV at the net 0.054 A
drain. Nothing in this slice asserts `flips == 0`, and an assertion that did would only be
satisfiable by oversizing the band.

### Two consequences protection's band does not have

* **It cannot be derived in code.** The BMS knows `R_bleed` and the measured voltage, so
  it knows `I_bleed` — but the group resistance is ground truth, and the BMS never reads
  ground truth (`CLAUDE.md` principle 8). A configured voltage is the only shape
  available, and the reason is a design principle rather than a precedent.
* **It is not a property of the chemistry alone.** The load line scales as `1/parallel`
  and with `1/R_bleed`, and `soh_resistance` grows it over pack life, so a margin measured
  on new 1P cells shrinks with both age and parallel count.

Shipped default **10 mV**: 3.2× the 1P/33 Ω fixture's load line. The last sweep row is the
honest caveat — a 5 Ω bleed on a 1P string needs ≥ 20 mV and will chatter until it gets
it. Sized against a fixture, not derived from one, and labelled as such.

## Where the latch lives, and the property that decided it

`Bms::bleed_conductances` is documented as a **pure read**, safe to call on a `dt == 0`
probe step, and `Pack::step` calls it outside the `sensor_tick` gate for exactly that
reason. So the decision could not go there. It lives in a new `update_bleed_latches()`,
called once per sampled frame from `Pack::step` immediately after `corrupt_sensors`, with
`bleed_conductances` reading latch state and nothing else.

Three things fall out, and each is pinned by a test:

* **A probe step moves no switch**, and still reports the bleed the next real step will
  carry — stronger than "leaves the engine unchanged", and the reason the decision is not
  in the read.
* **The pack's nonlinear iteration cannot advance it.** Deciding inside
  `bleed_conductances` would have made the switch depend on how many solve passes a step
  happened to take.
* **The balancer switches on corrupted sensor readings.** A `+120 mV` offset on one
  group's voltage sensor makes a pack resting 60 mV *below* its threshold bleed anyway.
  That is principle 8 applied to balancing, and it is what the ordering buys.

### The seed, which is what keeps the zero-band control exact

`Bms::new` seeds the latch from the initial open-circuit frame rather than starting it
empty. Without that, a pack built above its threshold would delay its first bleed by one
step — and that one step is the entire difference between a band of zero and the bare
comparator this replaced. With it, **a band of `0.0` reproduces the old trajectory
bit-for-bit**, which is what makes every other measurement here a controlled one.

Confirmed against the pre-change build before any of the tests were written: at band 0 the
drain fixture gives 21 / 427 / 191 / 4271 flips and 140.787 / 461.174 / 140.969 / 460.991 J,
matching the pre-change run in every digit.

## The perturbation table

`docs/plans/phase-7-dfn.md`'s rule, run over five perturbations with `--no-fail-fast`:

| perturbation | caught by |
| --- | --- |
| the band is ignored (bare comparator is back) | `a_release_band_is_what_stops_the_bleed_switch_chattering`, `the_band_that_matters_is_the_bleeds_own_load_line` |
| the latch never releases | the two above, plus `a_zero_band_is_the_bare_comparator_step_for_step` |
| the latch is not snapshot state (`#[serde(skip)]`) | `a_held_bleed_switch_survives_a_snapshot_round_trip`, **and** `snapshot.rs::snapshot_restore_replay_is_bit_identical` |
| **the `Bms::new` seed is dropped** | **nothing in this file, first time round** — only `properties.rs::electrical_and_heat_energy_balance`, incidentally |
| **the latch is decided *before* `corrupt_sensors`** | **nothing at all, first time round** |

The last two are the ones worth recording, and both produced a new test.

The seed hole is the sharper of the two: the whole point of the seed is what happens on
step 1, and six tests written specifically about the band all started their measurement
later than that. `a_pack_built_above_the_threshold_bleeds_on_its_first_step` states it
directly — and immediately caught a fixture error, because the parked fixture rests at
`OCV(0.90)` = **exactly** `V_BLEED` on that OCV table and the comparator is a strict `>`.
It needed a pack that rests strictly above, not merely at, its threshold.

The ordering hole had no test because nothing in the suite drove the balancer through a
sensor fault at all. `a_lying_voltage_sensor_drives_the_balancer` does, with a control
arm that must never bleed.

**One question the table answered for free**: the protection slice left open whether
`snapshot.rs`'s replay tests share the `Clone` blind spot that let a `#[serde(skip)]`
field pass a round-trip test. They do not — `snapshot_restore_replay_is_bit_identical`
catches perturbation 3.

## The snapshot bump, re-argued rather than inherited

`SNAPSHOT_VERSION` 12 → 13, and **semantic**: a v12 pack ran its bleed switches off a bare
comparator, the new band does not default to zero, so a restored v12 pack would continue a
different trajectory.

The held bools default to the correct v12 reading ("nothing held") — but only nominally,
because `Bms::new` seeds them. "Nothing held" is right for a v12 blob and wrong for a pack
built fresh above its threshold, which is a second reason they cannot carry the bump.

The **structural** argument lands where v12's did, and the arithmetic is different so it is
derived rather than copied: `bincode` is positional with no framing, so a v12 blob carrying
`bms: Some(..)` is short by a scalar (`BalancingConfig`'s new `f64`) and a sequence (`Bms`'s
`Vec<bool>`), and fails at *deserialization* — the version check never sees it. Only for
`bms: None`, where neither field is emitted, do the bytes stay valid and the version field
decide. That is what `snapshot_version.rs`'s BMS-less fixture pins, and its pair moved
12 → 13. `#[serde(default)]` on the two new fields buys TOML scenario back-compat and the
self-describing JSON path, and buys `bincode` nothing.

## The instrument, and the hole it had for the fourth commit running

`M:\claud_projects\temp\phase6-baseline`, run at a **zero band** against
`after-chatter.txt`: of 1626 lines, **26 differ and every one is a `## final snapshot`
hash**. Not one telemetry value moved — the cross-build evidence for the
collapse-to-bare-comparator claim, which no in-tree comparison can give.

Three of the 26 also grew, and the growth reconciles exactly (the dumps are JSON): +62 and
+56 on the two cases with a balancer (`,"v_release_band_v":0.0` plus `,"bleed_held":[...]`
at 4 and 3 groups) and +16 on the BMS-with-no-balancer case (`,"bleed_held":[]`, no band
field, because `balancing` is `None`). The other ten moved on the version field alone.

At the **shipped** band the 13-case dump moved by four telemetry lines, all in a `dt=60`
coarse leg — the band moving the switch-*open* point, with **nothing in the step-rate limit
cycle the band exists to remove.** Fourth instrument in four commits with this hole (slice
D had no DFN case, the energy hole had no ECM-clamps-high case, the protection chatter had
no soft-rung case). `nmc_2s1p_parked_on_bleed_threshold` closes it — two legs at `dt` = 1 s
and `dt` = 0.25 s so the dump itself carries the coarse/fine pair — and discriminates hard:
**410 lines differ, 398 in that case**.

Its threshold needed two attempts, and the first is the lesson: at 4.12 V the pack starts
4.6 mV above and drifts at ~4 µV/s, so it would not have reached its own threshold for
~1150 s against a 700 s run. **A bleed threshold outside the range the pack actually visits
is decorative** — the same fact that made `scenario_protection.rs`'s over-voltage fixture
vacuous one commit ago, approached from the other side.

Anchor is now `after-bleed-band.txt`, 14 cases.

## Prose this commit falsified, found by measuring rather than reasoning

`scenarios/cc_cv_charge_pack.toml`'s header and `docs/plans/cc-cv.md` both quote the step
before the over-voltage trip. That pack's bleed threshold is 4.10 V/group and the charge
drives straight through it, so the band was in a position to move those numbers — and it
moved two of them.

Re-measured with a port of the page's own CC-CV controller, validated the way the
protection slice's was: it reproduces **every** documented number exactly with the bleed
band set back to zero (3111 s, 3986 s, 3990 s, 94.9 %, 24.8 mV, 4.18901/4.20004, 11.0 mV).

| quantity | band 0 | shipped 10 mV |
| --- | --- | --- |
| `BALANCING` first | 3111 s | 3111 s |
| `OV` first | 3986 s | 3986 s |
| ends / SOC | 3990 s, 94.9 % | 3990 s, 94.9 % |
| short of 16.80 V | 24.8 mV | **24.9 mV** |
| cells bottom..top | 4.18901..4.20004 | **4.18899..4.20001** |
| spread | 11.0 mV | 11.0 mV |
| **bleed switch flips** | **21** | **1** |

Corrected in both files. Note what this says about the previous amendment in
`cc-cv.md`, which asserted that the step-before-the-trip reading "is a step the change does
not reach": true of protection hysteresis, false of this one. **An exemption established
for one change is not inherited by the next**, which is why it was re-run rather than
re-read.

The page's own prose survives unedited — it rounds to "25 mV" and "11 mV" and says the top
cell "has already crossed 4.20", all still true. But 4.20001 clears 4.20 by 0.01 mV where
it used to clear it by 0.04, so that sentence is now thin enough to re-read rather than
assume next time.

## Adapter versions: checked individually, neither moved

Fourth time these have been checked one at a time rather than bumped as a pair.

* `sim_server::API_VERSION` stays **2**. Its rule exempts added fields, and nothing was
  renamed.
* `sim_wasm::WASM_API_VERSION` stays **4**. Its rule is the contract's *scope* — method
  names, and fields the page actually reads. No method changed, and the page never
  constructs a `BalancingConfig` (it comes from server-side scenario TOML) nor reads any
  new field; it already read `q_balancing_w`. Same reasoning as the protection slice, which
  also changed physics and `SNAPSHOT_VERSION` without moving this.
* `sim-godot` gets nothing: its telemetry surface is curated and exposes no balancing
  config at all, so an accessor here would repeat `docs/plans/ui-pedagogy.md`'s
  zero-caller mistake.

## Not done here

* **`bleed_r_ohms` has no lower bound relative to the band.** A stiff bleed resistor on a
  1P string chatters at the shipped default, and nothing warns. Validating it would need
  the group resistance, which is ground truth — so the honest form is a doc note (shipped)
  rather than a config check. Left as the doc note.
* **The over-discharge energy hole** (`docs/plans/energy-hole.md`) and the low-clamp
  solve-side fix (`docs/plans/low-clamp-solve-side.md`, measured and declined) are both
  still open and untouched by this.
