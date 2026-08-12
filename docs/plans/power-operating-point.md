# A power demand never says where it landed

The follow-up `docs/plans/voltage-target-blowup.md` left open, re-scoped by measurement.
That document recorded the symptom as a size:

> `Power` still reaches large numbers — `Spm Power(-1e12)` gives -1.2e11 A, `Dfn
> Power(-1e6)` reports 2952 V, both flagged.

**Both flagged** is the part that turned out to be an accident of the probe. Re-measured
on the shipped LG M50 at 50 % SOC, 1S1P, no BMS, the equivalent circuit is the model that
matters and it is silent:

| demand | `dt` | current | terminal | flags |
| ------ | ---- | ------- | -------- | ----- |
| `Power(-1e5)` | 1 s | -1 926 A | 54.6 V | *(none)* |
| `Power(-1e6)` | 1 s | -6 250 A | 171.8 V | *(none)* |
| `Power(-1e12)` | 1 s | -6.32e6 A | 171 790 V | `SOC_CLAMPED_HIGH` |
| `Power(-1e12)` | **0.001 s** | **-6.32e6 A** | **162 440 V** | ***(none)*** |

The last row is the finding. Six million amps at a hundred and sixty thousand volts, on a
cell whose declared window is 2.50–4.20 V, reported as a clean converged step with nothing
raised at all. The `SOC_CLAMPED_HIGH` on the row above it is **incidental** — it fires
because a one-second step at that current happened to fill the cell, not because the
current was absurd. Shorten the step below the fill and the last signal disappears.

## Why the existing safeguard cannot reach this

`SOLVE_UNCONVERGED` is raised when an iteration hits its cap. The equivalent circuit has
no iteration: `ecm::solve_current` answers `Demand::Power` with the closed-form root of
`r0·i² − e·i + P = 0`, exactly, on the first pass. So the flag that covers every other
absurd row in the sweep is **structurally unable** to cover this one, and the porous
models only raise it here because their tangent iteration struggles on the way — which is
a numerical accident, not a report about the operating point.

Nothing is wrong with the arithmetic, and a `dt = 0` probe says so: the reported `V·I`
equals the demanded power to five digits at every magnitude (-1e3 → 7.2163 V, -1e6 →
160.00 V, -1e9 → 5001.9 V, -1e12 → 158 120 V). The engine is not failing. It is answering
a question exactly and not mentioning that the answer is off the map.

Conservation is intact too, and this was checked before the slice was scoped rather than
assumed: at `Power(-1e6)` the 6 250 A really does enter the cell (`soc` 0.500 → 0.837 in
one second, `i_rejected_a` correctly zero), and at `Power(-1e12)` the part that will not
fit is refused and reported (`i_rejected_a` -6.3152e6 A against `i_actual` -6.3245e6 A).
**This is a reporting defect, not an energy one** — which is what keeps it a flag rather
than a fix to the ledger.

## The prior slice declined a window, and half its reason has since been falsified

`voltage-target-blowup.md:171` declined the symmetric fix:

> a power demand that sags a pack below `v_min` is **ordinary physics** and the `UV` flag
> exists for it, unlike an out-of-window voltage *hold*

`EventFlags::UV` is raised in exactly two places, both in `bms.rs`. **With `bms: None` —
the configuration in every measurement above, and a supported, first-class teaching mode
per `CLAUDE.md` — no flag exists for it.** The sentence is true of a pack with protection
configured and false of one without, and it was written without that distinction.
`solve_safeguard.rs`'s module header repeats the same reasoning and inherits the same
defect.

What survives the correction is the *other* half, and it is the half that decides the
design: **a window on power is still the wrong fix.** Not because the condition is
reported elsewhere, but because the two directions are not symmetric physics.

## The asymmetry, which is the content of this slice

Discharge power is **bounded by the cell**. `P = V(i)·i` on a Thévenin source has a
maximum at `i = e/(2·r0)`, where `V = e/2` and `P = e²/(4·r0)` — 133.803 W for this cell
at 50 % SOC. Ask for more and there is no operating point at all; the closed form's
`disc <= 0` arm snaps to the maximum, which is correct physics and should stay.

