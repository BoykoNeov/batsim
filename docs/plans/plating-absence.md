# A cell that does not plate can say so

**Status: LANDED 2026-08-28. `SNAPSHOT_VERSION` 18 → 19**, `WASM_API_VERSION` unchanged at 6
and `sim_server::API_VERSION` at 2. The measurements are in the `# Measured` half at the
bottom; everything above it is the pre-work text as written, kept rather than corrected.

**Authored as pre-work on 2026-08-28.** The repo was at `SNAPSHOT_VERSION` 18 and
`WASM_API_VERSION` 6. Everything below the predictions was written before anything was run;
where a later section contradicts an earlier one, the later one is the measurement and wins.

## The defect, and where it is already written down

`docs/plans/phase-8-slice-a-lto.md` recorded it as *"the finding that is already certain,
from reading rather than running"*:

> **`[safety]` has no way to say "this cell does not plate lithium."**

Three routes were enumerated there and all three were rejected:

* **Zero the cost fields.** `plating_fade_per_ah = 0` means *the flag is raised and it costs
  nothing*. For a lithium-titanate anode the flag itself is the false statement.
* **Drop `[safety]`.** That does switch plating off — and switches thermal runaway off with
  it, because one `Option` covers two unrelated mechanisms. LTO is more thermally stable
  than NMC, not immune, and a file that says nothing about its onset temperature is worse
  than one that says it late.
* **Move the gate below every reachable temperature.** This is what shipped:
  `t_plating_min_k = 1.0`, one kelvin, labelled in the file as a sentinel rather than a
  measurement, with `plating_c_threshold = 0.0` behind it as a deliberate tripwire.

Slice C narrowed the gap without closing it: a **non-lithium** chemistry can say "does not
plate" cleanly by dropping `[safety]`, because it does not want the runaway half either.
`chemistries/nimh_subc_3ah_generic.toml` does exactly that. LTO is still wearing the
sentinel, and the schema still cannot express the thing.

## The design, and the alternative that was considered and rejected

**Chosen: the two gate fields become optional, and their absence *is* the statement.**

```toml
[safety]
t_onset_k = 493.15
t_vent_k  = 533.15
# ... the runaway half, unchanged ...
# and no t_plating_min_k, no plating_c_threshold: this cell has no plating mechanism.
```

Three validator rules, all in an idiom `chem.rs::validate` already uses twice
(`plating_short_ohms` is required only when the hazard rate is positive;
`runaway_ea_j_per_mol` only when the amplitude is):

1. **Both gate fields present, or both absent.** A threshold without a temperature gate
   parameterises nothing, and a temperature gate without a threshold cannot be evaluated.
   This rule is also what makes the schema robust to a *typo*: `chem.rs` does not deny
   unknown TOML keys, so a misspelled key silently becomes an absent one — but misspelling
   exactly one of a matched pair is caught here, and misspelling both is not a plausible
   accident.
2. **When present**, the existing sign and finiteness checks, unchanged.
3. **When absent, the three cost fields must be zero.** This one is a *decision against the
   local precedent*, and is written down here so it does not look like an accident. The
   surrounding fields are permissive — "ignored when it is not" is the phrasing used twice
   in the same struct. Rejecting is right here for two reasons. A fade-per-amp-hour for a
   mechanism the cell does not have is an invented physical constant, which `CLAUDE.md`'s
   provenance rule forbids outright; and it is what **replaces the tripwire this change
   deletes**. The LTO file's `plating_c_threshold = 0.0` exists so that a future reader who
   "corrects" the sentinel upward gets a loud wrong answer rather than a plausible one.
   After this change there is no sentinel to correct, and rule 3 is the thing that catches
   the other half — a file that declares plating costs while claiming no plating.

**Rejected: `t_plating_min_k = 0.0` meaning "never".** This is the third in-section idiom
in the same struct — `0` means inert for `plating_fade_per_ah`, for
`plating_short_hazard_per_ah` and for `runaway_power_w_at_onset` — and it is strictly
cheaper, because it changes no serialized layout and therefore costs no `SNAPSHOT_VERSION`
bump. It is rejected on two grounds:

