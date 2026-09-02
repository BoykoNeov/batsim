# Phase 8, slice C — the spike, and what it decided

**Status: RUN 2026-08-28. This is a measurement, so where it contradicts
`phase-8-chemistries.md` it wins.** No engine code was written. The harness, the
throwaway parameter file, the pre-registration and the raw output live outside the repo
under `M:/claud_projects/temp/nimh-spike/`; the working tree was clean before and after.

`phase-8-chemistries.md` scheduled this before slice C's schema could be designed, on the
Phase 6 and Phase 7 discipline of spiking before authoring. It asked one question:

> The voltage of a full NiMH cell on constant current **falls**, and that fall must come
> out of the physics […] **Whether the existing overcharge-heat path is enough to produce
> it is the spike question and is not known today.**

## The answer in one line

**Yes, it falls — but through one channel the plan did not name, at roughly half the
size needed at the instant that matters, and with a corner shape that is an artifact of
the SOC clamp rather than physics.**

---

## The plan named three ingredients. One of them does not exist

The plan expected a peak-then-fall to come out of three things composing:

1. charge the cell stops accepting near full, and the heat that makes (`i_rejected_a`);
2. a negative `docv_dt_v_per_k`, which `OcvTable` already carries as an optional column;
3. "the thermal network, which already makes a warming cell's OCV fall."

**The third is false, and it was settled by reading before anything was run.**
`ecm.rs:672` is the whole of `ocv_lookup`:

```rust
pub fn ocv_lookup(table: &OcvTable, soc: f64) -> f64 {
    interp1(&table.soc, &table.volts, soc)
}
```

Open-circuit voltage is a pure function of SOC. The field's own doc comment
(`chem.rs:653`) says so — *"It is **not** used to temperature-correct OCV itself"* — so
this is a documented deferral rather than a defect, and `docv_dt_lookup` has exactly one
consumer, the reversible term in `cell_heat_w`. RC-pair resistances are temperature-free
too: `rc_update` takes an `r_ohms` with no `T`, and the `[[rc]]` schema has no grid.

So **the engine has exactly one temperature → voltage channel, `R0(soc, temp_k)`**, and
the spike question is not "do the three compose" but "is that one channel enough."

## Why the shape was never going to be evidence

`coulomb_step` clamps SOC hard at `1.0`. Above the clamp `ocv_lookup` is pinned at
`OCV(1.0)` while the cell keeps heating and `R0` keeps falling, so **a peak followed by a
fall is structurally guaranteed** for any `[r0]` table with a negative temperature slope,
at any magnitude, on any chemistry. Reporting "it peaks and falls" would have been
reporting the clamp — the same structural blindness that let the conservation tests miss
the reversal ramp (`low-clamp-reversal.md`).

This was written down before the run. Only magnitudes and arm-to-arm differences count.

## Three arms, and a pre-registration

Three runs differing in one thing each: **A** thermal live, **B** isothermal (temperature
pinned — the control), **C** thermal live with a `docv_dt` column of −1.0 mV/K applied as
one mutated field in Rust so the two chemistries are provably identical otherwise.
Everything else held: 1S1P sub-C NiMH, 3.0 Ah, 90 % SOC, 298.15 K, 1 C charge, no BMS, no
aging, no `[safety]`, 1200 s at `dt = 0.1`.

`R0` was made **deliberately flat in SOC** so that every millivolt of pre-clamp rise is
`OCV(soc)` and every millivolt of post-clamp fall is temperature. Its temperature axis is
Arrhenius with `Ea = 19 kJ/mol`, which is not a free number: it is what "NiMH internal
resistance roughly doubles from 25 °C to 0 °C" implies.

Seven numeric predictions were registered before the run. **Six held; one was falsified.**

| | prediction | measured |
| - | ---------- | -------- |
| ✅ | Arm B falls **0.0 mV** | **0.000 mV** |
| ✅ | Arms agree within **1.0 mV** pre-clamp | **0.333 mV** |
| ✅ | Clamp at **t = 360 s** | **360.1 s** |
| ✅ | Arm A peaks **1.4650 V** at the clamp, then monotone | **1.464667 V**, largest upward step after it **0.000000 mV** |
| ✅ | Arm A falls **15.6 mV** by 1200 s | **15.478 mV** (**15.811 mV** against arm B) |
| ⚠ | *"The fall **passes through** the 5–20 mV/cell band that real chargers detect −ΔV in, and spends most of the run inside it."* | **true as written, and uninformative** — see below |
| ❌ | Arm C falls **~2 mV less** than arm A | **1.821 mV more** |

