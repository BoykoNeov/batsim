# Charge acceptance: the third mechanism, and what a dome costs the signal

`docs/plans/phase-8-slice-c-spike.md` measured three things about the top of a NiMH charge
and named the fix for one of them as a mechanism the phase then cut twice: the slope of
the terminal voltage reverses 29-fold in a single 0.1 s step, because a hard SOC clamp takes
charge acceptance from 100 % to 0 % in one step, and *"no parameter fixes that; it needs a
third mechanism."* Slices C and D wrote the −ΔV lesson about the number rather than the
shape, and the spike's last section says what reopening it is: a slice against the phase
plan's stopping rule, not a reopening of Phase 8. This is that slice.

It is also the first engine slice after this repository gained a roadmap
(`docs/ROADMAP.md`), and it was chosen from that list because it is the one gap the notes
had named, priced and left with a recipe.

## What was open, stated as physics

On a nickel positive electrode the charging reaction competes with oxygen evolution, and the
closer the electrode is to full the larger the share oxygen takes. In a sealed cell that
oxygen recombines at the negative electrode, so the current it carried comes out as heat
and not as stored charge. That heat is what warms a full cell on a charger, and the warming
is what a −ΔV charger detects. The engine had the heat — `energy-hole.md` bills refused
charge at the cell's open-circuit voltage — but only *after* a clamp, all at once. Every
application note draws the acceptance curve as a shoulder; the engine drew a cliff.

## The design

One optional section, chemistry-as-data, ECM-only, no new per-cell state:

```toml
[charge_acceptance]
soc_onset = 0.985
```

Above the onset the accepted share of a charging current is the linear taper
`η = (1 − soc) / (1 − soc_onset)`, so `d(soc)/dt = j·η` for a charger offering `j`
capacity-fractions per second, and that is a first-order linear ODE in `1 − soc` with a
closed form. `coulomb_step_tapered` integrates it **exactly** over a step — splitting the
step at the onset when a cell crosses it — on the same reasoning the RC pairs use their
exponential update: unconditionally stable at any `dt`, step-size invariant to rounding,
and serving the real-time page and a fast-forward with one code path. The cell approaches
full as an asymptote and never enters the hard clamp on a charge.

Four choices inside that, each with a reason:

* **Linear, one parameter.** A power law has a closed form too, and a sigmoid does not; a
  second shape parameter would be a second placeholder with no source, on a file whose
  onset is already one. Real acceptance also falls earlier at higher current and when hot,
  and one number cannot say so — the section's doc says what it claims and at what rate.
* **The refused charge takes the clamp's path, unchanged.** `CoulombStep::rejected_as`
  carries it, `Telemetry::i_rejected_a` reports it, the pack bills it as heat at the
  cell's own `OCV(soc)` through the same `rejection_ocv_v` call, and `SOC_CLAMPED_HIGH` is
  raised on every step that refuses any. Nothing in the pack changed; the ledger that
  closed the energy hole closes here too. The flag's doc gains the second cause.
* **A `match` with the ordinary count in its other arm**, never a taper through a neutral
  onset, so a chemistry without the section executes not one line of it — the argument
  every optional section here makes, and what makes "no other file moves" structural.
* **Booked in amp-seconds, not capacity fractions.** The first draft computed the refusal
  as `(offered − stored)·capacity` and a cell storing nothing reported `−3.0000000000000027 A`
  against a `−3 A` charger; the round trip through `j` cost a few ULPs. Booking
  `offered_as = −i·dt` directly makes the full-cell case exact to the bit, which the test
  `a_full_cell_refuses_everything_from_the_first_step` then found the *pack solve* rounding
  by the same amount for its own reasons.

