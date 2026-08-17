# Eight voltages under one name, and the step that could not quote any of them

`docs/plans/path-ledger-particle-step.md` measured step 16 and found the blocker was not
the one the claims file had written down. The file said its twin readings "need a second
pack". They do — but a second pack is exactly what `Tie::Quoted` was built to avoid, and
the reason it could not be used is smaller and entirely inside this repo: **step 15 files
eight voltage readings under the single name `v_at`**, and the fence built one slice ago
refuses to resolve a name that answers differently depending on which claim the file lists
first.

So step 16 — a step whose whole argument is "the same cell, the same current, and the model
next door says something else" — could borrow nothing from the step it is arguing with.

This slice makes a reading addressable: `v_at:400` is the reading at 400 s, and the tag is
checked against the instant the claim already declares.

## What a tag is, and what it is not

* It is **the instant**, and `measure` asserts it equals the claim's `read_at_s`. A tag and
  a `read_at_s` that disagree are two addresses for one reading, which is the thing this is
  supposed to end, so it fails rather than picking one.
* It is **not a label**. `v_at:first` does not parse, so it never reaches the fallback and
  lands on the unknown-quantity panic instead — by name, with the known list, and now with a
  line saying the tag has to be a number. There is nothing to name but the instant.
* It changes **no measurement**. The tag is stripped and the same row is read — the whole
  of the machinery is one fallback in `measure`, entered only when the untagged name is not
  a row quantity.
* It reaches **only row quantities**, which is what keeps it from colliding with the two
  parameterised families already in the file: `pulse_rebound_mv:1` counts teeth and
  `t_at_v_below:2.5` names a threshold. Neither prefix is a row quantity, so neither can
  fall into this path.

Ten claims on step 15 take a tag — the eight voltages and the two states of charge, which
share a name across the step and its arm. `q_gen_at` does not: it names one reading on that
step and was quotable already, which is why the step-16 measurement could already promise
its `6.33 W`.

## Scoped to one step, and the rest of the file is priced rather than ignored

The same ambiguity exists elsewhere. With step 15 tagged, **22 groups of claims still share
a name and answer differently — 80 claims of the file's 186** — counted the way
`Tie::Quoted` sees them, by step and quantity. (Keyed by step, arm *and* quantity it is the
same 22 groups and 73 claims; the difference is readings that differ only because one is on
an arm, which the tie does not currently distinguish.) Tagging all of them would be a bigger
and more mechanical change than this one, and it is deferred for a reason that is not only
size: exactly two of the ambiguous names are not row quantities — `soc_lost_pts_at` and
`t_rise_k_at` are reductions over a trajectory — so the tag would have to grow a second
implementation to reach them. That is a slice with its own
question — whether every reading should be addressable or only the ones a sentence quotes —
and it should not be smuggled in behind this one.

What is *not* deferred is the statement of where the file stands: the fence still refuses
any quotation of those 23 groups, loudly, naming the values that disagree.

## The honest position on coverage

Nothing quotes step 15 yet. `Tie::Quoted` is reachable only from a ledgered step's
vocabulary and step 16 is not ledgered, so this slice ships a capability one step ahead of
its sentence. Two things stand in for that missing user, and both run today:

* `measure`'s assert, exercised by all ten tagged claims on every run of
  `every_claim_matches_the_engine`.
* `a_tagged_reading_resolves_to_that_reading`, which quotes `v_at:400` and requires it to
  resolve to that claim's value — and quotes an instant in the *middle* of the run, so a
  resolution that silently took the first or last claim would fail it.

The second test also carries its own evidence guard, on the pattern this file uses
elsewhere: if a later slice retags or reorders those readings, it says so instead of
passing on nothing.

## The case that moved, and why that is the good outcome

`a_quotation_of_a_quantity_two_claims_disagree_on_is_refused` pointed at step 15. It cannot
any more — that is the entire success condition of this slice — so it now points at step 20,
which files ten voltages under `v_at` and has no sentence quoting it. The test's own guard
is what made this safe: it asserts that its chosen quantity really is answered differently
by two claims, so repointing it could not quietly turn it into a test of nothing.

## Perturbations, registered before the run

| edit | must redden |
| --- | --- |
| `v_at:400`'s tag → `v_at:401` | `measure`'s tag assert, and the quotability test's lookup |
| that claim's `read_at_s` → `440.0` | the same assert from the other side |
| `v_at:400` → `v_at` (tag dropped) | the quotability test, and nothing else — which is what that test is for |
| **the tag assert deleted, with the `v_at:401` retag applied** | **nothing — a registered GREEN**, which is what prices the assert |
| the `should_panic` case repointed at step 15's voltages | its own evidence guard, because the ambiguity is gone |
| `soc_at:1060`'s tag → `soc_at:1058` | the tag assert, on a claim read on an arm rather than the step's own run |
| `measure`'s fallback made to dispatch on the tagged name | every tagged claim at once |
| `v_at:400` → `v_at:first` (a tag that is not a number) | the unknown-quantity panic, loudly and by name — not a silent mismeasurement |

### What the table found

Five of the seven came back as registered. Two did not, and both were the table doing its
job.

**The `should_panic` guard could not fail.** Repointing that test at step 15 — whose
readings are now tagged, so the filter finds nothing — was registered as a red and came back
green. The reason is that the guard's message contained the words `should_panic` was
matching on: the guard fired, the attribute saw its expected phrase, and the test passed
while proving nothing at all. That is exactly the "passing on nothing" the test's own doc
comment claims it prevents, and it was true before this slice as well as after. The guard is
reworded to fail in words the attribute cannot mistake for a refusal, and the case is red
now. **A guard on a `should_panic` test needs vocabulary that does not overlap the panic it
is guarding.**

**The registered green was registered on the wrong claim.** "Neuter the tag assert, apply a
wrong tag, and nothing should notice" came back red — because the claim it retagged was
`v_at:400`, the one the quotability test names by hand, so that test caught the rename
rather than the mistagging. Repeating it on `soc_at:1060`, which no test names, is green as
registered.

**The green is only evidence as half of a pair.** On its own, "nothing failed" could mean
the mistagging is harmless. It is the *same edit* as the `soc_at:1058` row above — which is
red — with the assert neutered and nothing else changed. Red without the neutering, green
with it, is what pins the catching to the assert rather than to anything else in the run.
For the one reading a test names, that test is a second guard; for the other nine, the
assert is the only thing standing between a tag and a `read_at_s` that disagree.

## Learned while building

**A perturbation that fails on the wrong assertion is a green in disguise.** Both surprises
above are the same shape — a case reddened or stayed green for a reason that had nothing to
do with what it was testing. The tell in each was that the *failing test name* did not match
the case's intent. Reading which test failed, and not only whether the run failed, is what
separated them.

**Isolate a fence by perturbing something no other test names.** The quotability test picks
`v_at:400` deliberately, so it is the worst possible claim to use when measuring what some
*other* guard is worth. Pricing a fence means choosing an input that only that fence sees.