That sixth row is quoted verbatim because the paraphrase flattered it. Scored against its
own words it **holds**: the fall crosses 5 mV at about t = 517 s (interpolated between the
+10 K and +20 K rows of the table below) and stays in the band for roughly 81 % of the
840 s of overcharge. But it was **registered on the wrong instant** — over a whole
twenty-minute run, rather than at the few-minute mark where the behaviour is defined and
detected. Finding 1 below is the prediction that should have been written, and it comes out
the other way. A green registered on the wrong claim is a distinct defect from a red, and
this is one.

Arm B's peak is **exactly** 1.465000 V; arm A's is 0.33 mV under it because arm A has
already warmed 0.73 K by the clamp, which is prediction 2 showing up in prediction 4.

---

## Three findings, in the order they matter to slice C

### 1. The magnitude is short by about a factor of two where a charger actually looks

The 15.5 mV headline is measured after a **49 K** rise over twenty minutes. Real chargers
terminate on 5–10 mV per cell within a few minutes of overcharge, when the cell has risen
about 10 K. Arithmetic on the measured temperature trace — not a second simulation, since
there is no OCV temperature term to run:

| rise after the peak | when | measured **ohmic** fall | + an OCV term at −0.5 mV/K |
| ------------------- | ---- | ----------------------- | -------------------------- |
| +5 K | 430 s | 2.296 mV | 4.80 mV |
| **+10 K** | **503 s** | **4.589 mV** | **9.59 mV** |
| +20 K | 658 s | 9.033 mV | 19.04 mV |
| +30 K | 827 s | 11.597 mV | 26.60 mV |

**At +10 K the ohmic channel alone gives 4.589 mV — just under the 5 mV a charger fires
on.** With an OCV temperature correction at a plausible −0.5 mV/K it is 9.59 mV, inside
the band, with roughly half the signal from each mechanism — which is also how the
literature splits it.

The engine can therefore produce a −ΔV today, but only by running the cell far hotter and
longer than the behaviour is about.

### 2. The corner is the wrong shape, and no parameter fixes it

Charge acceptance goes from 100 % to 0 % in one timestep, because that is what a hard SOC
clamp is. Across the clamp:

```
t = 359.9 s  V = 1.464570 V   dV/dt =  +0.9714 mV/s
t = 360.0 s  V = 1.464667 V   dV/dt =  +0.9714 mV/s     <- peak
t = 360.1 s  V = 1.464664 V   dV/dt =  -0.0334 mV/s
```

**The slope reverses by a factor of 29 in a single 0.1 s step.** A real NiMH peak is
rounded over tens of seconds, because oxygen recombination takes a growing share of the
current well before the cell is full. A student looking at this plot sees a wedge.

This is the part that is not fixable in the chemistry file, and it is a **third**
mechanism — a charge-acceptance taper — distinct from both the hysteresis state slice C is
scoped around and the OCV term above. The plan asked whether the phase needs one. **It
does, if the lesson is about the shape.** It does not, if the lesson is about the number.

### 3. `docv_dt` currently reaches −ΔV only through a temperature-history artifact

The falsified prediction is the most useful result here.

I predicted arm C would fall *less*: a negative `∂U/∂T` on charge makes
`q_rev = −I·T·∂U/∂T` negative, the cell cools, `R0` drops less, the fall shrinks. The
temperature half was right — arm C ends at **334.23 K** against arm A's **348.01 K**, on
3.35 W against 4.35 W.

What I missed is that the term also acts **before** the clamp, where it dominates:
pre-clamp ohmic heat is 72 mW against an entropic −894 mW, so **arm C's cell cools to
294.06 K, four kelvin below ambient, on the way up**. That raises `R0`, which lifts the
peak to 1.468995 V, and the bigger fall is the recovery from it.

> So the effect of `∂U/∂T` on the fall is the difference between how much it cooled the
> cell before the clamp and how much after — two large opposing terms whose net sign
> depends on where the run started and how long the approach was. **A lesson resting on
> that is resting on cancellation.**

That is a stronger argument for wiring the coefficient into voltage than the one I
registered, not a weaker one.

---

## What this decides for slice C

### Recommendation: slice C should add an OCV temperature correction, `Option`-shaped

