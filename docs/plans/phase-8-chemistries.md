# Phase 8 — new chemistries

**Status: planned, not started. Nothing below has been measured** except where a line says
so and names what measured it. The repo is at `SNAPSHOT_VERSION` 17 and `WASM_API_VERSION`
6. Everything here is pre-work text in the shape `phase-6-porous-electrodes.md` and
`phase-7-dfn.md` use; where a slice note later contradicts it, **the slice note is the
measurement and wins**.

## Framing: this phase opens a bullet, and tests a principle

`CLAUDE.md` has no Phase 8. This phase comes from two sentences elsewhere in it. The first
is in the chemistry section:

> Ship LFP and NMC first; lead-acid (Peukert) and NiMH (−ΔV, hysteresis) later.

Lead-acid landed across three documents — `lead-acid-data-only.md` (parameters, zero code),
`diffusion-overpotential.md` (the mechanism, v16 → v17), `lead-acid-client.md` (scenario and
guided-path steps 22–24). **NiMH is the unclaimed half of that sentence**, and it is the
harder half, because it needs state the engine does not have.

The second is design principle 10:

> **Chemistry is data, not code.** A chemistry is a TOML parameter set. Adding a chemistry
> must never require a code change.

That principle has never been tested. Every chemistry in the repo was shipped alongside
code that was being written anyway: LFP and NMC with Phase 0, LG M50 with Phases 6 and 7,
lead-acid with a snapshot bump for the mechanism it exposed. **A chemistry added with zero
lines of Rust changed does not yet exist**, so the principle is currently an aspiration
with four non-examples behind it. Slice A exists to settle it either way.

## What is shipped today

| file | cell | what it parameterises |
| ---- | ---- | --------------------- |
| `lfp_26650_generic.toml` | LFP 26650 | ECM, aging, safety |
| `nmc_18650_generic.toml` | NMC 18650 | ECM, aging, safety |
| `nmc_21700_lgm50.toml` | NMC 21700 (Chen2020) | ECM + `[spm]` + `[dfn]` |
| `pba_agm_2v_generic.toml` | lead-acid AGM 2 V | ECM + `[diffusion]` |

Four files, three chemistries, one of them non-lithium. **The validator carries no lithium
assumption** — measured, not assumed: `pba_agm_2v_generic.toml` is a 2 V cell that loads,
validates and ships, so the OCV monotonicity rule and the `v_min`/`v_max` ordering rule are
already known to accommodate a voltage range far below lithium's. A 1.2 V or 1.5 V cell
therefore hits no known wall in `chem.rs::validate`. That is the single cheapest fact
available about this phase and it is why slice A is scoped as small as it is.

## The owner's scoping decisions, recorded

Both were asked and answered on 2026-08-27, and both change the size of the phase, so they
are written here rather than left in a transcript.

1. **A chemistry is "done" when a guided-path lesson teaches it.** Not when it loads, and
   not when a scenario can select it. This is the lead-acid treatment — parameters, then
   client wiring, then lesson steps — and it is roughly three times the work per chemistry
   of the "loads and validates" reading. It is chosen deliberately: the engine is the
   product, but a chemistry nobody can see teaches nobody anything.
2. **English-spelled quantities in lesson prose are banned, not read.** See slice 0.

Decision 1 is what bounds the phase at **two** chemistries rather than a list. Adding a
third after the phase closes is a one-slice job against a written recipe (below), not a
reopening.

## Exit criteria (authored here)