Charge power is **not bounded by anything**. `V` grows without limit as `i` goes negative,
so `V·I` covers every magnitude and any demand is met exactly, at an operating point
arbitrarily far outside the cell's window.

That asymmetry is written down nowhere in the repo. `solve_current`'s doc comment says
"snap to the max-power point if the target power is unreachable" and is silent about the
charge branch having no such point.

So the fix is not a bound. It is that **the engine should say when a power demand's
operating point left the pack's declared voltage window**, in either direction, and it
currently never does.

## One predicate covers both arms, and the discharge arm is provable

The predicate is the pack's own declared window, `series × [v_min, v_max]` — the same
source the voltage demand window uses, required in every `[cell]` block and carrying
provenance, so no constant is invented.

The charge arm is obvious: 162 440 V is not in 2.50–4.20.

The discharge arm is not obvious, and is worth deriving rather than measuring. At the
max-power point `V = e/2` exactly. So the snap lands outside the window whenever

```
e < 2 · v_min
```

and since `e = OCV(soc) − Σ V_rc − η ≤ OCV(1.0)` for any cell that is not carrying a
negative overpotential into the step, the shipped chemistries all satisfy it with room:

| chemistry | `OCV(1.0)` | `2 · v_min` | headroom |
| --------- | ---------- | ----------- | -------- |
| LFP 26650 | 3.60 | 4.00 | 0.40 V |
| NMC 18650 | 4.20 | 6.00 | 1.80 V |
| NMC 21700 LG M50 | 4.20 | 5.00 | 0.80 V |
| Lead-acid AGM 2 V | 2.13 | 3.50 | 1.37 V |

**The caveat is real and belongs in the doc comment, not in a footnote.** `e` exceeds
`OCV(soc)` whenever the RC pairs carry a negative voltage into the step — i.e. immediately
after charging. LFP's 0.40 V of headroom is the tightest, so an LFP cell entering a huge
discharge-power demand with more than 0.40 V of accumulated charging overpotential would
snap to a max-power point *inside* its own window and not raise the flag. That is a
narrow, real, and stated limit rather than a claim that the arm always fires.

Deliberately **not** enforced in `ChemistryParams::validate`: the inequality is a property
of when a *flag* fires, not of whether a chemistry is physical, and a validator that
refused `OCV(1.0) ≥ 2·v_min` would reject parameter sets that simulate perfectly well.
Recorded in the flag's own documentation instead, where the reader who hits the gap is
looking.

## The flag fires on ordinary demands too, and that is intended

`Power(140.0)` on this cell delivers 134.0 W at 1.93 V — a demand essentially *met*, at a
terminal below `v_min`. It flags. This is not an absurdity detector: it is a report that
the operating point is outside the declared window, and 1.93 V on a 2.50 V cut-off cell is
exactly that. With a BMS configured the same condition also raises `UV`; with `bms: None`
this is the only thing that says so.

Named `POWER_OUT_OF_WINDOW` rather than anything mentioning absurdity or failure, so the
name cannot be read as "the engine went wrong here."

## Why `Demand::Power` only

`Demand::Current` sags a pack below `v_min` just as easily and will not raise this. That
is deliberate and is the distinction the slice rests on: **with a current demand the
client chose the operating point and knows it; with a power demand the engine chose it and
the client cannot predict it.** Asking for 1 000 W does not tell you whether you are about
to draw 5 A or 6 million. `Demand::Voltage` is already fenced by the demand window and
cannot leave it at all.

Widening this to every demand type is a bigger and defensible slice — "the engine reports
when it is operating outside its declared window, whatever it was asked" — but it would
move flags on existing hard-discharge trajectories rather than only on power demands, and
it is not this one.

## Implementation

Three commits, following the staging that `diffusion-overpotential.md` established, so
that the "nothing else moves" claim is tested separately from the physics:

