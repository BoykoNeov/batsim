# Phase 9, slice A — the spike: does a many-particle LFP electrode give a plateau, and can the engine's solve carry it

**Status: RUN 2026-10-06. A measurement, so where it contradicts `ROADMAP.md`'s Phase 9 it
wins.** No engine code was written. The harness, the pre-registration (with two post-run
addenda), the raw output and two superseded runs live outside the repo under
`W:/temp/claude/phase9-spike/` (`PREREG.md`, `harness/`, `out/s20/` and `out/s40/` for the
numbers below, `out/s10/` and `out_v1/` superseded); the working tree was clean before and
after.

`ROADMAP.md` H1 proposes giving LFP — the chemistry the guided path opens with — a porous
model in which its flat plateau *emerges*: an ensemble of single particles sharing one
electrode potential, each with a non-monotone (double-well) open-circuit potential, so that
particles fill one at a time. Slice A was to ask two things with a toy: does the plateau
emerge, and is the solve stable across the unstable (spinodal) region.

## The answer in one line

**The solve is stable once each particle has 20 shells, but the physics is not what the
plan hoped: with the published interaction strength a plateau needs on the order of a
hundred particles per electrode and the charge/discharge gap at rest is exactly zero —
real LFP shows 20 mV; with an interaction strength fitted to that 20 mV, twenty particles
give a flat plateau and a 15 mV rest gap at real-time steps, but the gap is then capped by
the constant we fitted.** Which of those to build on, if any, is the owner's call (see the
end).

---

## The toy

* One LFP positive electrode, Prada2013 geometry printed from PyBaMM 26.6.2.0 (ε_s 0.374,
  0.18 m², 80 µm, `c_max` 22 806 mol/m³, D 5.9e-18 m²/s), split into N particles of equal
  volume; particle k has radius `R_k` and area `3·V_k/R_k`. Counter electrode: lithium
  metal held at 0 V; electrolyte at a constant 1200 mol/m³; 298.15 K.
* Each particle is diffused by **the engine's own `sim_core::spm::diffuse`**, at **20
  shells**, and read at the **end of the step** under its trial current, as `probe_at`
  does; the surface is the engine's half-shell extrapolation. Kinetics: Prada2013's
  Kashkooli 2017 exchange current, symmetric Butler–Volmer.