| exit criterion | carried by |
| -------------- | ---------- |
| **1. Principle 10 is settled by measurement — CLOSED, zero-code branch (slice A).** One chemistry is added with **zero lines of Rust changed**, or the code it needed is named, its reason recorded, and `CLAUDE.md`'s principle amended to say what a chemistry can and cannot be. Either outcome closes this; only leaving it untested does not. | slice A |
| **2. Both new chemistries are taught.** Each has a parameter file with provenance on every constant, a scenario file, guided-path steps, and claims in `path-claims.toml` under the digits rule. | slices B, D |
| **3. The floor did not move.** Every existing trajectory — ECM, SPM, DFN, lead-acid — is bit-identical before and after the phase, unless a slice argues a measured exception the way slice A of Phase 7 did. A new file that no existing pack loads cannot move one; the hysteresis state in slice C is the place this criterion is actually at risk. | slice C, re-checked by every slice |
| **4. The nickel cell's end-of-charge signature is emergent.** The voltage of a full NiMH cell on constant current **falls**, and that fall must come out of the physics — the charge the cell stops accepting, the heat that makes, and a negative `docv_dt_v_per_k` — not from a scripted override. `CLAUDE.md` forbids the scripted kind, and this is the phase's one genuinely new emergent behaviour. **Whether the existing overcharge-heat path (`energy-hole.md`) is enough to produce it is the spike question and is not known today.** | slice C spike, then slice D |

## Slices

| slice | scope | version |
| ----- | ----- | ------- |
| **0** | **LANDED 2026-08-27** — `docs/plans/path-digits-rule.md`. The digits rule. The count was 70, not 68, and **half of them were tied to nothing**, which is what made this two slices rather than one: the rewrite makes the digit ledger see a quantity, and the ledger has no waiver. Thirty-five rewritten, the ban built path-wide and wider than the reader, the other 48 (as the ban counts them) named phrase by phrase in `[[english]]`. The reader stays for now. No engine change. | v17 (no bump) |
| **A** | **LANDED 2026-08-27** — `docs/plans/phase-8-slice-a-lto.md`. `chemistries/lto_20ah_generic.toml`, datasheet-anchored on a 20 Ah LTO cell. **Zero engine code changed, so exit criterion 1 closes on the zero-code branch** — with one caveat found by reading rather than running: `[safety]` has **no way to say a cell does not plate**, because zeroing the cost fields still raises the flag and dropping the section switches off runaway too, so the file ships a labelled sentinel. Two *test-side* guards had to be amended, and both had assumed every cell is a graphite cell. | v17 (no bump) |
| **B** | **LANDED 2026-08-27** — `docs/plans/phase-8-slice-b-lto-client.md`. Three scenario files (a 10 C discharge, and a cold fast charge with its graphite control), guided-path steps 25 and 26, seventeen claims, four arms, eighteen ledger rules. Both steps are LEDGERED, so `unledgered` is still empty. No engine code; the only Rust touched is the claims harness. Found by the checks rather than by reading: the digits rule refused the first sentence written under it, a flag instant that was also the timestep could be accounted two ways and therefore neither, and nineteen self-stated counts were stale. **Exit criterion 2 is half closed** — the LTO cell is taught. | v17 (no bump) |
| **C** | **The hysteresis state, and NiMH's parameters.** Per-cell memory of drive direction. **Carries the phase's one snapshot bump.** Scoped with lead-acid voltage memory, not separately — see below. | **v17 → v18** |
| **D** | **Teach NiMH,** including the falling end-of-charge voltage. Carries exit criterion 4. | v18 (no bump) |

Each slice keeps `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` clean, run below normal priority.

**One `SNAPSHOT_VERSION` bump for the phase, on slice C** — the budget Phases 6 and 7 both
set and held.

## Slice 0 is a prerequisite, and why

`docs/plans/path-word-numerals.md` and `path-word-batch-two.md` built a reader for
quantities the lesson prose spells in English — "half a point", "eight and a half seconds",
"about ten times". It covers **7 of the path's 24 steps**. Finishing it is seventeen more
rounds of work at roughly one round per step, which is the shape of the backlog that
prompted the question that started this phase.

The owner's decision is to **ban the practice instead of reading it**: one pass rewriting
the quantities into digits, one check forbidding new ones, and the digit scanner — which
already covers all 24 steps — becomes the whole of the coverage.

**It must land before slice B**, and this is the only hard ordering constraint in the phase.
Slices B and D write new lesson prose. Prose written before the rule is prose written in the
old style, and the phase would then close having re-created the gap slice 0 exists to shut.

