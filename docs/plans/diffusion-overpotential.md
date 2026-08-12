# A diffusion overpotential for lead-acid — can the ECM carry Peukert at all?

2026-08-12. `lead-acid-data-only.md` ended by pricing a lead-acid **phase**: the missing
mechanism is "a **diffusion overpotential** — a voltage term that grows under sustained
current and relaxes at rest", it is per-cell state, and it therefore costs a
`SNAPSHOT_VERSION` bump and a migration. That pricing was written from the *diagnosis*
(ohmic sag is flat-then-knee, Peukert is a power law), not from any model of the proposed
replacement. A phase should not be started on a mechanism nobody has done the arithmetic
for.

So this document does the arithmetic before the code, on the terms the previous slice set:
**everything below is a hand model run against the shipped parameter file, with no Rust
written, no snapshot version moved and no chemistry file changed.** The question is not
"how should the term be tuned" but the prior one — **can a diffusion overpotential added to
today's ECM reproduce Peukert's shape at all?**

The hand model is `M:\claud_projects\temp\diffusion\`. It is validated before it is
trusted: with the diffusion term switched off it reproduces the shipped engine's rate sweep
to within 0.3 points at every rate (its 25.9-point worst error against the engine's
measured 25.7) — the same agreement `lead-acid-data-only.md` recorded between its own hand
model and the engine.

---

## The answer

**Yes, and the parameter that makes it work is the one a rate sweep is least likely to
look at.** A single extra `f64` per cell, advanced by the exact exponential update the RC
pairs already use, tracks Peukert n = 1.1 across 0.05C → 3C to **3.8 points worst
leave-one-out error**, against **25.9 points** today. It produces a genuine power law
rather than the right average slope (deviation from its own best power law: **1.9 points**
against today's 7.8). And **rest recovery falls out for free** — after a hard discharge to
cutoff and a four-hour rest, the cell delivers 22–29 % more, which is the acceptance test
`lead-acid-data-only.md` named and never scored.

**But the first three searches said no**, and they were wrong for a reason worth recording
rather than deleting. That story is below the results, because it is the part most likely
to be repeated.

---

## What the mechanism is

One state per equivalent-circuit cell — call it the **depletion** `D`, the shortfall of
acid at the reaction site relative to the bulk:

```text
dD/dt = i/q_d − D/τ_d          exact exponential update, as rc_update already does
η     = −k · ln(1 − D/(D_lim · soc))
```

Three things about the shape, each load-bearing:

* **The denominator carries `soc`.** The limiting flux a lead-acid cell can sustain is set
  by how much acid is left, and the acid is a *reactant* — so the cell's ability to supply
  current falls as it empties. Without this the term is an extra resistance and reproduces
  nothing (see "the three forms that fail").
* **The logarithm is Nernstian in form**, `η = (RT/nF)·ln(c_bulk/c_surface)`, which is what
  a concentration difference costs in volts. `k` is fitted rather than set to `RT/nF`, and
  the fitted value is *not* thermodynamic — see the honesty section.
* **`D` is read from the previous step**, exactly as `soc_deficit` is
  (`docs/plans/low-clamp-reversal.md`). Within a step the cell is still a fixed line, so
  `CellModel::is_linear` stays `true`, the pack solve stays one closed-form pass, and
  nothing here allocates or iterates. The non-linearity is spread across steps rather than
  packed inside one.

## The results, checked

A fit reported at the rates it was fitted on measures itself, so every headline here is a
**hold-out**. Three designs, because the first one chosen was too loose and saying so is
part of the result:

| hold-out design | what it fits | worst **held-out** error |
| --- | --- | --- |
| endpoints only (0.05C, 3C) | one ratio, three parameters | 6.0 pts |
| fit 4 (0.05, 0.2, 1, 3C), hold 3 | four rates | **1.9 pts** |
| leave-one-out, each rate in turn | six rates | **3.8 pts** |
| — *today's model, same measure* | — | *25.9 pts* |

**Leave-one-out at 3.8 points is the number to quote.** It is the most conservative of the
three that constrain the fit properly, and it is honest in the way the endpoints-only run
is not: **scoring two rates cannot pin three parameters**, so that run landed in a far
corner of the degenerate valley and its 6.0 points measured the under-constraint, not the
mechanism. Fitted on all seven rates the model reaches 1.8 points, which is the
self-measuring number and is quoted here only to bound the others.

Leave-one-out in full:

| held-out rate | model | Peukert n = 1.1 | error | `τ_d` the other six chose |
| --- | --- | --- | --- | --- |
| 0.10C | 95.2 % | 93.3 % | +1.9 | 1.98 h |
| 0.20C | 85.6 % | 87.1 % | −1.4 | 3.35 h |
| 0.50C | 76.8 % | 79.4 % | −2.6 | 1.98 h |
| 1.00C | 75.4 % | 74.1 % | +1.3 | 3.35 h |
| 2.00C | 71.3 % | 69.2 % | +2.2 | 2.57 h |
| 3.00C | 62.6 % | 66.4 % | −3.8 | 2.57 h |

**The errors change sign across the sweep**, which is what a fit tracking a curve looks
like; a same-signed run is the signature of a missing parameter, and it is what the failed
searches below all produced. The last column is the useful surprise: **six independent fits,
each blind to a different rate, agree on the relaxation time to within a factor of 1.7**
(1.98–3.35 h). Properly constrained, the valley is much narrower than the endpoints-only
run made it look.

### Rest recovery, which nothing in the fit scored

The mechanism's real acceptance test, and it was never part of the objective:

| | rest 0 h | 1 h | 4 h | 24 h |
| --- | --- | --- | --- | --- |
| after a 1C discharge to cutoff | +1.2 % | +10.4 % | +21.7 % | +25.5 % |
| after a 3C discharge to cutoff | +8.2 % | +17.0 % | +28.9 % | +31.9 % |

A hard discharge to cutoff, a rest, and a second discharge that delivers materially more —
met by a wide margin, with the right ordering (a harder discharge recovers more) and the
right timescale (hours, not minutes). The +1.2 % at zero rest is the RC pair relaxing, not
the new term.

### Four checks, because a verdict-flipping result gets checked rather than quoted

1. **Timestep.** At 10× finer `dt` the worst error moves 1.86 → 1.83. Reported as a
   *bracket*, not a `dt` vs `dt/4` comparison — the cutoff-crossing omission scales with
   `dt`, so both runs are wrong in the same direction and agree with each other
   (`lead-acid-data-only.md`'s own recorded trap). Bracket width is 0.0004 Ah at every rate.
2. **The saturation guard is never reached.** The `log` argument goes non-positive if `D`
   ever reaches `D_lim·soc`; the code has a branch for it; **it does not fire at any rate.**
   The largest overpotential reached is 0.22 V, well below the divergence. So the fit rests
   on the mechanism, not on a numerical guard — which is the specific way this tree has been
   burned before ("a guard documented as numerical was load-bearing physics").
3. **`τ_d` is sharply determined, and my first reading of it was wrong.** The coarse search
   appeared to saturate above ~7700 s. It does not: holding `k` and `D_lim` fixed, halving
   `τ_d` costs 13.8 points and doubling it costs 12.6, against 1.8 at the fit. The apparent
   saturation was the search quietly re-fitting the other two parameters at each `τ_d`.
4. **The parameters are degenerate, but far less than the first check suggested.** The
   all-rates fit lands at `k = 0.114 V, D_lim = 1.23, τ_d = 2.1 h`; the endpoints-only fit
   at `k = 0.433 V, D_lim = 1.41, τ_d = 5.0 h` — a 3.8× spread in `k` that looked alarming.
   It was an artifact of the under-constrained objective: **once four or more rates are
   scored, every fit lands at `k = 0.08–0.11 V` and `τ_d = 2.0–3.4 h`.** The mechanism is
   identifiable; two points on a capacity-versus-rate curve simply are not enough to
   identify it, which is an argument about the *fitting procedure* rather than about the
   model.

---

## How the first three searches said "no", and why

This is the part worth keeping. The first search grid returned **nothing better than 13.7
points**, and the analysis behind it was clean, quantitative, and pointed at a stop.

### The demand table, which is still true

For each rate, take the ending SOC that Peukert requires, read the chemistry's own OCV
there, subtract the cutoff and the ohmic drop the engine already produces. What is left is
the sag any additional term must supply:

| rate | required soc_end | OCV there | ohmic + RC | **extra sag needed** |
| --- | --- | --- | --- | --- |
| 0.05C | 0.0000 | 1.9500 | 0.0069 | **0.1931** |
| 0.10C | 0.0670 | 1.9647 | 0.0110 | **0.2038** |
| 0.20C | 0.1294 | 1.9785 | 0.0174 | **0.2111** |
| 0.50C | 0.2057 | 1.9952 | 0.0337 | **0.2115** |
| 1.00C | 0.2589 | 2.0068 | 0.0657 | **0.1911** |
| 2.00C | 0.3085 | 2.0167 | 0.1281 | **0.1386** |
| 3.00C | 0.3360 | 2.0222 | 0.1893 | **0.0829** |

**The requirement peaks at 0.5C and then falls by more than half.** An overpotential that
grows with current cannot satisfy a requirement that shrinks with it. That reads like a
proof of impossibility. It is not, and the reason is in the fine print of its own first
column: it holds `soc_end` fixed at what Peukert requires *of a reference that delivers the
full nominal capacity*. Once the new term is strong enough that the C/20 reference discharge
also ends on voltage, the reference moves, every entry in that column moves with it, and the
table no longer describes the problem being solved. **It is a correct calculation of the
wrong quantity**, and it is exactly the kind of argument that is most convincing when it is
misleading.

Underneath it is a structural fact that *is* true and is worth carrying forward:
`OCV(soc = 0)` is **1.950 V** against a `v_min` of **1.750 V**, so this cell has 200 mV of
headroom before a single amp-hour is lost, and its C/20 discharge today ends on the charge
clamp with its terminal still near 1.94 V. Both numbers are right — a flat lead-acid cell
really does rest near 1.95 V, and C/20 capacity really is quoted to 1.75 V. What sits
between them in a real cell is ≈ 195 mV of polarization present *even at C/20*, where the
ohmic drop is 5 mV. **The missing physics is real and it is large; it is simply not shaped
like a correction to the end of a discharge.**

### The three forms that fail, and the one that does not

All four carry the same state and differ only in what a given `D` costs in volts:

| | form | best worst-error |
| --- | --- | --- |
| — | **today**, no diffusion term | 25.9 pts |
| A | `k·ln(1 + D/D₀)` | 20.6 |
| B | OCV read at `soc − D` (a surface state of charge) | 25.9 — **no effect at all** |
| C | `−k·ln(1 − D/D_max)` (Nernst, fixed limit) | 18.8 |
| D | `−k·ln(1 − D/(D_lim·soc))` (limit falls with the acid left) | **6.0 held out** |

A, B and C all leave the 0.05C/0.1C/0.2C errors at **exactly** their no-diffusion values
(+0.0/+6.7/+12.9): none can lose any capacity below 0.5C. Form B — the most physically
appealing, since acid depletion really is a local state of charge — does *nothing*, because
the OCV table's slope near empty is 0.22 V per unit SOC, so shifting the lookup by `D` buys
0.22·`D` volts and reaching 0.2 V would need `D` ≈ 0.9.

### The three objections, tested and disposed of

Each is the "you just chose the wrong number" reply, and each is answered the way
`lead-acid-data-only.md` answered the same reply about `R0`:

1. **"`R0`'s rise toward empty is a placeholder."** Scale every resistance by a free factor
   and refit. At **0.00×** — the ohmic path deleted entirely — the best quasi-static fit
   improves from 13.6 to **11.6 points**. Two points for removing all of it.
2. **"The engine has one fixed cutoff; real ratings lower it at high rate."** True, and the
   parameter file already flags it. With a datasheet-shaped schedule (1.75 V at C/20 falling
   to 1.60 V at 3C): **13.5 → 11.6 points.** Same two points. Note separately that the
   schedule *alone*, with no diffusion term, makes things **worse** (30.9 points) — it hands
   capacity back at exactly the rates that already have too much.
3. **"Try harder on the functional form."** Free exponents (`K·i/soc^p`,
   `−k·ln(1 − i/(i_lim·soc^q))`, `A·i^m/soc^p`) do reach 2.5–3.8 points, but **every one
   drove the SOC exponent to the top of whatever grid it was given** (12, 9 and 10, on grids
   capped at 12, 10 and 10). That is not a mechanism converging; it is a fit asking for a
   step function at a fixed charge level — a cutoff wearing a mechanism's name.

### The mistake: an approximation adopted for speed deleted the answer

The first three searches solved for the ending SOC by **bisection on a quasi-static
condition** — assume `D` has reached `D_ss = i·τ_d/q_d` and root-find. It made a real grid
search affordable (a full time-stepped C/20 discharge is tens of thousands of steps) and it
is a fair approximation at C/20, where any plausible `τ_d` is short against twenty hours.

**It is not fair at 3C, where the discharge lasts seventeen minutes — and it does not merely
add error, it removes `τ_d` from the problem entirely.** Under the quasi-static assumption
`τ_d` enters only through the ratio `τ_d/q_d`, so the parameter that turned out to decide
the whole question was not being varied at all. Every fit was then too steep at high rate,
which is precisely the signature of assuming a slow state had saturated when it had not.

The tell was visible and was misread: every candidate came out too flat below 0.5C **and too
steep above 2C**, and a systematic same-signed error at one end of a sweep is a missing
parameter, not a wrong functional form. The conclusion drafted from those three searches was
"no, and the blocker is upstream" — with a demand table to prove it. Restoring `τ_d` moved
the worst error from 13.5 to 1.8 on the same functional form.

**The general shape of this trap: an approximation adopted for tractability silently
collapsed two parameters into one, and the collapsed one was the answer.** Nothing about the
search reported that; it reported a clean, monotone, believable "no".

---

## What this prices, if the phase is wanted

* **One new `f64` per equivalent-circuit cell** in `EcmState`, plus one new optional
  chemistry section `[diffusion]` with three constants. `SNAPSHOT_VERSION` 16 → 17, a
  migration, and `sim-wasm`'s own constant checked **against its own doc** rather than
  assumed to move in step (`docs/plans/ui-bms-view.md` — they have parted before).
* **LFP and NMC must not move a bit.** No `[diffusion]` section means the term is absent,
  and the no-section path should be an **early return** rather than a multiply by a neutral
  value, exactly as `open_circuit_v` handles a zero deficit. That is a registrable
  prediction: every existing golden and every existing trajectory identical.
* **A decision this forces, and it belongs in the fitting objective rather than in a
  meeting.** With the term active the C/20 reference discharge ends on voltage at
  `soc ≈ 0.05`, so the cell delivers **≈ 95 % of the `capacity_ah` its file declares**
  rather than 100 %. That is physically right — 7.2 Ah is the coulombic capacity and a real
  cell's C/20 rating is *defined* to a 1.75 V cutoff. But note that the figure **moves with
  the fit**: the under-constrained endpoints-only run gave 91 %, the properly constrained
  ones give 95.2 %. So "what does `capacity_ah` mean" cannot be settled by fiat beforehand
  and then fitted around — **the C/20 delivered capacity has to be one of the things the fit
  targets**, or the answer is whatever the objective happened to imply.

  The blast radius is smaller than that warning suggests, and was measured rather than
  assumed: **`pba_agm_2v_generic` is referenced by exactly one file in the tree**
  (`crates/sim-data/tests/lead_acid_rate.rs`), and its only absolute assertion is on the
  *declared* `capacity_ah` of 7.2, which does not move. Every capacity figure it checks is a
  **ratio against its own C/20 reference**, so the sweep's assertions survive a reference
  that shifts. What changes is the prose describing what the model gets wrong, which is the
  point of the slice.
* **Provenance for three constants, and one of them cannot be dressed up.** `τ_d ≈ 2–5 h`
  is defensible for lead-acid acid diffusion and is independently checkable against
  rest-recovery data. `D_lim ≈ 1.2–1.4` reads as "the C-rate this cell could sustain at full
  charge" and is physical. **`k` is fitted, is 0.11–0.43 V, and is one to seventeen times
  `RT/F` = 25.7 mV — so it must not be labelled as a thermodynamic constant.** It absorbs
  the electrode kinetics this one-state model does not carry.
* **Not NiMH, and not resting-voltage memory.** Those need a different state (a hysteresis
  term) and should be their own slice with their own version bump. A dead field costs more
  than a second small migration.

## Deliberately not done here

* **No Rust, no chemistry file change, no version bump.** The deliverable is the answer to
  whether the phase is worth starting.
* **Not fitted properly.** Three parameters against a seven-point sweep, with a demonstrated
  degenerate valley. A real parameterisation should fit against a family of full discharge
  curves, not a capacity-versus-rate table, and the rest-recovery numbers above should be a
  *check* rather than an output.
* **Charge accounting untested.** The term is a voltage, so `∫I·dt = ΔSOC × capacity` should
  be untouched by construction — but "should be by construction" is what the property test
  exists to stop anyone asserting. It gets run, not assumed.
* **Thermal held fixed**, as in the previous slice, so a rate effect is not read off a curve
  that is partly self-heating.

---

# Landed — 2026-08-12

**`SNAPSHOT_VERSION` 16 → 17.** Three commits, each a green gate, staged so that the one
claim most worth being able to disprove — *no chemistry without the section moves by a
ULP* — is checkable in isolation rather than tangled with the physics that motivated it:

1. the `depletion` field, the `[diffusion]` section, the version bump and the no-section
   early return, **with no physics wired** — 492 tests, none of them different;
2. the term wired into `cell_source`, `heat_w` and `overpotential_v`, with **still no
   chemistry declaring the section** — 492 tests, still none of them different, which is
   the sharpest test the `None` path will ever get;
3. the fit, the chemistry file, and the tests — 503 tests, and only `lead_acid_rate.rs`
   and the new `diffusion.rs` moved.

`sim_server::API_VERSION` stays at 2 and `sim-wasm`'s at 6. Both were read from their own
docs rather than moved as a set, and both notes are extended in place — the interesting
one is the server's, because this is a case its rule does not quite reach:
`overpotential_v` is neither renamed nor added, but the *number it carries* now has a
second contributor. Nothing owed, by the letter and by the purpose, and recorded so the
next reader finds it decided.

## What the engine measures

Every figure below is the **engine's**, from `crates/sim-data/tests/lead_acid_rate.rs`,
not the fitting harness's. The two agree to 0.1 points, which is the check that the hand
model was worth trusting; they are not the same arithmetic (the engine reports its terminal
from end-of-step state where the harness tested it at the start of one).

| rate | shipped | Peukert n = 1.1 | error | **control arm** | its error |
| --- | --- | --- | --- | --- | --- |
| 0.05C | 100.0 % | 100.0 % | +0.0 | 100.0 % | +0.0 |
| 0.10C | 96.6 % | 93.3 % | **+3.3** | 100.0 % | +6.7 |
| 0.20C | 90.2 % | 87.1 % | +3.1 | 100.0 % | +12.9 |
| 0.50C | 78.8 % | 79.4 % | −0.6 | 99.9 % | +20.5 |
| 1.00C | 72.6 % | 74.1 % | −1.5 | 99.9 % | **+25.7** |
| 2.00C | 67.8 % | 69.2 % | −1.4 | 92.7 % | +23.6 |
| 3.00C | 63.4 % | 66.4 % | −3.0 | 83.8 % | +17.4 |

**25.7 points to 3.3.** The control arm is the shipped file with `[diffusion]` deleted —
i.e. the model exactly as it was — and it is *run*, not remembered. Every finding the
previous slice paid for survives as a live measurement beside its replacement, which makes
the comparison a subtraction rather than a claim, and makes the stripped arm a direct check
that the engine's no-section path is the old path rather than something close to it.

### Rest recovery, still unscored, now measured in the engine

Second discharge to cut-off as a fraction of the first, after a rest:

| | 0 h | 1 h | 4 h |
| --- | --- | --- | --- |
| after a 1C discharge | 0.0 % | 16.5 % | **26.2 %** |
| after a 3C discharge | 0.0 % | 22.0 % | **32.2 %** |
| *control arm, 3C* | — | — | *7.0 %* |

Never in the objective, right in ordering (a harder discharge recovers more), right in
timescale (hours), and four times what the RC pair alone produces. The zero-rest column is
0.0 % by construction rather than by physics — with no rest the cell is still below its
cut-off and the second run ends on its first step — and it is kept because it says plainly
that all of the recovery is the rest.

## Three things the paper study could not have found

1. **Check #2 was scoped to a harness that could not reach the case.** The plan reported
   that the saturation guard "does not fire at any rate", measured on a loop written
   `while soc > 0`. The engine has no such loop: `soc` genuinely arrives at `0.0`, the
   limit `D_lim·soc` is zero there, and **any** depletion saturates. So the claim is
   re-scoped to what was actually measured — *nothing in the swept range comes near it*,
   0.233 V against 1.950 — and re-measured in the engine, where the test now asserts the
   sweep stays inside a fifth of the ceiling. That is the load-bearing form: if the ceiling
   bound inside the sweep, the rate fit would be resting on a declared limit rather than on
   the mechanism, which is precisely the failure `phase-7-dfn.md` records.

2. **A fourth constant, and the derivation is what makes it not a fudge.** Something has to
   answer at `soc = 0`, and a bare number in the physics is the
   guard-documented-as-numerical shape this tree has already paid for once.
   `max_overpotential_v` is therefore chemistry data with provenance — and it is
   **derived**: `OCV(0) − reversal.floor_v`. At that value a saturated cell at rest sources
   exactly `floor_v`, which is where the reversal ramp puts a cell driven the whole way past
   empty. The two ways this engine can collapse a cell now agree on where the bottom is
   instead of disagreeing by an arbitrary amount.

   The consequence is worth stating rather than leaving to be discovered: **a chemistry with
   this section treats `soc = 0` as the end of the cell**, where one without it keeps
   sourcing at `OCV(0)`. Charging recovers it on the first step that lifts `soc` off zero.

3. **The `0/0`.** At `soc = 0` with a depletion of exactly zero the ratio is `NaN`, a bare
   `x >= 1.0` guard answers *false* for it, and the `NaN` reaches the cell's Thévenin source
   and from there every sibling sharing the node — no panic, no flag, no failing test
   anywhere. `diffusion.rs` walks a scattered 2S3P pack down through empty, rests it there
   for eight hours, and charges it back out, checking finiteness at every step of all three
   legs. (In the shipped arrangement the early return on `depletion == 0.0` reaches the
   rested case first, so the `NaN` is currently unreachable rather than merely handled. The
   guard stays: "unreachable today" is a fact about the caller, not about the function.)

## What the anchor changed, and the number it cannot reach

The plan's last open question was that the C/20 delivered capacity "moves with the fit"
(91 % under-constrained, 95.2 % properly), so it had to be **in the objective** rather than
read off afterwards. It is. The objective is now one number in one unit with no weights:

```text
worst | error in points of capacity | over
    {  100·(delivered(C/20)/7.2 − 1)                           <- the anchor
    U   100·(delivered(r)/delivered(C/20) − peukert(r))  for each rate r  }