* **It is still a sentinel**, just a better-mannered one. Absolute zero is a defensible
  "below every temperature", but the reader still has to be told that a temperature means
  the absence of a mechanism, which is the shape of the defect rather than its fix.
* **`plating_c_threshold` would remain required and meaningless.** The LTO file would still
  carry a C-rate for a mechanism it does not have. Absence removes both fields together;
  zero removes neither.

The repo expresses "this mechanism is not here" by **absence** in five places already —
`[spm]`, `[dfn]`, `[aging]`, `[diffusion]`, `[hysteresis]` — and the only reason plating
could not join them is that it shares an `Option` with runaway. Choosing a weaker schema to
avoid a bump the rules sanction is not a saving worth making.

**Not chosen, and worth saying why not: a `[safety.plating]` sub-table.** It expresses the
same thing with a smaller validator, but it moves fields out from under `[safety]` in three
shipped files and diverges from `CLAUDE.md`'s example block, which is a much larger blast
radius for the same outcome.

## The snapshot bump is real, and it was checked rather than assumed

`SNAPSHOT_VERSION`'s own doc states the rule and `bincode` decides it: struct fields are
written positionally with no framing, the chemistry is serialized **inside** every
snapshot, and an `Option<f64>` is a tag byte plus a payload where an `f64` was eight bytes.
So **v18 → v19**, for the same mechanical reason v14, v15, v17 and v18 all bumped.

The `v10 unmoved` precedent — an added `#[serde(default)]` field that round-trips both ways
— does not apply. That argument works for a *self-describing* format and for an appended
optional field; this is a **type change** on two existing fields, and under a positional
format it moves everything after them.

`sim_server::API_VERSION` and `sim-wasm`'s constant both stay put: no call signature
changes, no telemetry field is added or removed, and no client can see the difference except
by loading a chemistry file that omits two keys.

## Predictions, registered before anything is run

**P1 — the floor does not move, bit for bit.** Every shipped chemistry except LTO keeps both
gate keys, so `validate` sees `Some(..)` and `plating_risk` computes what it computed
before. LTO's own trajectory is unmoved too, and by a *stronger* argument than "the numbers
are the same": its gate was unreachable, so the predicate was already constant-`false` on
every step, and it is constant-`false` afterwards for a different reason. No golden moves.
*Confidence: high.*

**P2 — the version pair flips back to "the version field is what refuses it", for the
fixture, and this is the first bump in four where it does.** `snapshot_version.rs`'s
chemistry has `safety: None`, so no byte of its snapshot changes across this bump: a genuine
v18 blob of that pack is structurally valid at v19 and only `Pack::restore`'s version check
stands between it and a build it was not written for. That is the v10/v11 situation
returning after v17 and v18 both said "it does not parse at all". *Confidence: high — it
follows from the fixture having no `[safety]` section, which is read, not guessed.*

**P3 — a chemistry that *does* carry `[safety]` is the interesting half, and I predict LOUD
with a live quiet alternative.** A v18 blob writes `t_plating_min_k` as eight raw bytes; v19
reads the first of them as an `Option` tag. For 273.15 the little-endian first byte is
`0x00` — a **valid `None` tag** — so v19 consumes two bytes where v18 wrote sixteen and
every field after shifts by fourteen. My prediction is that the shift blows up downstream on
a `Vec` length or an enum tag and the blob fails to parse. The alternative — that it parses
into a plausible-looking wrong pack, the v16 quiet failure — is live and is the reason this
is measured rather than asserted. **Whichever way it lands, it is written into the v19
note.** *Confidence: moderate, and deliberately so.*

**P4 — the LTO plating test passes for a new reason, and needs a control arm to stay
honest.** `cold_fast_charge_plates_the_nmc_cell_and_not_the_lto_cell` currently passes
because one kelvin is unreachable; afterwards it passes because the mechanism is absent.
Same green, different cause, and the test stops discriminating between "the gate is shut"
and "the gate is gone". A control arm — the LTO parameters with a *graphite* gate spliced
in, under the identical demand — turns the absence into a subtraction. I predict the spliced
arm raises `PLATING_RISK`. *Confidence: high; this is the arm, not the question.*