**"Retire the reader" is a migration, not a subtraction, and the order inside the slice is
load-bearing.** Some of the 283 claims already in `path-claims.toml` read a spelled
quantity — the ledger's own comment on `belief-drifts` says so in as many words ("two claims
on its estimator gap, which the prose spells in words") — and the reading machinery those
claims depend on is `spelled_numbers`, `word_blind`, `Tie::Hours`, `Tie::Seconds`, the
silent-skip guard and thirteen vocabulary rules. Deleting any of it while a claim still
points at a word turns that claim red for a reason that has nothing to do with the physics.
So, inside slice 0: **add the ban and rewrite the prose first; re-point every claim that
read a word at the digit form; only then take the reader out** — and if any claim cannot be
re-pointed, the reader stays and the slice ships the ban alone. A phase that ships a red
suite has failed whatever else it did.

Two costs, both stated rather than discovered later:

* **The prose reads more clinically.** "0.5 points" for "half a point". Accepted by the
  owner as the price.
* **A ban cannot be policed perfectly either.** Deciding whether an English number word is
  acting as a quantity — "one of the cells" is fine, "one volt" is not — is the same hard
  problem the reader had. What changes is which way the mistakes fall. A false alarm under a
  ban costs a rewritten sentence and is visible immediately; a miss under a reader is silent
  and a wrong number ships. **That asymmetry is the whole argument**, and it is worth more
  than the seventeen rounds it saves.

Two counts, with different standing, because one of them is weaker than the other and the
difference matters:

* **68 spelled quantities path-wide** — measured, by the survey in `path-word-batch-two.md`
  that ran the accounting check over every step with the per-step gate disabled. This is a
  real measurement and the slice can budget against it.
* **Roughly 30 article-form phrases** — "an hour of simulation", "a third of the sag", "half
  a percent", "a tenth of a millivolt" — **hand-counted** from a context grep over
  `web/app.js`, excluding the code comments the grep also catches and the ordinal uses that
  are not quantities at all ("a second opinion", "a second 3 C discharge"). **Treat it as a
  floor, not a count**: the grep keys on a closed list of unit nouns, so a phrase like "a
  fraction of the run" is only in the list because that noun happened to be in it. The
  honest statement is "at least thirty, and the instrument cannot say how many more."

That second number is the one that moved while this document was being reviewed. It was
first written as "about a dozen", which was a miscount — the grep's raw output includes
`a second pulse` and `a second number beside`, which are ordinals — and correcting it made
the article form **bigger**, not smaller. It does not weaken the ban; it strengthens it. A
reader extension would have to cover thirty-plus phrases of a shape the silent-skip guard
**cannot watch**, because that guard walks numerals and this shape has none. The rewrite
covers them by construction, and that is the second argument for the ban.

## Slice A: which chemistry, and what "cheap tier" means

Cheap tier means the cell fits the existing model shape exactly: an OCV table, an `[r0]`
grid, one or two RC pairs, thermal, `[reversal]`, and optionally `[aging]` and `[safety]`.
No new state, no new term, no new code — that is the hypothesis under test.

Candidates, with what makes each pedagogically worth a lesson. **No constants are stated
here on purpose**: `CLAUDE.md` forbids an unlabelled physical number and this document has
sourced none.

**The pick is ruled here rather than left to the slice**, because the candidates test
exit criterion 1 with very different strength and a free choice lets a future session pick
the weakest one and call the criterion closed. **The rule: take the candidate that strains
the existing model shape hardest, subject to a citable public parameter set.** Strain means
a voltage window unlike the shipped lithium files, a `[reversal]` answer the shipped files
do not already imply, and a `[safety]` story that differs. A zero-code result is only
evidence about principle 10 in proportion to how far the file was from what already works.

**On that rule the pick is LTO**, and the other two are named as fallbacks for when the
parameter set cannot be sourced — the one condition the rule is subject to.

* **Lithium titanate (LTO).** The most distinct of the tier. It charges at rates the other
  lithium cells cannot survive, it does not plate at low temperature the way graphite does,
  and it trades a large amount of energy for that. Against the existing plating lesson it is
  a direct contrast: the same cold fast charge, a different outcome, from parameters alone.
* **Sodium-ion.** Topical, and its distinguishing behaviour — what happens when it is taken
  to zero volts — lands on machinery the engine already has in `[reversal]`.