* OCP: a regular solution, `U(y) = U0 − kT·ln(y/(1−y)) − Ω·(1 − 2y)`, with `U0 = 3.42 V`
  and **Ω = 0.183 eV** from Bai, Cogswell & Bazant 2011 (arXiv:1108.2326, SI §1: "0.183 eV,
  which gives a broad miscibility gap of 0.035~0.965 at room temperature"). A second value,
  **Ω = 0.0759 eV**, was fitted here so the curve's two turning points sit 20 mV apart —
  the residual gap Dreyer et al. 2010 (Nat. Mater. 9, 448) measured on LiFePO4 at
  vanishing current. That one is fitted to one number by this spike, not cited.
* The split: one common electrode voltage, particle currents summing to the demand. The
  toy's **reference integrator** halves a step whenever the split misses its demand or a
  particle's answer leaves a band of ±5 C (of its own share) around its previous current —
  a continuity guard, so a step never jumps to another branch. That is a measuring
  instrument, not a proposal for the engine.
* Arms: N = 1; N = 2 identical (control); N = 2 with 40/60 nm radii; N = 20 and N = 100
  with lognormal radii (median 50 nm, σ 0.2, fixed seed). C/20 over the window
  y = 0.02…0.98, steps of 1 s, 60 s, 15 min and 1 h.

### Two runs that were thrown away, and why

**The first run was void.** Its split computed how far it missed its demand and discarded
the number, so a failed step was recorded as a good one. At Ω = 0.183 and N = 20 it lost
charge (397 steps where 1 200 conserve it at a 60 s step). Its numbers — including a
"flat" 0.12 mV plateau and a 27 mV rest gap — are not used.

**The second run, at the engine's default 10 shells, hit a fold that was the grid's.** At
Ω = 0.183, while the smallest particle (42 nm) transformed at 140× its share of the 1 C
current, its end-of-step curve had two answers 5 % apart even at a 2 ms step, and 5 622
sub-steps had to be committed unsolved. The cause is the surface read: at a tiny step the
surface's dependence on current is the half-shell extrapolation `0.5·dr·j/D`, which is
proportional to the shell width and does not vanish as the step does — the continuum's
does. Predicted before the rerun (`PREREG.md`, second addendum) and confirmed: at 20 and at
40 shells the same run has **no** unsolved sub-step, a miss ≤ 5e-13 of the 1 C current, and
the 20- and 40-shell trajectories agree (20–80 % span 167.61 mV both, mean offset −64.67 and
−64.70 mV). Every number below is from 20 shells unless marked, from runs whose charge
balance closes to ≤ 1.2e-12 of the window. **This is a finding for the engine too:** an
`Spm` at 10 shells is fine for a monotone OCP, but the half-shell surface read sets a
current above which a phase-separating one folds.

## What was measured

### Plateau (C/20 discharge, 1 s step; swings counted with a 2 mV hysteresis)

| arm | voltage span over 20–80 % | swings | median / largest swing | where it sits |
| --- | --- | --- | --- | --- |
| Ω 0.183, N = 1 (10 shells) | 143 mV | 1 | 143 / 143 mV | through the single-particle hump |
| Ω 0.183, N = 2 identical (10 shells) | bit-identical to N = 1 | — | — | particles never part (spread exactly 0) |
| Ω 0.183, N = 2 spread (10 shells) | 182 mV | 3 | 112 / 182 mV | one particle at a time, each through its own hump |
| Ω 0.183, N = 20 | 168 mV | 26 | 130 / 168 mV | mean 3.42 V − 65 mV |
| **Ω 0.183, N = 100 (40 shells)** | **48 mV** | **124** | **14 / 48 mV** | mean 3.42 V − 90 mV |
| Ω 0.0759, N = 1 (10 shells) | 20.0 mV | 1 | — | through the (small) hump |
| Ω 0.0759, N = 20 (10 shells) | 7.4 mV | 17 | 3.6 / 7.2 mV | mean 3.42 V − 9.4 mV |
| PyBaMM Prada2013 (Afshar 2017 OCP) | 8.5 mV | — | — | a slope written into the input table |

N = 100 still folded at 20 shells (191 unsolved sub-steps — its smallest particles are
smaller) and is quoted from 40 shells, where it has none. N = 1 and N = 2 have no fast
transformation to fold, so their 10-shell runs needed no
sub-step and stand. The Ω = 0.0759 discharge arms never come near the fold (no halving at
all at 1 s, miss ≤ 1.2e-12); its rest and long-step arms were rerun at 20 shells and read
the same to the digits quoted. The ±2 C probe row for Ω = 0.0759 is at 10 shells, rerun
on the audited harness.

Particles with different radii **do** fill one at a time (largest spread between two
particles 0.998 at N = 2, 20 and 100). But at the published Ω every particle that crosses
the unstable region drags the electrode voltage through most of its own 182 mV hump: N = 20
is twenty-odd humps, not a plateau. The swings shrink with N — the median falls about 9×
from N = 20 to N = 100 — so a plateau does emerge, but **at a hundred or more particles per
electrode**. The engine's `Spm` measured 0.215 µs per cell-step at 10 shells — two
particles, before the pack's nonlinear solve (`phase-6-porous-electrodes.md`) — so a
hundred-particle electrode at 20 shells is a single-cell teaching model, not a pack model.

### Rest gap (C/20 to 50 % from either end, then 2 h at zero current)

| Ω | after discharge | after charge | gap | with the band halved |
| --- | --- | --- | --- | --- |
| 0.183 | 3.42000 V | 3.42000 V | **0.00 mV** | 0.00 mV |
| 0.0759 | 3.41255 V | 3.42745 V | **14.91 mV** | 14.91 mV |

Predicted 120–182 mV at Ω = 0.183: **false, and by the whole amount**. Under current the
two branches sit ~130 mV apart (discharge mean −65 mV, charge +65 mV), but at rest the gap
is zero. The trace says why: at the published Ω a transforming particle runs away, pulling
lithium from every other particle so hard that, by the end of the drive, the untransformed
particles sit at y ≈ 0.0008 and the transformed ones at ≈ 0.9992 — the equilibrium ends of
the miscibility gap — rather than parked part-way up their branches. Particles at the
equilibrium ends read exactly `U0`, whichever way they got there. At Ω = 0.0759 the
particles stay part-way (0.13 and 0.95) and a 14.9 mV gap survives — but that Ω was fitted
so the turning points are 20 mV apart, so **a gap below 20 mV is guaranteed by
construction** and is not a confirmation of Dreyer. What it adds is that three quarters of
the allowed gap survives a two-hour rest, and that the gap depends on which way the cell
arrived — the path dependence H1 asks for.

### Stability — the deciding measurement

| | Ω 0.183 | Ω 0.0759 |
| --- | --- | --- |
| 1 s step, N = 20: steps needing sub-steps | 150 of 72 000, smallest 31 ms, all solved | **0** |
| 1 s step, N = 100 (40 shells) | 1 161 of 72 000, smallest 2 ms, all solved | not run |
| 60 s step | 723 of 1 200, 5 118 sub-steps, smallest 29 ms | 38 of 1 200, 1 247 sub-steps, smallest 15 s |
| 1 h step (20 steps) | all 20, 4 377 sub-steps, smallest 27 ms | 19 of 20, 596 sub-steps, smallest 14 s |
| each particle's end-of-step curve, ±2 C around its operating point: rising anywhere | 0 of 1 000 at 1 s, 3 at 5 s, 5 at 15 s, 321 at 60 s, ~all from 300 s | 0 at 1–15 s, 2 at 60 s, 523 at 300 s, ~all from 900 s (10 shells) |
| the same over every current the step allows: rising anywhere | 999 of 1 000 at 1 s | 0 of 1 000 at 1–15 s, 836 at 60 s |

**At Ω = 0.0759 the engine's kind of solve works as it stands at real-time steps**: every
particle's curve falls with its current over the whole range at 1–15 s, so the split has
one answer and a bracketed search finds it. Fast-forward needs sub-steps of about 15 s.

**At Ω = 0.183 the model has an answer, but not a unique one.** Near the operating point
every curve falls; over the whole range a step allows, almost every curve rises somewhere,
so a search that brackets the split — what the engine does today — can land on a different
branch. The reference integrator keeps to the branch it came from by refusing any answer
more than 5 C from the last one, and needs that during every transformation (sub-steps down
to ~30 ms); halving the band changes no number by more than 0.02 mV. An engine version would
need the same continuity rule built into the pack's split, and a fast-forward hour costs
~220 solves.

**The closed-form boundary was refuted.** The pre-registered check was that a step needs
splitting once it is longer than the unstable mode's growth time, `Q_k·(dη/di)/U′`. On every
halved 1 s step that time was **longer** than the step (0 of 150); at 60 s the halvings fall
on both sides of it (588 above, 135 below). The halvings follow the transformation currents,
not the bulk growth rate, so there is no one-line formula for the safe step.

### Scoring the pre-registration

| prediction | result |
| --- | --- |
| P1 N = 1 rises ~180 mV mid-discharge | held (143 mV over 20–80 %, 182 mV whole run) |
| P2 identical particles never part | held, bit-identical to N = 1 |
| P3 N = 2 spread shows no 180 mV hump | **false** — each particle shows its own hump |
| P4 N = 20 plateau within 20 mV at U0 − 60…91 mV | **false** at Ω 0.183 (168 mV); held at Ω 0.0759 (7.4 mV), by construction |
| P5 rest gap 120–182 mV at Ω 0.183; 12–20 mV at 0.0759 | **false** at 0.183 (0.00 mV); held at 0.0759 (14.9 mV), by construction |
| P6 local curve monotone at 1 s and 5 s, rising from ~15 s | held at 1 s, rising at 5 s on 3 of 1 000 — and the ±2 C band was the wrong instrument: transforming particles run 140 C |
| addendum 1: sub-steps where the growth time is shorter than the step | **refuted** |
| addendum 2: the 2 ms fold is a 10-shell artifact, gone at 40 shells | **held** — gone at 20 already |

## What it decides

The pre-registered **stop** condition — particles moving through the unstable region
together — was not met: they fill one at a time. The **more-expensive** branch was met at
the published constant: real-time steps need a continuity rule and sub-steps in the split.
But the expense is the smaller problem. **At the published constant the model does not
produce the two things H1 wants**: no plateau at a particle count a pack can afford, and no
hysteresis at rest at all. The options, for the owner:

1. **Build on the fitted constant (Ω ≈ 3 kT, fitted to the 20 mV rest gap).** Works with the
   engine's solve as it is at real-time steps, twenty particles, ~15 s sub-steps for
   fast-forward. Produces: a plateau flatter than any single particle (7 mV against 20 mV),
   particles filling one at a time, and a rest voltage that depends on which way the cell
   arrived. Does not produce: the size of that gap — it is capped by the number we fitted.
   And the miscibility gap comes out 0.075–0.925 against the measured 0.035–0.965. This
   closes H1's plateau half and the *existence* of its hysteresis, not its magnitude.
2. **The published constant plus internal phase separation in each particle** (a
   gradient-energy, Cahn–Hilliard-family term) — the literature's answer to why real
   particles hold a 20 mV gap at that constant. A second spike, a PDE per particle, and
   still ~100 particles for a plateau on this measurement.
3. **Shrinking-core** (the ROADMAP's named alternative): a plateau with no hysteresis,
   cheaper, no unstable region to cross. Closes the plateau half only.
4. **Stop Phase 9 here.** LFP stays on the equivalent circuit; H1 is recorded as
   measured-and-declined with this note as the reason.

## Still open

* Charge direction at N = 100, and N between 100 and a few hundred, were not run.
* The engine's cost for an N-particle cell was not measured; the toy's own cost (~30 min
  for 20 simulated hours at N = 100, nested bisection) is not representative of anything.
* No PyBaMM run reproduces the phase-separation claims; Prada2013's OCP is monotone by
  construction. Any later slice still needs a cited reference with a tolerance for the
  plateau and the rest gap.
* Rate dependence of the knee — the fourth item H1 lists — was not measured.