`SNAPSHOT_VERSION` 20 → **21**, for the chemistry's layout alone: the section is one
`Option<f64>` between `hysteresis` and `aging`, and the chemistry is serialized inside the
snapshot. The stale-blob hazard is the quiet one and, for the first time in
`snapshot_version.rs`, value-independent — any eight bytes are a valid `f64`, so a v20
chemistry with `[aging]` parses its `cal_pre_exp` into the onset and carries on displaced.
`a_v20_shaped_chemistry_tail_misparses_at_v21` pins that; the version check is what stands
in front of it.

## Predictions, registered before the NiMH file was edited

The engine slice was built and measured on a synthetic fixture first (flat resistance, a
straight OCV, an onset at 0.90). These were written down before the shipped NiMH file was
given the section, with the onset candidate **0.85** — the middle of the 80–90 % range
handbook curves draw the shoulder over:

* **P1.** No golden and no shipped scenario except the two `nimh_overcharge` files moves;
  the two `nimh_memory` scenarios stay below the onset and are bit-identical.
* **P2.** The peak terminal voltage is *below* the clamped 1.518 V, and the slope reversal
  across it drops from 29× per step to under 2×.
* **P3.** The fall at +10 K stays inside the 5–10 mV window but is *smaller* than 8.772 mV,
  because the OCV is still rising through the taper while temperature pulls the terminal
  down.
* **P4.** The +10 K point arrives later after the peak than 136.5 s but inside 600 s.
* **P5.** At the old mark (3240.5 s) the clamp row reads less than 3.000 A refused and
  `soc (true)` reads below 100.0 %.
* **P6.** The isothermal control's fall is still exactly zero and its trace is monotone
  non-decreasing to the end.
* **P7.** `a_charging_cell_cools_before_it_warms` still passes.
* **P8.** `past_the_peak_nothing_turns_round` still passes.

# Measured

## The first run falsified three of them, and the reason is the finding

At onset 0.85 the shipped file **does not peak inside the lesson's window at all**. The
terminal is still rising at 4500 s, at 1.4677 V and +65 K, and the peak comes at 4712 s in
a 20 000 s run. P2's first half held (lower) and its second half was unmeasurable; P3 and
P7 failed outright; P4 held vacuously.

The mechanism is not the taper's; it is the file's. `[ocv]` for this cell rises **50 mV
over its last 3 %** — a knee placed by hand, which under a clamp was reached at the clamp
instant and then held there. Under an asymptotic approach that knee is spread into a slow
creep, `d(OCV)/dt ∝ (1 − soc)·slope`, and the creep is the same order as the temperature
fall it is now fighting. A wider taper spreads it further. So the fall at the +10 K instant
a charger reads, swept over the onset at the page's `dt = 0.5`:

| onset | peak | at | +10 K after | fall at +10 K |
| ----- | ---- | -- | ----------- | ------------- |
| none (clamp) | 1.517832 V | 3240.0 s | 136.5 s | **8.806 mV** |
| 0.85 | 1.468095 V | 4712.5 s | 340.0 s | 0.686 mV |
| 0.90 | 1.477653 V | 3931.5 s | 223.0 s | 1.199 mV |
| 0.95 | 1.494333 V | 3458.5 s | 173.0 s | 1.462 mV |
| 0.975 | 1.505699 V | 3342.5 s | 156.0 s | 4.728 mV |
| 0.98 | 1.508064 V | 3321.0 s | 152.5 s | 5.274 mV |
| **0.985** | **1.510460 V** | **3300.0 s** | **148.5 s** | **5.933 mV** |
| 0.99 | 1.512886 V | 3279.5 s | 144.5 s | 6.762 mV |

**The knee and the taper are not independent parameters of this file.** The hand-placed
knee stood in for the oxygen-evolution overpotential a real cell shows at the top of
charge, and a taper wider than the knee's last segment erases the signal the file exists
to produce. That bounds the onset from below by measurement: 0.975 is outside the
charger's window, 0.98 is inside by 0.27 mV, 0.985 by 0.93 mV.

