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
pairs already use, tracks Peukert n = 1.1 across 0.05C → 3C to **6.0 points worst error on
rates the fit never saw**, against **25.9 points** today. It produces a genuine power law
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

Fitted on the **two endpoints only** (0.05C and 3C) and reported at the five interior rates
the fit never saw. Peukert itself has two free parameters, so this is the comparison at
matched cost:

| rate | model | Peukert n = 1.1 | error | |
| --- | --- | --- | --- | --- |
| 0.05C | 100.0 % | 100.0 % | +0.0 | *fitted* |
| 0.10C | 93.9 % | 93.3 % | **+0.6** | held out |
| 0.20C | 88.6 % | 87.1 % | **+1.5** | held out |
| 0.50C | 83.7 % | 79.4 % | **+4.3** | held out |
| 1.00C | 80.1 % | 74.1 % | **+6.0** | held out |
| 2.00C | 73.7 % | 69.2 % | **+4.6** | held out |
| 3.00C | 66.4 % | 66.4 % | −0.0 | *fitted* |

**Worst held-out error 6.0 points, against 25.9 for the model as it ships.** Fitted on all
seven rates instead it reaches 1.8 points, and that number is *not* the one to quote,
because a fit reported at the rates it was fitted on measures itself.

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
4. **The parameters are degenerate along a valley.** The all-rates fit lands at
   `k = 0.114 V, D_lim = 1.23, τ_d = 2.1 h`; the endpoints-only fit at
   `k = 0.433 V, D_lim = 1.41, τ_d = 5.0 h`. Both fit their own objective; they are not the
   same cell. **Two rates do not pin this mechanism** — the interior of the curve carries
   real information, which is an argument for fitting against a full discharge family rather
   than a headline number.

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
* **A decision this forces, and it is not small.** With the term active the C/20 reference
  discharge ends on voltage at `soc ≈ 0.05–0.09`, so the cell delivers **91–95 % of the
  `capacity_ah` its file declares** rather than 100 %. That is physically right — 7.2 Ah is
  the coulombic capacity and a real cell's C/20 rating is *defined* to a 1.75 V cutoff — but
  it changes what `capacity_ah` means for this chemistry and it will move any test that
  assumes a full discharge delivers the declared number. **This has to be settled before the
  slice, not during it.**
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
