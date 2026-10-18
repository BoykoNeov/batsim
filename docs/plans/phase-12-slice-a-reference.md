# Phase 12 slice A — the plating law's reference, and the size of its constant

**Status: DONE 2026-10-18** — `k = 4.8e-11 m/s` on the fine grid, the reference built and
running; the predictions were registered before any run (933ef4e) and are scored below. Plan: `phase-12-plating.md` §A. No engine code: the instruments are a PyBaMM script
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

## Results (run 2026-10-18)

**Status: DONE.** `k = 4.8e-11 m/s`, sized on the fine grid on the charge with its hold; the
PyBaMM reference ran at `rtol` 1e-10 on every charge at the first formulation (exact `max`/`min`,
`w = 1e-3`), so the stop rule was not reached and the reference is the closed-loop model.

### The engine, open loop, per unit `k`

`cargo test --release -p sim-data --test plating_open_loop -- --ignored --nocapture`. Gross plated
lithium at the end of each leg, A·h per (m/s); "share" is the plating current over the cell current,
per unit `k`, its largest value on each leg. No step of any run was unconverged.

| charge | sheet | CC gross | CC–CV gross | hold ÷ CC | CC–CV net | lowest `η`, mV | share, CC / hold |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 25 °C 0.3C | allows | 0 | 0 | — | 0 | +37.0 | 0 / 0 |
| 45 °C 0.7C | allows | 0 | 0 | — | 0 | +29.7 | 0 / 0 |
| 0 °C 0.3C | allows | 1.658e6 | **2.113e6** | 1.27 | 1.057e6 | −8.65 | 8.7e6 / 8.5e6 |
| 25 °C 0.7C | allows | 4.74e3 | 6.08e3 | 1.28 | 3.04e3 | −1.61 | 1.4e5 / 1.7e5 |
| −10 °C 0.3C | forbids | 3.008e7 | **5.215e7** | 1.73 | 2.608e7 | −32.3 | 1.2e8 / 1.2e8 |
| 0 °C 1C | forbids | 5.565e7 | 1.265e8 | 2.27 | 6.411e7 | −88.1 | 8.2e7 / 1.6e8 |
| 0 °C 2C | forbids | 5.518e7 | 4.055e8 | 7.35 | 3.094e8 | −206.9 | 2.0e8 / 4.4e8 |
| −10 °C 1C | forbids | 1.003e8 | 3.606e8 | 3.59 | 2.659e8 | −126.7 | 1.6e8 / 4.2e8 |
| −10 °C 2C | forbids | 3.004e7 | 1.107e9 | 36.8 | 1.001e9 | −251.9 | 3.3e8 / 6.0e8 |

The recommended grid, recorded and not sized on: 0 °C 0.3C 2.035e6 (3.7 % under the fine grid),
−10 °C 2C 1.004e9 with the hold (9 % under) and 1.20e7 without (2.5× under).

### The size

The target is the spike's, 0.002 % of the 5.153 A·h capacity on the coldest charge the sheet allows,
read on that charge with its hold: `k = 1.031e-4 / 2.113e6 = 4.88e-11` m/s; on the constant-current
leg alone it would be 6.22e-11, **1.27× apart — under the 1.5× that would have gone to the owner**, so
recorded and not asked. Rounded **down to 4.8e-11** so the allowed charge sits under the target
(0.00197 %), 21× under OKane2022's 1e-9. At that size, per charge, gross plated / lost (`d` = 0.5):

| charge | engine, gross | lost | PyBaMM closed loop, gross | PyBaMM ÷ engine | PyBaMM at `k`/100, ×100 |
| --- | --- | --- | --- | --- | --- |
| 25 °C 0.3C, 45 °C 0.7C | 0 | 0 | 0 | — | — |
| 0 °C 0.3C | 0.00197 % | 0.00098 % | 0.00227 % | **1.152** | 1.156 |
| 25 °C 0.7C | 5.7e-6 % | 2.8e-6 % | 8.5e-6 % | 1.50 | — |
| −10 °C 0.3C | 0.049 % | 0.024 % | 0.049 % | 1.018 | — |
| 0 °C 1C | 0.118 % | 0.059 % | 0.118 % | 1.001 | 1.012 |
| 0 °C 2C | 0.378 % | 0.189 % | 0.368 % | 0.973 | — |
| −10 °C 1C | 0.336 % | 0.168 % | 0.331 % | 0.987 | 1.011 |
| −10 °C 2C | 1.031 % | 0.515 % | 0.943 % | **0.915** | 1.0004 |

