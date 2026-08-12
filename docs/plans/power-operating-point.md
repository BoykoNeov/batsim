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
`properties.rs:495`. None of them assert on `flags` for a power demand; each is to be
re-read rather than assumed.

## Versions

Read from each constant's own doc, per the standing rule that these have parted before.

* `SNAPSHOT_VERSION` (17): unmoved — no serialized field moves.
* `API_VERSION` (2) and `WASM_API_VERSION` (6): flags cross the wire as a **joined name
  string** (`"OV | UV | THERMAL_RUNAWAY"`), so a new flag is a new name that can appear
  there. Precedent is direct: `SOLVE_UNCONVERGED` was added the same way in Phase 6 slice D
  (44272b5), whose own note reads "API versions stay 2 (added fields, not renames)". What
  these constants version is names that a client *reads by name*; a client splitting the
  flag string on `" | "` sees an unfamiliar entry, exactly as it would have at Phase 6.
  Unmoved, on precedent rather than on a fresh argument — and recorded here so the next
  reader finds it decided.

## Results

*(filled in as the commits land)*