Three reasons, in descending strength:

1. **It is what `CLAUDE.md` already specifies.** The ECM section says the `dOCV/dT` table
   is "optional […] for temperature correction **and** entropic heating". The code does
   only the second half. Same spec-vs-code shape as the RC resistance-growth divergence,
   which was resolved on the code side (`rc-resistance-growth.md`).
2. **It cannot move an existing trajectory, and that is structural rather than lucky.**
   **No shipped chemistry carries a live `docv_dt_v_per_k` column** — all five files
   mention the field only in a comment recording its absence
   (`lfp:84`, `lto:135`, `nmc_18650:87`, `lgm50:126`, `pba:232`). The `Option` is `None`
   on every trajectory in the repo, so a new term behind it is reached by nothing that
   exists. Exit criterion 3 stays structural, the way `[diffusion]` made it structural.
3. **It is the mechanism the behaviour is actually named for**, and it doubles the signal
   at the instant the behaviour is detected (table above).

Note this makes the phase's one snapshot bump carry **two** things — the hysteresis state
and this term — which is still one bump, the budget Phases 6 and 7 both held. **And the
term on its own would not force one**, which is worth arguing rather than asserting: it
adds no state and changes no layout, it changes how an *existing* field is read. That is
the semantic-bump case `pack.rs` makes at length for v10 and v11, where the version field
was the only thing standing between an old blob and a build whose meaning had moved. Here
even that does not bite, for the same reason criterion 3 is safe: **no blob in existence
carries a `docv_dt_v_per_k` column**, so there is no snapshot whose meaning the new reading
could change. It rides slice C's bump because slice C is having one, not because it needs
one.

### The cut order, priced against the stopping rule

This spike returns **two** scope additions where the plan asked about one, and the phase
has a stopping rule written precisely because earlier work here ran seventeen days on an
unbounded generator. So the cut order is named here rather than left to be inferred:

1. **Hysteresis state — required.** It is what slice C *is*, it serves NiMH and the shipped
   lead-acid cell together, and two earlier documents already argued it should not be split.
2. **OCV temperature correction — strongly recommended, and cheap.** It is already specified
   in `CLAUDE.md`, it cannot move a trajectory or a snapshot, and it doubles the −ΔV signal
   at the instant that matters. Cutting it means the nickel lesson is carried by an ohmic
   side-effect rather than by the mechanism it is named for.
3. **Charge-acceptance taper — cuttable, and this is the recommendation.** *(Built
   2026-09-02, after the phase closed, as the slice this section anticipates:
   `docs/plans/charge-acceptance.md`, `SNAPSHOT_VERSION` 21.)* It is the only
   one of the three that could grow the phase, it is a genuinely new mechanism rather than
   an un-stubbing, and slice B already recorded the reason it can go: *"a shape on a plot is
   weaker than a number on a row."* **Cut it, and write slice D's lesson about the number**
   — a −ΔV in millivolts on a telemetry row, with an isothermal control arm beside it, which
   is the pattern slice B measured as the cheapest reachable contrast in the repo. The wedge
   stays wrong on the plot; the lesson does not depend on the plot.

If a future session wants the rounded peak, that is a slice against the recipe in the phase
plan's stopping rule, not a reopening of Phase 8.

### Open, and deliberately not decided here

* **Whether to model the charge-acceptance taper.** Finding 2 says the shape is wrong
  without one and says nothing about what it costs. This is the question slice C's author
  should price first, because it is the only one of the three that could grow the phase.
  If it is refused, the NiMH lesson must be written about the **number** (a `-ΔV` in
  millivolts, on a row) rather than the **shape** (a peak, on a plot) — which is the same
  choice `phase-8-slice-b-lto-client.md` recorded: *"a shape on a plot is weaker than a
  number on a row."*
* **The coefficient itself.** −0.5 mV/K is used above as plausible, not sourced. A shipped
  file needs a real one under the provenance rule.
* **Everything about hysteresis.** No arm here touched drive-direction memory; the spike
  was scoped to exit criterion 4 alone.

### What is not sensitive to my typing

The 15.5 mV scales with the `[r0]` activation energy, which is anchored on one
order-of-magnitude observation. **The arm-to-arm structure is not**: arm B's exact zero,
the 29× one-step corner, and arm C's sign reversal hold for any magnitude, and those are
the three things the recommendation rests on.