1. **The bit and its doc, raised nowhere.** `EventFlags::POWER_OUT_OF_WINDOW = 1 << 13`.
   The full suite must be identical, twice.
2. **Wire it in `pack.rs::step`**, on the `Demand::Power` arm only, against the solve's own
   operating point rather than `Telemetry::v_terminal` — the latter is an **end-of-step**
   read and a step that ran 6 million amps has moved the SOC underneath it.
3. **Tests, perturbation, and this document's results.**

**The predicate must be written as the negation of in-window, not as `v < lo || v > hi`.**
A NaN operating point answers `false` to both comparisons and would leave the flag down —
the same trap the diffusion slice paid for at `soc = 0`, where `x >= 1.0` answered `false`
for NaN and the value reached the Thévenin source unnoticed. `Demand::check_finite` exists
but is a check the *caller* opts into, so `step` can still be handed a NaN. A NaN case goes
in the test file.

## Blast radius

Flags are recomputed fresh each step and are **not** part of the snapshot, so
`SNAPSHOT_VERSION` does not move. **No scenario file uses a power demand at all**, so no
committed golden trajectory is in scope. The affected call sites are all tests:
`nonlinear_solve.rs:284` (`Power(150·s)`, self-described as at or past the knee),
`solve_safeguard.rs` (1e3/1e6/1e12), `dfn_cell.rs:809`, `topology.rs:245`,
`properties.rs:495`. None of them assert on `flags` for a power demand; each was re-read
rather than assumed.

`properties.rs` is the one worth naming, because it is the one that could have passed by
luck: it is a proptest, and its magnitudes are drawn from `-500.0..500.0` W against a
fixture whose max-power point is far below that, so it **does** reach past the knee and
**does** raise the new flag on some draws. It stays green by construction rather than by
seed — it runs with a BMS configured and asserts on the C-rate window the protection
enforces, which the flag does not touch.

## Versions

Read from each constant's own doc, per the standing rule that these have parted before.

* `SNAPSHOT_VERSION` (17): unmoved — no serialized field moves.
* `API_VERSION` (2) and `WASM_API_VERSION` (6): flags cross the wire as a **joined name
  string** (`"OV | UV | THERMAL_RUNAWAY"`), so a new flag is a new name that can appear
  there — which is neither a method name nor a JSON field name, the two things
  `WASM_API_VERSION`'s own doc says it versions. Precedent is direct (`SOLVE_UNCONVERGED`
  was added the same way in Phase 6 slice D, 44272b5, whose note reads "API versions stay 2
  (added fields, not renames)"), but precedent is not the same as checking, and the rule
  here is to read each constant's own doc rather than move them as a set.

  **So the client was read instead of assumed, and it does not break.** `web/app.js`'s
  `parseFlags` splits the string and keeps *every* name; the renderer turns each into a
  chip, styling it from a `SEVERE` set and falling through to plain for anything else. There
  is no known-flag list to fall off, so `POWER_OUT_OF_WINDOW` displays on a page built
  before it existed. The one hardcoded list, `PROTECTION_FLAGS`, answers a different
  question — which flags mean *the BMS stopped the charge* — and this is correctly not one
  of them. `sim-godot` names only `THERMAL_RUNAWAY` and `CONTACTOR_OPEN`, for signals, and
  is untouched. Unmoved, on evidence.

## Results

The headline row moves and nothing else does:

| demand | `dt` | current | terminal | before | after |
| ------ | ---- | ------- | -------- | ------ | ----- |
| `Power(-1e12)` | 1e-6 s | -6.32e6 A | 162 440 V | *(none)* | `POWER_OUT_OF_WINDOW` |
| `Power(-1e6)` | 1 s | -6 250 A | 171.8 V | *(none)* | `POWER_OUT_OF_WINDOW` |
| `Power(1000)` on ECM | 0 | 75.04 A | 1.78 V | *(none)* | `POWER_OUT_OF_WINDOW` |
| `Power(1000)` on SPM | 0 | 451.5 A | 1.35 V | *(none)* | `POWER_OUT_OF_WINDOW` |
| `Power(600)` on SPM | 0 | 235.6 A | 2.52 V | *(none)* | *(none)* — in window |
| `Power(100)` on ECM | 0 | 34.66 A | 2.84 V | *(none)* | *(none)* — in window |