It is bounded from above by the thing the section exists to remove. The instrument is the
largest one-step change in `dV/dt` within two minutes of the peak — a curvature, so a
smooth dome scores near zero and a corner scores its whole slope reversal — read at the
page's own `dt = 0.5`:

| | worst kink near the peak |
| --- | --- |
| clamp | 0.532 mV/s per step |
| 0.985 | 0.005 mV/s per step |

A hundredfold rounder, and the refused share ramps over a `(1 − 0.985)·3600 s = 54 s` time
constant at 1 C, which is the "tens of seconds" the spike said a real peak is rounded over.
The shipped value is **0.985**, and its provenance note carries both bounds and says it is
a placeholder between them.

## The predictions, scored at the shipped onset

| | outcome |
| --- | ------- |
| **P1** | **HELD.** Every golden unmoved; the memory pair unmoved (its charged arm ends at 50 %); the full workspace green at 643 tests with no tolerance touched. |
| **P2** | **HELD in substance, and the registered instrument was the wrong one.** 1.510 V against 1.518. The "slope ratio across the peak" is noise on a dome — both one-step slopes are a fraction of a microvolt — so the kink measure above replaced it. |
| **P3** | **FALSIFIED as registered, held after the onset moved.** At 0.85 the fall at +10 K is 0.69 mV. At 0.985 it is 5.93 mV, inside the window and smaller than 8.77, for the reason the prediction gave. The prediction was silent on the onset, and the onset was the whole story. |
| **P4** | **HELD.** 148.5 s after the peak, against 136.5 before. |
| **P5** | **HELD.** At 3240.5 s: `refused 1.901 A`, `soc (true)` 99.45 %. |
| **P6** | **HELD, and it is sharper than written.** The pinned cell never turns over at all; its peak is its last sample, at every run length tried. `with_temperature_pinned_the_fall_is_exactly_zero` now pins the whole trace monotone non-decreasing. |
| **P7** | **FALSIFIED as written.** The test ran to 3240 s and asserted the cell had barely warmed by then; with heating starting at the onset (3186.5 s) it had warmed 13.7 K at 0.85. The test was re-scoped to *the charge up to the onset*, which is the claim it was making, and passes there (+0.86 K). |
| **P8** | **HELD.** Monotone fall, monotone warming, ends at 155 °C. |

Two of the three falsifications were the same fact seen twice, and the test that caught P7
was the right one to have: a claim about "the charge" that had quietly meant "the first
3240 s".

## What the lesson now says, and what it no longer can

Steps 27 and 28 of the guided path were re-measured and rewritten; their sixteen claims in
`web/path-claims.toml` were re-derived and `every_claim_matches_the_engine` is green. At
the page's `dt = 0.5`:

| | clamp (before) | taper (now) |
| --- | --- | --- |
| step 27's mark | 3240.5 s, one step past the clamp | **3300.0 s, the peak sample itself** |
| `terminal` | 1.518 V | 1.510 V |
| `soc (true)` | 100.0 % | 99.8 % |
| `clamp` | refused 3.000 A | refused 2.635 A |
| `heat` | 4.24 W | 3.70 W |
| `cell t` | 25.9 °C | 30.8 °C |
| step 28's mark (+10 K) | 3376.5 s | 3448.5 s |
| `terminal` | 1.509 V (9 mV below) | 1.505 V (5 mV below) |
| isothermal twin there | 1.519 V, flat since the clamp | 1.518 V, still creeping |

Three things the prose had to stop saying: that the mark is "the instant the cell fills"
(there is no such instant now — the peak is the instant a charger can see); that `clamp`
and `soc (true)` "did not move" between the marks (both creep, and neither pulls a voltage
down, which the sentence now argues); and that the pinned twin "has been reading that since
the cell filled" (it never stops climbing, and the isothermal arm now has to stop exactly
at the mark, because fifty seconds later the row prints `1.519 V`). One thing it gained:
the mark on step 27 is the peak *sample*, which a clamp could never offer — a corner's
maximum sits one step before the state a reader wants to see.

