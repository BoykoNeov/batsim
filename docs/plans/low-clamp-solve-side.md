# The low clamp: a solve-side fix that was priced, measured, and does not work

The first of the two items `docs/plans/energy-hole.md` left open. It was priced there as
a slice with a known cost:

> **The low clamp still fabricates energy.** Reported and pinned, not fixed. The
> solve-side fix costs `CellModel::is_linear() == true` for the equivalent circuit and
> collides with `CLAUDE.md`'s "with `bms: None`, demands pass through unclamped"; it wants
> its own slice and its own argument.

This document is that argument, and its conclusion is that **the price was quoted for
something that does not close the hole**. Nothing is committed to `sim-core` for it. The
work is a spike, at `M:\claud_projects\temp\lowclamp-spike` — a `git worktree` of the
engine with the candidate patches applied, plus a probe crate path-depending on it.

## The plan the measurement was taken against

`is_linear()` becomes a question about a cell's *state* rather than its model: an
equivalent-circuit cell answers `false` only when it is parked on `soc == 0.0`. The pack
already asks the question per cell — `pack.rs`'s own doc comment says the solve is
"mixed-ready … `CellModel::is_linear` is asked per cell", with a `debug_assert` that "will
have to be deleted rather than satisfied" — so the change is one `first()` becoming an
`any()`, and a pack away from empty keeps the exact closed form. The empty cell then gets
a curve that refuses to over-deliver.

Testing `soc == 0.0` exactly is reliable because `coulomb_step` writes exactly `0.0`, and
it keeps `is_linear()` a `&self`, `dt`-free question.

The high clamp is deliberately **not** touched. A symmetric cap there would delete the
refused-charge heat term the previous commit shipped, which on an ECM is now *the*
runaway path.

## Three candidate curves, and what each one did

Every run: 1S1P, 2S2P and 2S2P-with-scatter, driven from `soc = 0.05` at 40 A for 60
steps of 0.5 s — 1200 As offered against 450 As stored — on `Current`, `Voltage` **and**
`Power`, because `solve_current`'s three arms are three different closed forms and only
one of them is the obvious one to spike.

| mode | the empty cell | `is_linear()` |
| --- | --- | --- |
| 0 | presents a blocking resistance `R_block` for `i >= 0`, its ordinary `R0` for charge | false |
| 1 | its *open-circuit voltage* collapses to zero, keeping the RC overpotential (and so the heat term) intact; direction-blind, so still a fixed line over the step | **true** |
| 2 | mode 1's collapse on the discharge branch only | false |

### Mode 0 with `R_block = ∞`: the aggregation does not survive it

This was the crux question, and the answer is no. A zero-conductance arm makes a group's
`Σ 1/R_k` zero, and `(Σ E_k/R_k) / 0` is where the step goes wrong:

| case | first non-finite step | worst `solve_iterations` | `SOLVE_UNCONVERGED` steps |
| --- | --- | --- | --- |
| 1S1P `Current(+40)` | 22 | 32 (the cap) | 37 of 60 |
| 1S1P `Voltage` | 43 | 32 | 16 |
| 1S1P `Power` | 12 | 32 | 47 |
| 2S2P scatter, all three | 36–44 | 32 | 16–24, **and `NaN` heat** |

Terminal voltage `-inf`, the iteration pinned at its cap on every step past empty, and on
the scattered pack `q_gen_w` itself goes `NaN`. A strict diode does not fit this solve at
all, for a reason worth naming: the convergence test is *"each cell's terminal voltage
equals its group's node voltage"*, and an ideal blocking element is precisely an element
for which that is false.

### Mode 0 with a finite `R_block`: converges, and changes nothing that matters

At `1e12`, `1e6` and `1e3` the solve converges in **one pass** — the kinked curve never
limit-cycles, because both `source()` and `probe_at` take the blocked branch on the
discharge side, so the linearisation is exact where it is used. No `NaN`, no unconverged
step. And then:

| 1S1P, `R_block = 1e6` | delivered | stored | terminal V | `q_gen_w` peak |
| --- | --- | --- | --- | --- |
| `Voltage` | 457.8 As | 450 As | 2.50 | 12.5 W |
| `Power` | 471.3 As | 450 As | 1.47 | 112 W |
| **`Current(+40)`** | **1200.0 As** | **450 As** | **−4.0e7** | **1.6e9 W** |

