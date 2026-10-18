# Phase 12 slice A — the plating law's reference, and the size of its constant

**Status: PRE-REGISTERED 2026-10-18** — written before any run; the results section below is
written after. Plan: `phase-12-plating.md` §A. No engine code: the instruments are a PyBaMM script
in `tools/reference/` and an `#[ignore]` report test in `crates/sim-data/tests/`, and the
`[dfn.plating]` section this slice drafts goes **in this note only** — the chemistry file and the
placeholder list in `lgm50_provenance.rs` are slice B's to change, with the reaction that reads them.

## The law, as both instruments compute it

Per negative node, `η = φ_s − φ_e` (no film in this slice), `f = F/RT`, `α = 0.65`, `i0 = F·k·c_e`:

    J = i0·[exp(−α·f·η) − exp((1 − α)·f·η)]      positive plates, negative strips

and stripping is **limited by the node's reversible inventory**, so a node holding no plated lithium
above 0 V does nothing at all. A fixed share `d` of what plates is dead at once and never strips.

Two things the plan left implicit, fixed here before any number is read:

1. **Gross and net.** On a hold at 4.2 V the current tapers, the graphite's potential rises back over
   the line, and reversible lithium strips. The dead share is a share of what *plated*, so the loss
   follows the **gross** figure (the time integral of the plating branch alone); the lithium left as
   metal at the end is the **net**. Both instruments report both. The sizing target is on gross.
2. **Which protocol the target is read on.** The spike sized `k` on constant-current charges that stop
   at 4.2 V; a charger holds 4.2 V afterwards, and exit 1 reads "every charge the sheet allows" on the
   charge a user would run. **The size is read on the constant-current + constant-voltage charge**
   (4.2 V held to a C/20 taper); the constant-current figure is recorded beside it. If the two sizes
   differ by more than 1.5×, the owner is asked before the constant is written.

## The instruments

* **Open loop, in the engine** (`crates/sim-data/tests/plating_open_loop.rs`, `#[ignore]`, a report):
  the shipped LG M50 `Dfn`, 1S1P, isothermal, from empty, 1 s steps, every negative node's `φ_s − φ_e`
  and `c_e` read off the committed state and the law integrated beside it per unit `k`. It cannot
  feed back — the potential is the plating-free model's. Gross is linear in `k`; net is too (the cap
  scales with it). Run on the fine grid (sized) and the recommended grid (recorded only).
* **Closed loop, in PyBaMM** (`tools/reference/plating_reference.py`): PyBaMM's DFN with its
  `"reversible"` plating submodel replaced by one that computes the law above. The parameter swap
  alone (stripping exchange current set to `F·k·c_e`) gives the uncapped law and no dead share; the
  cap has to act on the **net** current where it strips, or the plating branch still runs at zero
  inventory above 0 V — PyBaMM's own flaw again, smaller. So the submodel's coupled variables are
  overridden: `J_c = max(J, 0) + min(J, 0)·c⁺/(c⁺ + w)` in plating-positive sign, `c⁺ = max(c_rev, 0)`,
  and the rate equations split the plating branch `(1 − d)` reversible, `d` dead. **Cap width
  `w = 1e-3 mol/m³`**: about 0.2 % of the electrode-mean plated concentration the target implies on
  the coldest allowed charge (0.002 % of 5.153 A·h in about 8.5e-6 m³ of negative electrode,
  ≈ 0.44 mol/m³). Grid 40/20/40 in x and 200 shells, IDAKLU, `rtol = atol = 1e-10`, the same
  parameter values the spike matched the engine to within ~1 mV.

Conditions, as A1b: allowed 25 °C 0.3C, 45 °C 0.7C, 0 °C 0.3C, 25 °C 0.7C; forbidden −10 °C 0.3C,
0 °C 1C and 2C, −10 °C 1C and 2C. Each as CC to 4.2 V, and as CC then a 4.2 V hold to C/20.

**Stop rule** (the plan's): if the modified model does not finish the 0 °C 0.3C and −10 °C 1C
CC–CV charges at `rtol` 1e-10 after three formulations — the exact `max`/`min`, PyBaMM's smoothed
ones, and a tenfold wider `w` — the reference is the open-loop integral alone, and exit 2 of the plan
says so.

## Predictions

1. **The open loop reproduces A1b.** Constant-current gross figures on the fine grid equal the spike's
   table to three significant figures (the 1S1P `Dfn` path has not changed since; the split damping
   acts on parallel groups only).
2. **The hold costs the allowed cold charge.** At 0 °C 0.3C, gross with the hold is 1.2 to 3 times the
   constant-current figure, so the size moves by that factor — the 1.5× question to the owner is more
   likely than not.
3. **The warm allowed charges stay free.** 25 °C 0.3C and 45 °C 0.7C plate exactly nothing with the
   hold too, in both instruments (in PyBaMM: below `w`'s footprint, 1e-9 of capacity).
4. **The hold strips.** On every forbidden charge, net at the end of the hold is below gross: the
   graphite crosses back over the line as the current tapers.
5. **The sheet still separates.** With the hold, the mildest forbidden charge plates at least 10× the
   worst allowed one (exit 1's bar), gross; between 10× and 40×.
6. **Open loop holds at the size**: at the sized `k`, plating stays under 1 % of the cell current on
   every allowed charge throughout; on the forbidden ones it may exceed it in the hold's tail, where
   the current is small — the share at the end of each hold is reported.
7. **The reference agrees with the open loop.** At the sized `k`, PyBaMM's closed-loop gross is within
   10 % of the engine's open-loop gross on every charge plating more than the sizing target; the gap
   is the two models' potentials (~1 mV, 3 % in rate at 0 °C) plus the feedback the open loop drops.
8. **The recommended grid plates less in the cold** at the same `k`, by more than 2× at −10 °C 2C
   (it read the line 38 mV high there, A1).

The dead share `d` is a round placeholder, **0.5** (the prototype's), labelled so; no prediction
rests on it — it splits gross into lost and reversible, and moves net only through the stripping
inventory.

## Results

*(not yet run)*