**P5 — one guard's arm becomes unreachable, and gets stricter rather than deleted.**
`load.rs::shipped_plating_coefficients_give_a_plausible_cold_charge_cost` has an arm for "no
cost declared, so the gate must be shut", written because the schema could not say the other
thing. Afterwards no shipped file reaches it: the three lithium files price plating, and LTO
has no gate. The replacement is the *stricter* rule the change makes expressible — a shipped
file that carries a plating gate must price it, or drop the gate — and it is a rule about
shipped files rather than about the schema, which still permits a priced-at-zero gate.
*Confidence: high on the unreachability, which is enumerable; the replacement is a choice.*

**P6 — no lesson prose and no claim moves.** Checked before writing this: the step-26 prose
says "this cell has no plating to bill" and "the difference between the two runs is a
parameter file", both of which stay true, and no `[[claim]]` in `web/path-claims.toml` reads
the sentinel value or the gate. The claim that reads the *absence* of the flag is already
written as pointing at the Rust test rather than at a quantity, because no quantity in that
file can assert a flag that never fires. *Confidence: high; this is a grep, not a guess.*

## The prose that becomes false, and where

Every one of these asserts the gap in the present tense and has to move with the fix:

| file | what it says |
| ---- | ------------ |
| `crates/sim-core/src/chem.rs` | the field doc block headed *"There is no way to say this cell does not plate"* |
| `chemistries/lto_20ah_generic.toml` | ~15 lines: the `PLATING` preamble, the sentinel comment, the tripwire comment, and the `[cell] t_charge_min_k` cross-reference near the top |
| `chemistries/nimh_subc_3ah_generic.toml` | the note recording that LTO had to ship a sentinel and NiMH did not |
| `scenarios/cold_charge_nmc.toml` | the comment calling the LTO gate a sentinel of one kelvin |
| `crates/sim-data/tests/lto_chemistry.rs` | the plating test's doc, which points at the sentinel discussion |
| `crates/sim-data/tests/load.rs` | the guard's comment naming itself *"the second place in the repo that assumed every cell plates"* |
| `docs/plans/phase-8-slice-a-lto.md` | the finding, which is now closed rather than certain |
| `docs/plans/phase-8-chemistries.md` | slice A's row, which states the caveat |
| `docs/plans/phase-8-slice-c-hysteresis.md` | the note saying the gap is narrowed but open |

`CLAUDE.md`'s `[safety]` example block keeps both keys and needs no edit: they are the right
thing for the graphite cell it illustrates, and the block is a shape rather than a source.

## What would falsify the whole slice

That some existing trajectory moves. P1 says none does, and the argument is structural
rather than numerical, so a single moved golden means the argument is wrong and not that a
tolerance is tight.

---

# Measured

**LANDED 2026-08-28. `SNAPSHOT_VERSION` 18 → 19**, `sim_server::API_VERSION` and
`sim-wasm`'s constant unmoved. `cargo test --workspace --no-fail-fast`: 70 test binaries,
607 tests, all green. `cargo clippy --workspace --all-targets -- -D warnings` and
`cargo fmt --all` clean. Everything below was run, not reasoned about.

## The predictions, scored

| | outcome |
| --- | ------- |
| **P1** — the floor does not move | **HELD.** Nothing moved. Every golden — ECM, SPM, DFN, lead-acid, NiMH — is unchanged, and no tolerance was touched to keep it that way. |
| **P2** — the version pair flips back to "the version field is what refuses it" | **HELD.** The fixture chemistry has `safety: None`, so its snapshot is byte-identical at v18 and v19, and the retag pair is the real case rather than a stand-in. First bump since v11 where that is true. |
| **P3** — a chemistry *with* `[safety]` is loud, with a quiet alternative live | **HELD, and the derivation inside it was WRONG.** The prediction named `0x00` as the first byte of `273.15`; it is `0x66`. The verdict is unchanged — `0x66` is not a valid `Option` tag, so the shipped case is loud — but the *reason* had to be measured rather than derived. The quiet alternative is real and is now pinned: a round `273.0` begins `0x00`, a valid `None`, and the stale section parses into a cell claiming it cannot plate with every field after the gate slid along. Loudness at this bump is **value-dependent**, which no earlier bump in `snapshot_version.rs` has been, and the v19 note therefore does not claim the loud direction the way v17's and v18's do. |
| **P4** — the LTO plating test passes for a new reason and needs a control arm | **HELD, and perturbation 5 below proves it empirically rather than by argument.** |
| **P5** — one guard's arm becomes unreachable and gets stricter | **HELD.** `shipped_plating_coefficients_give_a_plausible_cold_charge_cost` now reads: no gate ⇒ nothing may price plating; gate ⇒ it must be priced. |
| **P6** — no lesson prose and no claim moves | **HELD.** `web/path-claims.toml` and `web/app.js` are untouched, and `every_claim_matches_the_engine` is green. |