`Voltage` and `Power` are *fixed*: the delivered charge falls to the stored charge plus
the entry-step remainder, which is the fraction the plan already reports and pins.
`Current` is not fixed at all — the fabrication is exactly what it was — and the terminal
voltage and heat are now catastrophic numbers that would drive a thermal network to
nonsense.

### The finding that resizes the whole item

`solve_current(Demand::Current(i), e, r)` returns `i`. It does not read `e` or `r`. **No
change to any cell model can refuse a demanded current**, because under a current demand
the current is not solved for — it is given.

That is the demand every "drive this pack flat" scenario in the repo uses. So the plan's
framing — *"the honest fix is for the cell to stop sourcing, which is a solve-side
change"* — is measurably wrong about what a solve-side change can achieve: it can change
the reported **voltage**, never the **current**. Refusing a demanded current means
changing what `Demand::Current` means with `bms: None`, which is the `CLAUDE.md` collision
the plan named, and it is not avoidable by making the cell nonlinear. The two costs are
not alternatives; the second is unconditional.

### Mode 1: closes 98 % of the energy hole for free, and breaks the charge direction

The one candidate that keeps `is_linear() == true` — so one pass, no iteration, no
allocation, no perf cost, no snapshot change. Energy ledger over the same 1S1P run, with
the chemical side taken from ground-truth stored charge:

| | baseline | mode 1 | instrument floor |
| --- | --- | --- | --- |
| `Current(+40)` | **−2244 J** fabricated | **+36.2 J** | 0.062 J |
| `Voltage` | −450.9 J | +40.9 J | |
| `Power` | −4650 J | +272 J | |

A 62× reduction, and the sign flips: the model stops *making* energy. The mechanism is
that the terminal voltage goes negative and the external circuit pays for the ohmic heat,
which is what cell reversal physically is.

It is not free, and the cost is on the other side of the clamp. `Voltage(3.3)` into an
empty cell draws **−180.5 A** where the correct answer is −30.5 A: a 0 V source under a
3.3 V demand looks like a dead short. A six-fold spurious charging spike is not a trade
worth taking silently.

### Mode 2: fixes that, and buys back the limit cycle

Applying the collapse only on the discharge branch restores the correct −30.5 A recharge
— and makes the cell nonlinear, and *then* the kink does exactly what a kink is supposed
to do to a tangent-retaking solve:

| 1S1P, mode 2 | `solve_iterations` | `SOLVE_UNCONVERGED` |
| --- | --- | --- |
| `Current(+40)` | 1 | 0 |
| `Voltage` | 32 (the cap) | 16 of 60 |
| `Power` | 32 | 47 of 60 |

This is the failure mode that was predicted for mode 0 and did not appear there. It
appears here, and the difference is instructive: mode 0's two branches agree on the
discharge side, so its linearisation is exact where the solve evaluates it; mode 2's do
not.

## One thing every candidate got right

The control — `soc = 0.5` at 4 A for 60 steps, which never clamps — is **bit-identical
across all five arms**, including the unpatched baseline, to 17 significant digits
(`elec 3.48055038170888963e2`, `heat 1.18828122419230127e1`). Whatever is eventually done
at the bottom of the window, it need not perturb a pack that stays inside it. The
`is_linear()` design is sound; it is the curve that has no good answer.

## Where this leaves the hole

Exactly where `energy-hole.md` left it, and now with a reason rather than a price:
`i_rejected_a` reports the fabricated charge and `properties.rs` pins the energy residual
at `OCV(0)·∫i_rejected_a dt`. That is a defect with a number on it and an invariant
around it.

What a real fix needs, in the order the measurement puts them:

1. **A decision about `Demand::Current` with `bms: None`.** Until a demanded current can
   be refused, the cell model is the wrong layer and no work there closes anything. This
   is a `CLAUDE.md` question, not an implementation one.
2. **A curve for an empty cell that is monotone, direction-correct, and continuous
   enough for a tangent iteration.** Mode 1 is direction-blind, mode 2 is discontinuous,
   mode 0 is a diode the convergence test cannot express. None of the three is it.
3. Only then the `is_linear()` change, which is the cheap part and the part that was
   priced.
