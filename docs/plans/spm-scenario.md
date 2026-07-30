# The SPM scenario: a pulse train, and the two non-linearities that point opposite ways

Item 2 of the queue `docs/plans/cc-cv.md` left open, and the oldest open item in the
client's queue — it was named and priced in `docs/plans/scenario-catalog.md`, deferred
there for want of a demand program, and deferred again in `cc-cv.md` with the note that
CC-CV *is* that machinery so the price had come down. Not a numbered phase: Phase 6 built
the single-particle model, and this is the fifth slice of the client catching up to the
engine.

## What is missing, stated as source

`crates/sim-core/src/spm.rs` is 900 lines of porous-electrode physics, validated against
PyBaMM to 2.6 / 6.7 / 1.9 mV over three scenarios, and **reachable from no client**.
`PackConfig::cell_model` is settable only from a scenario file, and not one of the eight
files in `scenarios/` sets it. `scenarios/cc_discharge_lgm50.toml` says so in its own
comment, and says why it does not:

> It does not select the SPM cell model, which is reachable from no client and is a slice
> of its own. […] The comparison that would pay is a high C-rate, where diffusion limits
> the particle rather than the circuit, and that needs a demand program rather than an
> initial condition.

That program now exists in outline — `advance()` learned to chop a frame at
simulation-time boundaries for CC-CV — so the comment's premise is spent, and this slice
must edit it rather than leave it asserting something false.

## The measurement that came before the design

A throwaway harness against `sim-core` in the temp tree, not committed, driving
`nmc_21700_lgm50` through both cell models at the client's own `dt = 0.5 s`. Recorded
first, and it is the reason the design below is not the design that was drafted. **Three
candidate experiments died here**, which is the argument for measuring before writing
rather than after.

### 1. The CC-CV charge comparison is dominated by a placeholder

The obvious first thought — charge the same cell under both models and watch the SPM's
knee arrive early as the particle surface saturates — is **backwards**. At 2 C from 20 %
SOC the *ECM* reaches 4.20 V first:

| model | knee | SOC at the knee | done | SOC at the end |
|---|---|---|---|---|
| Ecm | 550 s | 0.5058 | 3527 s | 0.9936 |
| Spm/20 | 750 s | 0.6169 | 3831 s | 0.9852 |

`nmc_21700_lgm50.toml`'s `[r0]` is a **labelled placeholder** at ~25 mΩ, which is 258 mV
at 10.3 A. Its `[spm]` section's `contact_resistance_ohm = 0` — Chen2020's own value —
so the particle model has no ohmic term at all. The gap is the file's own provenance
split, not physics, and a lesson built on it would be teaching a placeholder.

### 2. At 0.5 C the two models differ by less than the ECM's own table error

Through the middle of the same charge the difference oscillates and **changes sign** —
−21.6, +1.8, −8.0, +9.2, −24.3 mV. The `[ocv]` table's documented maximum
piecewise-linear interpolation error is 13.91 mV. There is nothing to see here that is
not the grid, which is what the deferral note said and is now confirmed in the shape the
reader would drive it.

### 3. An isolated pulse sweep measures a pack no reader ever sees

The first sweep that looked decisive — voltage drop after a 60 s pulse, at 0.5/1/2/3 C —
was measured from a **freshly built, fully rested pack** at each SOC. A reader drives a
*train* on a *draining* pack, where every pulse after the first starts from an
incompletely relaxed state, and the SPM is measurably still relaxing 1800 s later. Same
error as the aging slice's 3.6× rate ratio: right about a pair nobody looks at. Every
number below is therefore measured **as the train the lesson drives**.

### 4. What survives: one pulse, decomposed, at two currents

The experiment the slice ships. 1S1P, 90 % SOC, 25 °C, isothermal, no BMS. 60 s on,
600 s off. The rest is long deliberately: it is what separates the part of the response
that vanishes with the current from the part that has to diffuse away.

Pulse 1, in volts:

| | Ecm 1 C | Ecm 3 C | ratio | Spm/20 1 C | Spm/20 3 C | ratio |
|---|---|---|---|---|---|---|
| **jump** when the current stops | 0.1328 | 0.3973 | **×2.99** | 0.1139 | 0.2132 | **×1.87** |
| **climb** over the next 600 s | 0.0748 | 0.2243 | **×3.00** | 0.0173 | 0.1039 | **×6.01** |
| **total sag** (what the plot shows) | 0.2128 | 0.6374 | **×3.00** | 0.1357 | 0.3370 | **×2.48** |

The `jump` is taken with a `dt = 0` probe under `Rest`: no time passes, so what
disappears is exactly what is zero at zero current — the ohmic drop, and on the particle
the kinetic overpotential too. What is left to relax is the concentration gradient, and
that is the `climb`.

**Ask a circuit for three times the current and every part of its answer is three times
bigger.** That is not a property of these coefficients; it is what "linear" means, and no
choice of `R0` and RC can escape it. The particle's fast part *saturates* (1.87, Butler–
Volmer's `asinh`) while its slow part *accelerates* (6.01, because the OCP is a nonlinear
function of surface stoichiometry) — and the two sum to a total that merely looks mildly
sub-linear. A single resistance cannot be both at once, which is the whole lesson.

### 5. Repeatability separates the models without any appeal to magnitude

Five pulses, same train, the `climb` column:

