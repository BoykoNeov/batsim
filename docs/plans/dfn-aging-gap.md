# The DFN's aging gap: a rule implemented for three commits and verified by none

`CLAUDE.md` forbids one thing outright in the physics section — "never model capacity
fade without the matching resistance growth" — and Phase 7 shipped a cell model for which
that rule was *implemented* and *unverified*. `docs/plans/phase-7-dfn.md` names it three
times: slice B raises it, slice C records it untouched, slice D says it did not close it
and does not claim to. This is the slice that closes it.

Nothing here changes engine behaviour. The out-of-tree trajectory instrument is
**byte-identical** against `after-bleed-band.txt` both before and after the one `sim-core`
edit, which is the correct and complete result for a slice of this kind rather than a
weaker one.

## What was actually open, which is narrower than the sentence that named it

`phase-7-dfn.md:1199` reads: "the `eff_r0_factor` path is wired the same two ways for the
DFN as for the SPM **and still exercised by nothing**." The second half is false for the
SPM, and was false when it was written: `crates/sim-data/tests/spm_pack.rs:151` is
`aging_grows_the_dc_resistance_of_the_shipped_spm_cell`, written in Phase 6 slice B for
exactly this. Checked at the *configuration* level rather than by grepping the internal
parameter name — a test that exercises this does not mention `eff_r0_factor`; it sets
`aging: Some(..)` or calls `set_cell_factors`, and picks a `cell_model`. Every DFN test in
the repo passed `aging: None`, and no scenario pairs a `[pack.aging]` block with `Dfn`.

So the gap was DFN-only, and the slice is half the size the sentence implies. This repo
has now been mis-scoped by a stale plan-doc claim three times (the fault queue and
`clearBmsFault` in `protection-escalation.md`, and this one), and the pattern is the same
each time: **a claim that was true when written, about a thing a later commit changed.**

The second half of the gap is smaller still and is also closed here: `probe::jacobian_pair`
hardcoded both health multipliers to a cell in new condition, so the analytic Jacobian —
the one piece of `dfn.rs` that can be *silently* wrong — was only ever differentiated at
`1.0`.

## The protocol, which is not the SPM sibling's, and the copy that would have passed

`spm_pack.rs` measures DC resistance as `ΔV/ΔI` between two **zero-length** probe steps:
one at 0 A, one at 5 A, neither of which moves the cell. Copying that here would have been
wrong in the worst available way — it would have gone **green**.

[`dfn::probe_at`] answers a non-positive `dt` with the **stored line** rather than a solve
(its own doc comment says so, and `a_zero_length_probe_step_does_not_reach_the_solver`
pins it). Two zero-length probes therefore return two points on one straight line, and
`ΔV/ΔI` is that line's slope exactly. On a freshly built cell the line is
`seed_resistance`, which *does* carry `eff_r0_factor` — so the test would have passed
while exercising none of the solver, and would have passed identically had the real solve
path lost the factor entirely.

The two readings are not close, which is the useful part:

| protocol | healthy | at `soh_resistance` 1.5 | ratio |
| --- | --- | --- | --- |
| two zero-length probes (the stored line) | 0.055836 Ω | 0.081048 Ω | **1.4515** |
| one real step each at 0 A and 1 A, `dt` = 1 s | 0.034591 Ω | 0.045781 Ω | **1.3235** |

Both are honest readings of different quantities. The first is the *linearized* resistance
at zero current, which is roughly **twice** the chord a real current draws. The shipped
test uses the second: two packs built from the same config, one step each, at two
currents.

**Transferable:** a probe protocol inherited from a sibling model carries that model's
`dt` semantics with it. For an SPM a zero-length probe is a table lookup and reports the
curve; for a DFN it is a documented short-circuit to a cached line. The same three lines
of test code mean different things in the two files.

## The threshold, derived rather than fitted

A DFN's DC resistance is a sum and only the charge-transfer term scales with the factor:
electrolyte ohmic, solid ohmic and the separator do not. The linearized reading above
decomposes exactly — `1.4515 = 1 + 0.5 × f` gives `f = 0.903`, and back-substituting,
`r_ct = 0.05042 Ω` against `r_ohmic = 0.005417 Ω`, which reproduces the aged 0.081048 Ω to
six figures. So **charge transfer is 90 % of the linearized total.**