* **NCA.** Closest to what is already shipped, and therefore the weakest lesson and the best
  pure test of principle 10, because if *this* one needs code the principle is in real
  trouble.

**The real cost of this tier is sourcing, not programming.** Every number needs a defensible
origin under the provenance rule, and `[reversal]` is required rather than optional, which
means the file must state what the cell does below empty and what that costs it — a question
most datasheets do not answer and most published parameter sets do not either. Expect the
slice to be a day of reading and an hour of typing.

## Slice C: why NiMH and lead-acid hysteresis are one slice

Two earlier documents say this in writing, and this phase takes the recommendation rather
than re-deriving it:

* `lead-acid-data-only.md:294` — the diffusion term "is the *same* piece of state OCV
  hysteresis needs, so the two should be scoped together rather than as separate phases."
* `diffusion-overpotential.md:280` — "**Not NiMH, and not resting-voltage memory.** Those
  need a different state (a hysteresis term) and should be their own slice with their own
  version bump. A dead field costs more than a second small migration."

What both are pointing at: a cell whose open-circuit voltage depends on which direction it
was last driven. `CLAUDE.md`'s ECM section already reserves the room — "optional simple
hysteresis term per chemistry (needed to do NiMH/lead-acid justice later; can be stubbed for
LFP/NMC v1)". Nothing has ever un-stubbed it. Doing it once serves the nickel chemistries
*and* improves the lead-acid cell already shipped; doing it twice costs two migrations for
one idea.

The same `Option`-shaped absence the `[diffusion]` section uses applies here, for the same
reason and with the same structural payoff: a chemistry with no hysteresis block takes a
different code path rather than a neutral zero, so **no existing chemistry can move by a
ULP** and exit criterion 3 is structural instead of measured.

### The spike this slice needs first

Exit criterion 4 asks for a falling voltage at full charge to *emerge*. The ingredients
that might already be enough:

* charge the cell stops accepting near full, and the heat that makes — `energy-hole.md`
  built the charge-side path and `i_rejected_a` with it;
* a negative `docv_dt_v_per_k`, which `OcvTable` already carries as an optional column;
* the thermal network, which already makes a warming cell's OCV fall.

**Whether those three compose into a voltage peak followed by a fall of the right size is
not known.** It is a half-day spike with a hand-built parameter file, and it must run before
slice C's schema is designed, because the answer decides whether the phase needs a
charge-acceptance term as well as a hysteresis term. Phases 6 and 7 both spiked before
authoring; this is the same discipline.

## The stopping rule

This section exists because of what this phase is a reaction to. The guided-path
verification work ran for seventeen days and 79 commits without an exit criterion, because
it was never a phase and every plan document it produced ended by naming the next gap. It
found real defects the entire time, which is exactly what made it hard to notice.

So, in the same words the phase criteria use:

**This phase is done when criteria 1–4 close, with two chemistries taught.** It is not done
when the list of interesting chemistries is exhausted, because that list has no end.

Adding a chemistry *after* the phase closes is not a reopening — it is one slice against
this recipe:

1. Parameter file, provenance on every constant, `[reversal]` answered.
2. `chem` loading test, and a discharge that behaves.
3. Scenario file so a client can select it.
4. Guided-path steps, prose in digits, claims in `path-claims.toml`.

If step 1 ever requires a code change, that is a finding about principle 10 and belongs in
its own document — not silently in the chemistry slice.

## Deliberately not done here

* **No new cell model.** `Spm` and `Dfn` exist; nothing in this phase adds a variant to
  `CellModel`. A hysteresis term is a term inside the ECM, not a model.
* **No fitting pipeline.** `tools/reference/` generates goldens from PyBaMM; it does not fit
  parameters and this phase does not teach it to. Sourced parameter sets only.
* **Not every chemistry.** NiCd, LMFP, lithium-sulfur and solid-state are all real and none
  is in scope. NiCd in particular looks nearly free once slice C lands, and that is exactly
  why it is named here and not scheduled: "nearly free" is how the last unbounded body of
  work started.
* **No answer to how many lessons the path should have.** It has 24 today, this phase adds
  to it, and there is no argument in the repo about how long a guided path should be. That
  is worth having before a Phase 9 adds more.