The last two are the evidence that this is a window test and not an absurdity heuristic:
600 W on the single-particle cell holds 2.5214 V against a 2.50 V floor and stays down,
where 1 000 W holds 1.3513 V and fires. The two models' thresholds are in different places
because their curves are, which is what a predicate on the operating point should do.

**Full workspace suite green: 66 test binaries, 0 failed.** No golden moved, and none
could — no scenario file issues a power demand. Clippy clean at `-D warnings`, `cargo fmt`
clean.

## Verification

Nine tests in `sim-core/tests/power_operating_point.rs`, on a fixture whose every
threshold is derived rather than measured: at 50 % SOC the source is 3.20 V behind
0.02 Ω, so the in-window band is exactly −52.5 W to +43.5 W and the max-power point is
80 A at 1.60 V delivering 128 W.

**Both perturbations were run against a real exit code, not a read of the output.** The
standing rule here is that `start /belownormal` is exit-code-blind — proven twice in this
repo — so the harness drives `System.Diagnostics.Process` directly and sets
`BelowNormal` on the object after launch. (`Start-Process -PassThru` was tried first and
returns a null `ExitCode`, which would have looked exactly like a pass.)

* **Neuter the raise** → exit **101**, 7 of 9 red. The two survivors are
  `a_voltage_demand_can_never_raise_it` and
  `the_in_window_band_is_where_the_arithmetic_puts_it` — both assert the flag stays
  *down*, so they should hold when nothing raises it. They are inertness guards, not flag
  guards, exactly as `demand_window.rs`'s two survivors are.
* **Swap the predicate to the NaN-blind spelling** (`v < lo || v > hi`, which is what
  clippy's `nonminimal_bool` rewrites the honest form into) → exit **101**, and *exactly
  one* test red: `a_non_finite_power_demand_is_out_of_window_not_in_it`. That is the
  measurement that makes `within_inclusive` load-bearing rather than decorative — the
  other eight are indifferent to the spelling, and only a dedicated NaN case can see it.

## Prose this falsified elsewhere

Hunted by capability rather than by string, per the rule the balancing slice left:

* **`voltage-target-blowup.md:171`** — the paragraph declining a power window. Two of its
  three claims do not survive: "both flagged" was an artefact of probing only the porous
  models, and "the `UV` flag exists for it" holds only with a BMS configured. Corrected
  in place with a dated block rather than rewritten, so the original reasoning stays
  readable. Its *conclusion* stands, for a better reason than it gave.
* **`solve_safeguard.rs`** — module header and one test doc repeated the same argument and
  inherited the same defect; every pack in that file is built with `bms: None`, which is
  the configuration the claim is false for.
* **`ecm::solve_current`** — its doc said "snap to the max-power point if the target power
  is unreachable" and was silent about the charge branch having no such point. The
  asymmetry is now stated where the arithmetic is.

## Deferred, with a price

* **Widening this beyond `Demand::Power`.** A `Demand::Current` leaves the window just as
  easily and stays unflagged. Defensible as it stands — the client picked that operating
  point — but "the engine reports when it is operating outside its declared window,
  whatever it was asked" is a coherent and bigger feature. It would move flags on existing
  hard-discharge trajectories rather than only on power demands, so it needs its own
  argument and its own blast-radius measurement.
* **`i_rejected_a` on the charge side is already right**, and was checked before this slice
  was scoped rather than assumed: at `Power(-1e6)` the 6 250 A genuinely enters the cell
  (`soc` 0.500 → 0.837 in one second, nothing rejected), and at `Power(-1e12)` the excess
  is refused and reported. **This was the finding that kept the slice a flag** — had the
  ledger been wrong, the priority would have been a conservation fix and the flag
  secondary.