| pulse | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| Ecm @1 C | 0.0748 | 0.0748 | 0.0748 | 0.0748 | 0.0748 |
| Spm/20 @1 C | 0.0173 | 0.0243 | 0.0317 | 0.0354 | 0.0372 |

The ECM's is **identical to four decimals, five times running** — a linear
time-invariant system answers an identical pulse identically. The SPM's more than
doubles and is still growing: 600 s of rest does not undo 60 s of diffusion, so gradient
accumulates from pulse to pulse. The cell remembers the earlier pulses; the circuit
cannot.

Same in the tail: of the fifth rebound, the ECM delivers **0.4 mV of 74.8 in its last
five minutes** (0.5 %) and the SPM **3.1 mV of 37.2** (8 %). This half of the argument is
placeholder-*sensitive* in magnitude — this file's RC pairs are labelled placeholders at
τ = 9 s and 72 s — and must be stated that way. What is not a free parameter is that the
particle's timescale is `r²/D` from Chen2020's geometry and diffusivity, not a number
anyone chose to make a point.

### 6. Where not to point the reader

Past the SOC clamp both models are finite and bounded — nothing goes NaN — but the SPM
pins at **0.39–0.50 V** against the ECM's 1.79 V, because its surface concentration runs
to the bottom of the OCP table while the ECM cannot leave its own `[ocv]`. Named as a
limitation, in the family of the overcharge hole; lesson parameters stop above the
cutoff.

## Part A — the `Pulse` demand mode

A pulse train is a **pure function of simulation time**, which makes it strictly simpler
than the CC-CV policy it borrows its machinery from:

```
phase = sim_time_s mod (t_on + t_off)
demand = phase < t_on ? Current(i) : Rest
```

No feedback, no band, no state to restore — CC-CV needed a band because it reads back a
quantity its own actuator sets, and this reads back nothing. What it *does* inherit is
the part that matters for principle 3: the edges are computed in **steps** from
`sim_time_s`, and `advance()` splits the frame at them, exactly as `k = round(CCCV_PERIOD_S
/ dt)` does. Otherwise the edge lands wherever the frame happened to end and the speed
multiplier decides the trajectory.

Three fields: current \[A\] (signed, discharge-positive like everything else on the page —
so a reader can pulse-*charge* by entering a negative), on \[s\], off \[s\]. A status line
naming the leg, the pulse index, and the time to the next edge.

## Part B — the scenarios

Two files that differ in **exactly one field**, which is this repo's idiom for making a
difference attributable:

- `scenarios/pulse_train_ecm.toml` — 1S1P `nmc_21700_lgm50` at 90 % SOC, 25 °C,
  isothermal, no BMS, no aging, no faults. `cell_model` omitted, so Ecm.
- `scenarios/pulse_train_spm.toml` — the same file plus `[pack.cell_model.Spm]
  shells = 20`.

Both at the same initial SOC on purpose: the ratio claims are read off the **first**
tooth, and a comparison whose two arms start at different SOC would be measuring the OCV
curve instead. 20 shells is `spm::DEFAULT_SHELLS`, chosen in Phase 6 by the accuracy
curve; the cost is ~1 µs per step at 1S1P, which at 100× and `dt = 0.5` is nothing.

Third file to edit, not add: `cc_discharge_lgm50.toml`'s comment, whose premise this
slice spends.

## Part C — the lessons

Three steps, path 11 → 14. Ordered so that each comparison is with the step immediately
before it:

12. **the circuit that repeats itself** — `pulse_train_ecm`, 1 C, 60/600, to 3300 s.
13. **the same train, on a particle** — `pulse_train_spm`, same everything.
14. **three times the current** — `pulse_train_spm` at 3 C, to 1980 s.

Step 14's mark is deliberately *below* step 13's: `applyStep` reloads when
`sim_time_s > L.until_s`, and that reload is what puts the 3 C run back at 90 % SOC where
its first tooth is comparable with step 13's. The same mechanism step 6 already relies
on.

## Verification

- Both transports. **Nobody has ever built an SPM pack through `sim-server` or
  `sim-wasm`** — the model has existed for two phases with no client — so "it works" is
  two claims, not one, and this repo has twice shipped the overclaim.
- The frame-independence check that CC-CV's notes call the one to repeat: the same
  program at a high speed multiplier over the socket must finish at the same
  `sim_time_s` as a native run, read from `GET /sessions` rather than the page's rounded
  readout.
- The `<select>` trap has now recurred twice, so `Pulse` is a real mode set by
  `applyStep`, never prose telling the reader to work the dropdown.

## Deferred, with a price

**Surface-vs-bulk stoichiometry on the wire.** The thing no equivalent circuit can show
at all — the reader would *see* the gradient rather than infer it from a rebound. It is a
`Telemetry`/`CellView` change, therefore `API_VERSION` and `WASM_API_VERSION` together, a
`web/pkg` rebuild, and the out-of-tree trajectory instrument enumerates its 17 telemetry
fields by name so an 18th is invisible to it. A slice of its own. `overpotential_v` is
already model-neutral and already on the wire, which is exactly what makes this slice
cost no Rust.

## Exit criterion

The guided path runs 14 steps. A reader can select the single-particle model from the
picker, drive a pulse train at two currents over both transports, and read the ×2.99 /
×3.00 that a circuit is obliged to give against the particle's ×1.87 and ×6.01.