On a real chord it is diluted further, and by a second mechanism: Butler–Volmer's own
nonlinearity. Scaling `i₀` costs less once the kinetics turn logarithmic, so the observed
growth **falls with the current it is measured at**:

| probe current | ≈ C-rate | ratio at factor 1.5 | implied `f` |
| --- | --- | --- | --- |
| 1 A | C/5 | 1.3235 | 0.647 |
| 5 A | 1C | 1.1812 | 0.362 |
| 15 A | 3C | 1.1070 | 0.214 |

Two consequences. First, **the SPM sibling's `> healthy × 1.2` bound would fail on correct
code** anywhere above about C/3 — a fixture-on-its-threshold failure of the family this
repo has hit twice before, avoided here only because the number was measured before the
assertion was written. Second, the assertion is placed at C/5, where the reading is
closest to the linear claim the implementation actually makes, and bounded at **1.15**
against a measured 1.3235. The margin is not spent buying confidence in the effect —
removing the divide makes the ratio *exactly* 1.0, so any bound clear of 1.0
discriminates — it is spent on not pinning the shape of the Tafel curve.

The measurement is stable in `dt`: 1.1831 at `dt` = 0.1 s, 1.1812 at 1 s, 1.1673 at 10 s,
1.1346 at 60 s (all at 5 A).

## The premise, asserted rather than trusted

The whole claim is that the factor reaches the exchange-current density, and it only says
that while `contact_resistance_ohm` is zero. `nmc_21700_lgm50.toml` sets it to `0` —
Chen2020's own value, not an omission — so `dfn.rs`'s `Sides::new` divides `m_ref` on both
electrodes instead, which multiplies the linearized charge-transfer resistance by exactly
the factor.

A later chemistry with a nonzero contact resistance would make this test pass whatever the
implementation did. So the test **asserts the zero**, with a message saying what to do
about it. Its SPM sibling shares the premise, documents it in prose, and does not pin it;
that is noted rather than fixed, because reaching into another slice's test to add an
assertion is how a scoped change stops being one.

## The perturbation table

Run in a `git worktree` at `HEAD` with the new tests copied in, never by reverting the
working tree. **All runs used `--no-fail-fast`** — see the trap below. A and C were re-run
against the **final** tree after the Jacobian half landed, because a perturbation measured
against an earlier version of the tests it is quantifying proves less than the table
claims.

| # | perturbation | expected | caught by |
| --- | --- | --- | --- |
| A | `Sides::new` drops `/ eff_r0_factor` on `m_ref` (growth never reaches the cell) | red | **only** `aging_grows_the_dc_resistance_of_the_shipped_dfn_cell`, at ratio exactly `1` |
| B | `Sides::new` drops `* eff_r0_factor` on `r_contact` | **green** | nothing — failure set identical to baseline |
| C | every DFN arm in `ecm.rs` passes `chem.cell.capacity_ah` instead of `eff_capacity_ah` | red | **only** `capacity_multipliers_scale_the_amp_hours_of_the_shipped_dfn_cell` |
| D | `aging.rs` decouples resistance growth from the capacity loss (constant 0.02) | red | `resistance_grows_in_step_with_capacity_loss` (both crates) **and** `a_configured_dfn_pack_actually_fades` |

**A is the one the slice exists for**, and it confirms the gap was real rather than
notional: with resistance growth disconnected, every other DFN test in the workspace
passes — the 21 others in `dfn_cell.rs` (conservation bounds, the Jacobian check, the
demand-solve set) and the whole of `dfn_golden.rs` and `dfn_chemistry.rs`.

**B is the load-bearing control, and it is a green one.** `dfn.rs:552-557` claims in prose
that multiplying `contact_resistance_ohm` alone "would fade capacity with exactly zero
resistance growth". Nothing tested that claim. Removing the multiply changes no test in
the workspace, which is what "inert on this chemistry" means measured rather than argued.
The line stays: it is correct for a chemistry that reports a contact resistance, and it is
the reason the divide needed a comment in the first place.

