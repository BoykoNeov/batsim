# Phase 13, the spike — what the porous models do when a cell is charged on past full

**Status: SPIKE RUN 2026-10-20 — a census, no predictions registered (it counts what happens; it
tests no design). No Rust in the tree. The harness lives outside it
(`W:\temp\claude\phase13-spike\engine\`, a binary against the tree at 31916d5; per-run traces in
`W:\temp\claude\phase13-spike\out\*.csv`, summaries in `out\census2.txt`); only this note is
committed. Next: the owner's decisions below, then the phase plan.** The phase is ROADMAP H8's
"no physics past FULL", chosen by the owner 2026-10-20 as Phase 13 (`docs/ROADMAP.md` §3).

## The question

ROADMAP H8 records that once a step drives a particle surface past `c_max`, the `Spm` and the `Dfn`
clamp it and their curves go flat, that `SURFACE_OUT_OF_RANGE` flags it, and that "neither gives it
physics". Before designing anything this spike asks: past full, where does the charge go in each
porous model, does the solve keep up, and what does the cell's temperature do — with the
equivalent circuit beside it as the contrast, since its upper clamp refuses the charge and turns
it into heat (`energy-hole.md`).

"Past full" is three different things, kept apart here because one fix has already failed by
covering two of them (`dfn-cold-hold.md`):

* **(a) Gross overcharge** — a constant current pushed on after the cell is full, BMS off. This
  census.
* **(b) The cold 4.2 V hold** on the `Dfn` — a surface creeping ~1e-6 of `c_max` past full during
  the hold, and the discharge after it stalling on the clamp's kink (`dfn-cold-hold.md`). Not
  re-measured; its numbers are that note's.
* **(c) The positive electrode filling at the `Dfn`'s rate limit** on a hard discharge
  (`dfn-electrolyte-limit.md`). A different edge, a different missing reaction; **out of scope**.

## The runs

* One cell (1S1P), BMS off, `Demand::Current` at −1C or −3C from `initial_soc` 0.8, 1 s steps, for
  as long as it takes to push in one full capacity (so 0.8 of a capacity past full). A thermal
  network (`k_neighbor` 0, the cell's own `h·A`) at a 25 °C ambient, or 0 °C where named.
* Shipped `nmc_21700_lgm50` on the equivalent circuit, the `Spm` (`DEFAULT_SHELLS`), the `Dfn`
  (recommended grid; the fine grid once) with the threshold and with `PlatingModel::Reaction`;
  shipped `lfp_26650_prada2013` on the many-particle cell (20 particles, 20 shells, σ 0.2) and on
  the equivalent circuit.
* Read from the committed state through the snapshot: each electrode's outer shell (standing in
  for the surface) and particle mean, as fractions of `c_max` — the negative's maximum, the
  positive's minimum. "Ah past full" counts from the first step `soc_true` reaches 1.

## What it found

| run | V peak | where the charge past full went | neg reaches `c_max` (Ah past full, surface / bulk) | pos reaches 0 (surface / bulk) | end state | T peak | ignites |
| --- | ---: | --- | --- | --- | --- | ---: | --- |
| circuit, LG M50 1C | 4.49 V | refused, 4.12 A·h, as heat | — | — | — | 1001 °C | 1117 s |
| circuit, LG M50 3C | 4.91 V | refused, as heat | — | — | — | 1284 °C | 330 s |
| `Spm` 1C | 5.22 V | **all of it into the particles** | 0.46 / 0.52 | 1.95 / 2.31 | neg **1.62** `c_max`, pos bulk **−0.21** | 81 °C | no |
| `Spm` 3C | 5.32 V | all into the particles | 0.39 / 0.52 | 1.52 / 2.31 | neg 1.62, pos −0.21 | 157 °C | no |
| `Dfn` threshold 1C | 5.79 V | all into the particles | 0.46 / 0.46 | 2.01 / 2.28 | a neg node **1.79**, pos −0.22 | 129 °C | no |
| `Dfn` threshold 3C | 6.53 V | all into the particles | 0.40 / 0.42 | 1.72 / 2.18 | a neg node 1.83, pos −0.25 | 1001 °C | **777 s** |
| `Dfn` reaction 1C, 25 °C | 5.81 V | **3.51 A·h plated**, 0.56 into the negative | 0.58 / 0.65 | 1.91 / 2.26 | neg 1.0097, pos −0.22 | 110 °C | no |
| `Dfn` reaction 1C, 0 °C | 5.81 V | 3.52 plated, 0.56 into the negative | 0.72 / 0.82 | 1.76 / 2.23 | neg 1.0077, pos −0.22 | 85 °C | no |
| `Dfn` reaction 1C, fine grid | 5.81 V | 3.51 plated, 0.56 into the negative | 0.54 / 0.61 | 1.89 / 2.25 | neg 1.0092, pos −0.22 | 110 °C | no |
| many-particle LFP 1C | 4.12 V | all into the particles | 0.25 / 0.55 | **0.00 / 0.00** | neg 1.44, pos **−0.77** | 31 °C | no |
| many-particle LFP 3C | 4.23 V | all into the particles | 0.00 / 0.55 | 0.00 / 0.00 | neg 1.44, pos −0.78 | 43 °C | no |
| circuit, LFP 1C | 3.67 V | refused, 1.84 A·h, as heat | — | — | — | 49 °C | no |

(LG M50 capacity 5.15 A·h, LFP 2.30 A·h; a full push past full is 4.12 and 1.84 A·h.)

1. **Every porous model stores lithium its particles cannot hold.** Without the plating reaction,
   every amp-hour pushed past full went into the particles: the `Spm`'s negative ends at 1.62 of
   `c_max`, a `Dfn` node at 1.79–1.83, the LFP cell's negative at 1.44; and the positive, which
   gave that lithium up, at **negative** concentrations (−0.21 to −0.78 of `c_max` in its bulk). It
   is the charge side of what `porous-reversal.md` closed on the discharge side (the old `Spm`
   reached −1.26 of a stoichiometry there). The lithium book "closes" only because it counts
   impossible stock.
2. **The space between full and the particles' limits is real, and the models already get it
   right.** The cell's "full" is the chemistry's window edge, not a particle limit. On LG M50 the
   negative reaches `c_max` 0.39–0.82 A·h (8–16 % of capacity) past full and the positive reaches
   empty 1.5–2.3 A·h past it, so the first ~0.4 A·h of an LG M50 overcharge is physically possible
   and not a defect. A design that refuses charge at `soc = 1`, as the circuit does, would throw
   that away; the impossible part starts at `c_max` and at 0.
3. **On LFP the positive is the first limit, and it is reached at full.** The many-particle cell's
   positive is empty (to the third decimal) the moment the cell reads full: the file's
   `stoich_min` is 0.0038. So on LFP any charge past full is impossible at once, and the negative
   never gets to be the first limit — plating cannot be its outlet.
4. **The plating reaction is an outlet on the negative only, and only where it runs.** On the
   `Dfn` with `PlatingModel::Reaction`, plating took 3.51 of the 4.12 A·h past full (85 %), the
   same on the fine grid and at 0 °C; the negative still creeps 0.8–1 % past `c_max`. But every
   plated atom still came out of the positive, which went below empty exactly as on the threshold
   cell (−0.22). **So every porous model, on every chemistry, needs an outlet at the positive**,
   and nothing in the repo provides one.
5. **Whether a porous cell ignites on overcharge is decided by an artifact.** With the BMS off, the
   circuit cell ignites at 1C 1117 s into the run (charge refused at its `OCV(1.0)`, ~21 W of heat). The
   `Spm` never does (81 °C at 1C, 157 °C at 3C; onset is 170 °C). The `Dfn` does at 3C, at 777 s
   and 1001 °C — but by resistive heat at 6.5 V on a state holding 1.8 times the lithium its
   particles can. `CLAUDE.md` requires that an overcharge with the BMS off be able to reach
   runaway; on the porous models it does or does not for a reason that is not physics, which is
   `low-clamp-reversal.md`'s "emergent physics' clothes on a modelling artifact".
6. **Two readings say something false past full.** `soc_true` reads 1.0000 while the cell holds
   1.6 times its negative's capacity, and `SOC_CLAMPED_HIGH` fires on every step past full (2880 of
   3600 at 1C) while `i_rejected_a` is 0 — on the circuit that flag means "charge was refused", on
   the porous models nothing was.
7. **The solve mostly keeps up.** The `Spm` and the many-particle cell converge on every step; the
   `Dfn` fails 46 (1C) and 56 (3C) steps of the threshold runs and 4–8 of the reaction runs, all past
   full. `SURFACE_OUT_OF_RANGE` fires from 0.4–0.7 A·h past full on, as H8 says.
8. **Reachable from a client.** None of the 19 shipped scenario files on a porous model declares a
   `[pack.bms]`, and `phase-12-slice-d-lesson.md` records that a constant current typed in the
   plating steps' free play overcharges. Not re-measured on the page here.

**No outside reference exists.** PyBaMM 26.6.2.0's interface submodels are SEI growth and lithium
plating (`pybamm/models/submodels/interface/`); it has no electrolyte oxidation or overcharge
reaction (its oxygen diffusion is lead-acid's). Its plating could reference the negative side only
until the positive empties, where its solve stops. So whatever this phase builds can be held to
conservation, convergence and boundedness — not to a curve.

## Where (b), the cold hold, stands

Nothing here touches it. Its stall is in the discharge **after** the hold, at a negative node near
the separator sitting on the clamp's kink while it gives lithium back; plating has nothing to do
there (the reversible store is empty within 300 s, `dfn-cold-hold.md`). Its overfill is ~1e-6 of
`c_max`, not the 0.6–0.8 of (a). A design for (a) changes how a full negative surface is treated —
the same clamp in `dfn.rs` — so whether it also removes (b)'s kink is a prediction to register and
test, not something to promise. The prototype that failed (`dfn-cold-hold-taper.patch`) failed on
"an exchange current of exactly zero makes 'no reaction' a root"; an outlet at the edge would
remove that root on the charge side only.

## The other Phase 12 leftover, the 0.65 V rest — not this

`plating-unconverged-runaway.md`'s rest that settles at 0.65 V follows a cold 2C charge from **empty
to 4.2 V**, which stops far short of full; its solves fail because the negative's electrolyte runs
to zero near its current collector during the charge (on a threshold cell too). That is a third
mechanism — electrolyte exhaustion on a fast cold charge, the charge-side relative of (c) — and
nothing proposed here touches it. Read from that note, not re-run. The same note's typed 2C and 3C
charges in the plating steps (before their 4.2 V stop) **are** (a): the reaction cell plated 43–375 %
of its capacity and the threshold cell reached 5.5–6.2 V, and at −10 °C 3C swung between −1.6 MV and
26 MV.

## Decisions for the owner

1. **What happens at the particle limits.**
   * **(i) The particles stop at their limits and the rest becomes heat** — the circuit's rule,
     moved from `soc = 1` to the particles' real edges: the charge a particle cannot take is
     refused (`i_rejected_a`), and its energy is heat at a placeholder side-reaction potential.
     Ends the impossible storage, makes runaway reachable for the right reason, keeps the
     physically possible zone past full. One unnamed reaction for both electrodes; one or two new
     chemistry keys, labelled placeholders. Cheapest.
   * **(ii) Named reactions** — plating where the cell runs it, and electrolyte oxidation at an
     empty positive as a second reaction with its own kinetics. More physical; every constant a
     placeholder with no reference; the positive's reaction is new code on three models.
   * Either way, on a `Dfn` running plating the negative's 1 % creep past `c_max` goes too.
2. **Which models.** All three porous models have the defect. Recommended order: `Spm` first (one
   particle per electrode, closed-form edges, as `porous-reversal.md` did), then the
   many-particle cell (its positive is the limit at full, so it is the cleanest test of the
   positive's outlet), then the `Dfn`.
3. **(b), the cold hold**: inside this phase as its own slice with its own bars, after (a) has
   changed the clamp it lives on — or left out, as the lesson already works around it.
4. **What the exit can promise**: conservation, convergence and boundedness, and a BMS-off
   overcharge that reaches runaway by refused energy; not agreement with a reference, since none
   exists.

## What this spike did not do

* It registered no predictions; any prototype must, before it runs.
* It did not measure parallel groups, voltage or power demands past full, long steps, or the page.
  The pack's range holds a voltage or power demand's current to the chemistry's bulk empty and full
  and, on the `Spm`, to the surface range (`spm-pack-window.md`, `dfn-long-step-holds.md`) — which
  does **not** keep a `Dfn` node's surface inside `c_max`: (b) is a voltage hold whose surface crept
  past it.
* The outer shell stands in for the surface, so a surface figure may lag the solve's own by up to
  one shell's gradient; the many-particle cell's positive figures are the emptiest particle's.