The scenario files' comments were re-measured with the rest. The 40 %/60 % split between
the ohmic and OCV temperature channels quoted there survives (the OCV term is 3.5 mV of the
5.93), with one change of meaning: the "by difference" ohmic share now carries the storage
creep, which lands against the fall, so it is the ohmic channel *net* of that creep.

## The perturbation table

Five deliberate breaks, each in one place, with a green baseline before and after. What is
recorded is *which* tests reddened, over `sim-core`'s `charge_acceptance`, `properties`,
`energy_hole` and `snapshot_version` files and `sim-data`'s `nimh_chemistry`,
`path_claims`, `scenario` and `load`.

| # | broken | reddened |
| - | ------ | -------- |
| A | explicit Euler in place of the closed form (`u·(1 − k·τ)` for `u·e^(−k·τ)`) | `the_exact_update_is_step_size_invariant`, `refusal_ramps_instead_of_switching` — and `every_claim_matches_the_engine`, because the lesson's numbers move |
| B | the flag not raised on a taper refusal | `a_full_cell_refuses_everything_from_the_first_step`, `refusal_ramps_instead_of_switching`; in `sim-data`, `a_charging_cell_cools_before_it_warms` and `a_full_cell_falls_through_the_charger_termination_window`, both of which locate the onset by the flag |
| C | the charging guard dropped, so discharge tapers too | `discharge_above_the_onset_is_untouched`; `resting_voltage_remembers_the_drive_direction` (the discharged arm of the memory pair no longer lands on 0.5) and `every_claim_matches_the_engine` |
| D | refused charge stored in the count but never reported | four in `charge_acceptance.rs` (conservation, the declared share, the ramp, the full cell); **six** in `sim-data`, including `past_the_peak_nothing_turns_round` and the dome test — with no refusal there is no heat, and with no heat the nickel cell never peaks |
| E | the NiMH file loses its section (the control arm) | `the_peak_is_a_dome_and_the_clamp_it_replaced_is_a_corner`, `a_charging_cell_cools_before_it_warms` (its run now ends at the clamp, 13.7 K warm), `every_claim_matches_the_engine` |

Case D is the one worth reading twice: it is the taper *without its heat*, and it is
caught six ways in the shipped file's tests because the −ΔV depends on the heat and not on
the taper. Case E is the proof the dome test has a real control: it goes red on the file
this repository shipped yesterday.

## Deliberately not done

* **No rate or temperature dependence in the taper.** Oxygen evolution is a kinetic
  competition and a real acceptance curve falls earlier at higher current and when hot.
  That is two coefficients with no shipped source, and `docs/ROADMAP.md` H4 names the
  honest form: an explicit oxygen-evolution side reaction that replaces both the taper and
  the hand-placed knee it competes with.
* **The knee itself is untouched.** It could be lowered to give the taper room, and that
  would be fitting the file to the lesson. The coupling is recorded in the file's own
  provenance instead.
* **Not applied to the porous models.** A particle never rejects lithium; the section is
  read by nothing on their path, on the same terms as `[diffusion]` and `[hysteresis]`.
* **No NiCd.** It is "nearly free" now, which `phase-8-chemistries.md` says is exactly why
  it is named and not scheduled.
* **`docs/plans/phase-8-slice-d-nimh-client.md` is not rewritten.** Its numbers describe
  the clamped cell and are kept as the record of it, with one struck-through bullet
  pointing here.

## Versions

* `SNAPSHOT_VERSION` 20 → **21**; the pair test moved to v20 → v21 and gained the quiet
  value-independent shaped-tail case.
* `API_VERSION` and `WASM_API_VERSION` unmoved: no call signature changes, no telemetry
  field moves; `i_rejected_a` and `SOC_CLAMPED_HIGH` gain a second cause on one chemistry,
  which both were named to allow.