**C had to be aimed at the DFN specifically.** The obvious target — `Sides::new`'s
`kappa: geometric / eff_capacity_ah` — cannot be perturbed without changing its units, and
the shared call site in `pack.rs` would have been caught by the SPM and ECM siblings too,
proving nothing about this wiring. Perturbing the four `dfn::` arms in `ecm.rs` isolates it.

**D shows the third test is not vacuous, and that it says more than its sibling.**
`a_configured_spm_pack_actually_fades` does *not* fail under D; the DFN version does,
because it asserts the extra clause — the pack that faded further also grew more
resistance — which is `CLAUDE.md`'s rule stated as a comparison rather than as two
independent inequalities.

## Two traps, both of which would have produced a wrong table

**`cargo test --workspace` stops at the first failing target.** Not the first failing
*test* — the first failing binary — so every target ordered after it never runs and its
failures are invisible. Perturbation D's first run reported one failure
(`resistance_grows_in_step_with_capacity_loss`) and looked like a clean single-catch
result; with `--no-fail-fast` it reports **four**, including the one that matters here.
Any perturbation table built from fail-fast runs understates coverage, and understates it
*silently*. Every row above was re-run.

**A fresh `git worktree` on Windows fails a test the main repo passes.** `git worktree add`
checks out with CRLF, and `sim-server`'s `a_scenario_can_inline_its_chemistry_and_survive_
the_round_trip` compares an inlined chemistry TOML byte-for-byte, so it fails on `\r`
alone in *any* worktree, perturbed or not. This is a property of the harness, not of the
change: the same test passes in the main repo, and it appeared in the worktree baseline
before any perturbation was applied. Establish the baseline failure set in the worktree
first; a perturbation run compared against the *main repo's* green is one phantom row per
table.

## The Jacobian half

`probe::jacobian_pair` built its `Sides` with `1.0` and the nominal capacity, so the
entries carrying the two multipliers — Butler–Volmer's through `m_ref`, the particle flux
boundary's through `kappa` — were differentiated only for a cell in new condition. It now
takes a `probe::Health { eff_r0_factor, eff_capacity_ah }`, bundled as a struct because
the loose form is eight arguments and clippy's `too_many_arguments` fires at that.
`the_analytic_jacobian_matches_a_difference_quotient` gains two cases at factor 1.5 and
0.8 capacity, which run the cell aged as well as probing it aged so the state and the
Jacobian describe the same cell.

**What this can and cannot say, stated because the distinction is the whole value.** It
says the analytic derivative still matches its own residual in the aged regime — worst
row-relative disagreement 1.122e-9 and 8.568e-10, against 1.166e-9 and 8.845e-10 for the
fresh siblings, so the new cases sit four orders below the 1e-5 tolerance and are not
near-threshold. It cannot say the multipliers are applied *correctly*: both sides of the
comparison are assembled from the same `Sides`, so a factor in the wrong place moves them
together and cancels. That claim belongs to perturbation A, and only to it.

That last sentence is **measured, not reasoned**. Perturbations A and C were re-run with
the aged Jacobian cases present, and both stay green under both — resistance growth
disconnected entirely, and the capacity factor withheld from every DFN arm, and the
Jacobian check notices neither. An argument of that shape was wrong once already in this
repo (slice A's "provably inert for the SPM"), which is why it was run rather than
asserted.

## Versions: none moved, checked individually for the fifth time

* `SNAPSHOT_VERSION` **13**, unmoved. No state was added: the health multipliers are
  existing fields, and `probe::Health` is a call-site bundle that is never serialized.
* `API_VERSION` **2** — no route, field or shape changed.
* `WASM_API_VERSION` **4** — the page reads nothing new.
* `sim-godot` — nothing, per the standing rule against adding accessors with no caller.

The one `sim-core` edit is inside `#[doc(hidden)] pub mod probe`, which nothing in `step`
calls. That is an argument; the instrument is the measurement, and it agrees.

## Still open, unchanged by this slice

* The over-discharge half of the energy hole (`energy-hole.md`), still deliberately open:
  the term that closes it is a cooling one.
* The low-clamp solve-side fix (`low-clamp-solve-side.md`), measured not to work.
* `Pack::step` at 100S10P, still over the ECM-only budget.
* No DFN scenario file, so no client can reach the model — the Phase 6 SPM slice's
  counterpart has never been written for Phase 7.