## The perturbation table

Five cases, each breaking exactly one thing, with a green baseline before and after. What is
recorded is **which** tests reddened, not just that the exit code moved — a red exit code can
be the wrong check reddening.

| # | broken | verdict | reddened |
| - | ------ | ------- | -------- |
| 1 | `plating_risk` ignores the absent gate | RED | `a_chemistry_with_no_plating_gate_never_plates`, `cold_fast_charge_plates_the_nmc_cell_and_not_the_lto_cell` — and *not* the NMC control, which is the right shape |
| 2 | the both-or-neither rule dropped | RED | `half_a_plating_gate_is_rejected` |
| 3 | the costs-without-a-gate rule dropped | RED | `plating_costs_without_a_plating_gate_are_rejected` |
| 4 | the LTO file prices plating while having no gate | RED | 14 tests — everything that loads the file, because it no longer validates. This is the tripwire replacement firing on the *shipped* file rather than on a fixture |
| 5 | the LTO file carries its old sentinel gate again, priced at nothing | RED | `shipped_plating_coefficients_give_a_plausible_cold_charge_cost`, `the_lto_silence_is_caused_by_the_absent_gate` |

**Case 5 is the finding.** Restoring the sentinel leaves
`cold_fast_charge_plates_the_nmc_cell_and_not_the_lto_cell` **green** — the behaviour test
cannot tell a shut gate from an absent one, exactly as P4 said, and now that is a
measurement rather than a worry. Two things catch the sentinel instead: the shipped-file
guard, which refuses a gate nobody prices, and the control arm added here, which asserts the
shipped file has no gate before it splices one in. Had the control arm not been written, the
old sentinel could have been reinstated and the plating suite would have stayed green.

## What was found on the way

* **The bump is unavoidable for any fix.** `SafetyParams` is inside every snapshot and
  `bincode` is positional, so *any* schema change here costs the same version. That removes
  the cheapest argument for the zero-means-inert alternative — it does not save a bump for
  free, it saves it by declining to change the schema at all.
* **`plating_c_threshold = 0.0` in the shipped LTO file was a tripwire, and deleting it
  needed a replacement rather than a note.** Validator rule 3 is that replacement, and case 4
  is the proof it covers the shipped file and not just a fixture.
* **The guard could get stricter, not just adapt.** "A gate that nobody prices" was legal
  before only because a non-plating cell had no other way to be written. It is now refused
  for shipped files, which is a rule the old schema could not express.

## Deliberately not done

* **`[safety]` is still one `Option` covering two mechanisms.** A chemistry that wants
  plating but not runaway still cannot say so. Nothing shipped wants that — the four
  lithium files want both, the two non-lithium files want neither — so it stays a real but
  unexercised asymmetry rather than a second schema change with no file behind it.
* **The runaway trio is untouched.** `runaway_power_w_at_onset = 0` still means "onset is
  reported and nothing burns", which is the permissive convention rule 3 deliberately
  departs from on the plating side. The two are not made consistent here, because the
  plating side has a chemistry that needs the strict reading and the runaway side does not.
* **No lesson step, and none is needed.** Step 26 already teaches that the LTO cell does not
  plate, and its prose — "this cell has no plating to bill", "the difference between the two
  runs is a parameter file" — is true before and after. What changed is how the file says
  it, which is a fact about the schema rather than about the cell.