The last column is the reference run at a hundredth of the constant, where plating cannot feed back:
it separates the two causes of a gap. On the forbidden charges the open loop agrees with the closed
one at small `k` to 1 %, and the whole gap at size is plating's feedback — the closed loop plates
8.5 % less at −10 °C 2C, where plating reaches 2.6 % of the current in the hold. On the allowed cold
charge the gap is the same at either size, so it is **not feedback but the two models' potentials**:
PyBaMM's lowest is −9.01 mV against the engine's −8.65, within the ~1 mV A1 established; on a charge
that barely crosses the line, a third of a millivolt is 15 % of the plating. It is not the particle
grid: PyBaMM at the engine's 40 shells gives the same figure (1.1683e-4 against 1.1684e-4 A·h).

Cap width: `w` = 1e-2 and 1e-4 move 0 °C 1C's gross in the seventh digit and its net by 4e-5 of
itself; the warm charges plate exactly nothing in the reference at any width.

### Against the predictions

1. **Held.** Every constant-current figure equals A1b's to three significant figures.
2. **Held on the range, missed on the consequence.** The hold costs 0 °C 0.3C 1.27×, inside 1.2–3, so
   the size moved by 1.27× — and the owner's question, predicted more likely than not, was not needed.
3. **Held.** Exactly nothing on either warm charge, in both instruments, hold included.
4. **Held, and on the allowed cold charge too.** Net is below gross on every charge that plates; on
   0 °C 0.3C the hold strips **all** the reversible lithium back (net = the dead share alone), and on
   −10 °C 0.3C nearly all.
5. **Held.** With the hold the mildest forbidden (−10 °C 0.3C) plates 24.7× the worst allowed in the
   engine and 21.8× in the reference (constant current alone: 18×). Exit 1's 10× bar has room.
6. **Held as worded.** Allowed charges peak at 0.04 % of the current; the forbidden ones reach 2.0 to
   2.6 % in the hold's tail, past the 1 % at which the open loop was called suspect — and there the
   closed loop is the arbiter, within 2.7 % of the open loop except at −10 °C 2C (8.5 % under).
7. **Held on the letter, missed in substance.** Every charge plating more than the target is within
   10 % (worst −10 °C 2C, −8.5 %). The allowed cold charge, which plates just under the target and so
   is outside the bar's wording, is 15 % apart — and it is the charge that sets the constant. Slice B's
   exit 2 cannot hold the engine's reaction to the reference there to better than the two models'
   potentials allow; its bar is set on the forbidden charges, or against the engine's own open loop at
   small `k`, and says which.
8. **Held without the hold, failed with it.** At −10 °C 2C the recommended grid plates 2.5× less on the
   constant-current leg, as predicted, but only 9 % less on the full charge: the hold, at near-full
   charge and falling current, is where most of the plating is, and there the coarse grid's 38 mV
   deficit has gone.

### What this says about the law

On the charge a charger runs, the hold is where cold plating happens: at −10 °C 2C, 97 % of it comes
after 4.2 V (the constant-current leg ends 7.6 % charged), and every forbidden charge at 1C or more
plates more in the hold than before it. Which charge plates most is now the cell's, not the protocol's: −10 °C 2C >
0 °C 2C > −10 °C 1C > 0 °C 1C > −10 °C 0.3C, colder and faster both costlier, against A1b's
constant-current order in which 2C at −10 °C plated less than 1C.

### The `[dfn.plating]` draft (slice B writes it into the file; names may change)

```toml
[dfn.plating]
# placeholder — no source (phase-12-plating.md, owner decision 2). Sized on the fine grid
# (dfn::FINE_*), open loop, so the coldest charge LG's sheet allows (0 degC 0.3C, then 4.2 V
# held to C/20) plates 0.00197 % of capacity; 21x under OKane2022's 1e-9. On the recommended
# grid the same charge plates 3.7 % less. docs/plans/phase-12-slice-a-reference.md.
rate_constant_m_per_s = 4.8e-11
# OKane2022 (PyBaMM parameter set; it "does not claim to be representative")
transfer_coefficient = 0.65
# placeholder — no source (owner decision 3): a round share of plated lithium lost at once
dead_share = 0.5
```

### Instruments, and one unexplained failure

`crates/sim-data/tests/plating_open_loop.rs` (two `#[ignore]` reports, 31 s release) and
`tools/reference/plating_reference.py` (about 5 min a charge at 40/20/40, 200 shells) are in the tree;
the spike's `ref\` scripts are superseded by the second and its A1b integrator by the first. **Four
PyBaMM charges failed when run three at a time** — the hold leg came back empty — and all four ran
cleanly one at a time with the same arguments. Not traced; run the reference serially.

Per-second traces (`py_*.csv`) are not committed: no test reads them, and the slice B comparison will
fix its own sampling when it writes its golden.