```

Both terms are already percentages of a capacity, so they need no invented relative weight.
The fit lands at **`τ_d = 4218 s`, `D_lim = 1.59`, `k = 0.0665 V`**, and the anchor at
**96.7 %** of the declared 7.2 Ah.

**96.7 % is a ceiling, not a shortfall of effort, and the algebra was written before the
search ran.** `η` diverges as `soc → 0` at any current, so the cell always trips a little
short of empty:

```text
delivered(C/20)/7.2  <=  1 − (0.05/D_lim) / (1 − e^(−headroom/k))
```

At the fitted parameters that predicts **96.68 %** and the search reached **96.72 %** — a
pre-registered prediction confirmed to 0.04 points, and the reason the residual is reported
as a model error rather than tuned at. `capacity_ah` is therefore the **coulombic** capacity
here and the datasheet's 7.2 Ah is a *delivery*; the 3.3 % between them is what this
mechanism cannot close. The `[cell]` block keeps 7.2 so that "1C" means what a user of a
7.2 Ah battery means by it.

### The hold-out went degenerate, and that is a result rather than a gap

Leave-one-out was the plan's headline instrument (3.8 points). With the anchor in the
objective it stops measuring what it measured: **all six subsets select the same
parameters**, to every figure, so the held-out errors are just the full fit's errors and
the worst is 3.28 — the same number as the fit. The anchor is a hard constraint that no
single rate can move, and once it is scored, *no individual rate is load-bearing*. That is
a stronger statement than "generalises to 3.8 points" and a different one, so it is quoted
as what it is rather than as a hold-out number.

The fit was also run twice, by two implementations — the reviewed one and a second with the
two SOC lookups sampled onto a table because the original re-interpolated `R0` five times
per step and the grid is 207 million steps wide. They select the same parameters and report
the same errors to five figures.

## What did not move, checked rather than predicted

* **Every LFP and NMC trajectory, bit for bit**, goldens included — the `None` arm of
  `ecm_overpotential_v` is a match arm returning `Σ V_rc`, not a multiply by a neutral
  value, so this is structural. Gate 2 above is the isolated proof.
* `Cell` grew 176 → 184 bytes, `EcmState` 40 → 48. Priced in `pack.rs`'s footprint test
  rather than discovered later; the model-slot budget is stated as a *relation* to
  `EcmState`, which is the property that let it absorb the change without an edit.
* `cell_heat_w`'s third parameter is renamed `v_rc_sum` → `overpotential_v`, because that
  is what it now is. The heat carries the **same start-of-step** value the voltage did — a
  term recomputed from end-of-step state would drift the ledger by `∫i·η dt`, in one
  direction, and nothing else would notice. `diffusion.rs` closes a full discharge/charge/
  rest cycle to catch exactly that.

## Deliberately not done

* **Aging does not reach the diffusion parameters.** It reaches the term only through the
  capacity — a faded cell reads the same current as a higher C-rate — and deliberately not
  through `soh_resistance`, which grows `R0` and the RC pairs. An aged lead-acid cell really
  does have worse rate behaviour than a fresh one; the coefficient that would express it is
  one nobody has fitted, so the omission is stated in the code rather than papered with a
  plausible number.
* **The charge direction is unvalidated.** `D` goes negative on charge and the same
  expression returns a negative `η`. The sign is right, the magnitude is bounded and
  logarithmic, and `diffusion.rs` pins both — but nothing measured it, and the field doc
  says so rather than letting the form's plausibility stand in for data.
* **Not thermal.** The sweep is isothermal, as the previous slice's was — and the reason
  is now measured rather than estimated, because the estimate was wrong by a factor of
  four. At 3C the term raises the **peak** heat by 1.20x and the **mean** by 1.07x, and
  *lowers* the total over a discharge (2709 J against 3457 J) because it ends the run
  sooner — 736 s against 1008 s, so the cell never reaches the low-SOC region where `[r0]`
  is highest and spends less time making heat at all. A draft of this bullet said "roughly
  doubles". Three quantities that disagree, so prose has to pick one, and
  `the_term_adds_heat_and_the_peak_is_not_the_average` now asserts the ordering between
  them so that "a hotter cell" cannot be written here.
* **`[r0]`'s rise toward empty is still a placeholder** and should now be fitted to what it
  actually is — the instantaneous ohmic drop — rather than to a rate curve it was never the
  right shape for.
* **Not reachable from any client.** The chemistry is loadable by id, as it was before, and
  no scenario, page or catalogue entry uses it. Presenting this cell in the guided path is
  a UI slice; it is now worth doing, which it was not before.
  **Done — `docs/plans/lead-acid-client.md`.** `scenarios/cc_discharge_pba.toml` and three
  guided-path steps. The slice is unusual in having no asymmetry to confess: both of its
  rate arms and its rest recovery were already fenced by `lead_acid_rate.rs`, so unlike the
  porous-model slices it quotes no number a committed test does not bound. It also found
  that the pack's `overpotential` tile is **not** a clean read of this mechanism during a
  discharge — 82.39 mV of the 184.29 mV at the 3C cut-off is the placeholder RC pair — and
  that the clean window is the *rest*, where the RC pair is spent at half an hour and the
  fitted term still has 47.66 mV to give.
* **Not NiMH, and not resting-voltage memory.** Those need a hysteresis state and their own
  version bump. A dead field costs more than a second small migration, and that judgement
  is unchanged.
