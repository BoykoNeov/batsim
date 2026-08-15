# The last accounting arm, and the deadlock it was half of

`docs/plans/path-prose-ledger.md` laid out six ways a number a reader is shown can be tied
to something, and named the last one it did not build:

> **The `Derived` arm is the one with no precedent.** Every other arm checks a token against
> a file; this one checks a token against other tokens in the same sentence, which needs the
> sentence parsed into an expression. The cheap version — a list of declared identities — is
> a declaration, which is the thing the design refuses. Getting it wrong re-opens the hole
> rather than narrowing it.

This slice builds it, on the one sentence in the path that needs it and can pay for it
today. It also breaks a deadlock: the arm and the capability its target sentence needs each
cited the other as the reason it could not be built.

## The deadlock, written down twice

`crates/sim-data/tests/path_claims.rs` refuses a mark-side ambient drag:

> `ambient_c` is **restart-side only**: `run` keeps one `Env` for the whole trajectory …
> Step 8 asks a reader to raise the slider *at* the mark, and that arm needs the environment
> split in two — and the sentence that would pay for it prints `20 K` and `2.7×`, both
> figures derived from their siblings, so it is blocked on the accounting arm below as well.

And the accounting arm was blocked on there being a sentence to account. Neither half is
buildable alone: `2.7×` cannot be claimed until `2.84` can be measured, and `2.84` is a
measurement on a trajectory that does not exist until the environment splits. So both.

## The sentence

Step 8, `wearing-out-while-idle` — a 4S2P LFP pack at rest, aging on, run to 200 000 s at
25 °C, then the reader is told to raise the ambient slider to 45 °C and press Run:

> Then the ambient: 20 K buys 2.84 points over the next 200 000 s against the 1.06 the first
> leg cost, about 2.7×.

Five numbers, and after this slice each is tied to something:

| number | tied to |
| --- | --- |
| `20` | the **ambient step** of the trajectory this sentence's claims read — 45 °C dialled in against the step's own 25 |
| `2.84` | a claim: capacity lost since the mark, measured on the hot arm at 400 000 s |
| `200 000` | the instant that claim is read at, in the sentence's own frame (since the mark) |
| `1.06` | a claim: the complement of `soh cap` at the mark |
| `2.7` | **`Derived`** — the quotient of `2.84` and `1.06`, both printed in this sentence |

Measured before anything was written (temporary harness, deleted): the first leg costs
1.0579 points, the second 2.8375, and the ratio of the two is 2.6822. The prose's three
figures are all exact at the precision it prints them to, and the ratio taken from the
*printed* numbers — 2.84 / 1.06 = 2.6792 — rounds to the same 2.7.

The literal stops at `about 2.7×`. The next sentence says "Not the 3.5× two fresh packs
would show at those temperatures", and that is a counterfactual about packs this scenario
never builds: no trajectory produces it and no file holds it. Running the literal into it
would demand an accounting that cannot be built, and the honest answer there is that the
sentence stays unchecked rather than that the arm stretches to cover it.

## `Accounted::Derived`

**The token is the sentence's own arithmetic over numbers printed in the same sentence, and
every operand must itself be accounted by one of the other arms.** That last clause is what
makes it a tie rather than a circle: if `2.84` were free, then `2.7 = 2.84 / 1.06` would
say only that two unpinned numbers divide into a third.

Declared: which operand tokens, and which operation. Never the value — the quotient is
recomputed and compared at the precision the prose itself commits to, through the page's
own rounding rule (`to_fixed`), the same comparison `Tie::Product` already uses for
`4.61 Ah`. Point a row at the wrong operand and it fails on sight, because the arithmetic
stops reproducing the digit.

The declaration lives in a new `[[derived]]` table in `web/path-claims.toml`, not in a claim:
the number being accounted has no claim of its own, which is the whole reason it needs an
arm.

```toml
[[derived]]
step    = "wearing-out-while-idle"
literal = "20 K buys 2.84 points over the next 200 000 s against the 1.06 the first leg cost, about 2.7×"
spells  = "2.7"
op      = "ratio"
from    = ["2.84", "1.06"]
```

Fences, each of which is a way the arm could have been a waiver:

* **Every operand must appear in the literal**, as a number the scanner finds. A `from`
  naming a token the sentence does not print is the declared-identity shape the plan
  refuses — it would let an author supply both sides of the arithmetic.
* **Every operand must have a non-`Derived` accounting.** Chains are refused rather than
  resolved: a derived number derived from a derived number has no floor.
* **The result must not be an operand**, and the operands must differ from each other.
* **`op` is a closed enum with one variant**, `ratio`. A second variant gets built the day a
  sentence prints one, which is this file's standing rule against arms with nothing to
  account (`CCCV_PERIOD_S` sat pinned and unread for six slices).
* **Every `[[derived]]` row must be used** — its literal must be a sentence some claim
  quotes, and its `spells` must be a number in it. A row left behind by a prose edit fails
  here instead of sitting in the file looking like coverage.
* A token that is *also* spelled, read at, shown or a setting is refused rather than
  resolved, which is the existing `clash` and is what stops two readings of one number.

**This is the check-6 taxonomy, not the ledger's.** `Tie::Derived` is still missing, and the
step that needs it — `slow-and-patient`'s "six of these in series is the 12 V battery" — is
not ledgered yet. `Setting` already exists in both taxonomies, so one arm per taxonomy,
built when a sentence needs it, is the precedent rather than a duplication.

## The two capabilities underneath it

### The environment splits at the mark

`run` builds one `Env` and uses it for the probe, the pre-mark drive and the arm's actions.
It becomes two: the step's slider before the mark, the arm's after. A restart arm has no
pre-mark segment, so nothing about the existing arms moves — the split is observable only on
a continuation that overrides `ambient_c`, of which there were none.

The refusal in `every_arm_is_instructed_by_its_own_step` comes down with it. It was
explicitly a scoping refusal and not a fidelity one: `$("ambient").oninput` calls `applyEnv`
and rebuilds nothing, so the page really can do this mid-run.

### `soh_cap_fade_since_mark`

`2.84` is not a value the pack has; it is how far its capacity fell over the second leg.
`Complement` gives 3.90 points (the total from new) and `SinceMark` is a duration frame for
instants. So a quantity: `soh_cap_at` at the mark minus `soh_cap_at` at the read instant,
which the arm's own trajectory carries because a continuation arm runs the pre-mark stretch.

Fenced to instants past the mark — before it the quantity is a fold over a stretch that has
not happened, and at it the answer is zero for every possible sentence.

## Perturbations, run

Eleven edits, each applied to a green tree, the suite run, the tree restored. **All eleven
reddened, every one on the check named before it was run**, and the restored tree is green.
Several also redden the self-count tests, which is that contract working and is not listed.

| edit | reddened |
| --- | --- |
| `2.7×` → `2.8×` in the prose and in all three literals and `spells` | the derivation check: the sentence's own arithmetic no longer comes to what it prints |
| `2.84` → `2.90` in the prose, the literals, `spells` and `from` | **check 3**, the claim's spelled number against its measured value — before any engine runs |
| `from = ["2.84", "1.06"]` → `["1.06", "2.84"]` | the derivation check: the ratio inverts to 0.4 |
| `from` reads `"200 000"` instead of `"2.84"` | the derivation check: 200 000 / 1.06 is not 2.7 |
| `from` reads `"2.8375"` — the engine's number, which the prose does not print | the derivation check, on the operand-not-printed fence |
| the `[[derived]]` row is deleted whole | check 6, on `2.7` accounted by nothing |
| the `2.84` **claim** is deleted, leaving the arithmetic intact | the derivation check, on the operand having no accounting — hand-validated, because this is the fence the whole arm rests on |
| `20 K` → `19 K` in the prose and the literals | check 6: 19 is not the step the arm drags the slider by |
| the hot arm's `ambient_c` → 25 | the arm check, on an override that changes nothing |
| the hot arm's `ambient_c` → 40 | the arm check, on `40` not appearing in the sentence that instructs it — *not* the claim against the engine, which is the arm's number and its instruction being one statement |
| the env split is reverted to one `Env` | the `2.84` claim against the engine: the whole run at 45 °C fades further than the sentence says |

Two of them are worth naming. The `2.84 → 2.90` row was expected to fail against the engine
and fails one check earlier, without running one — the claim's own spelled number and its
measured value disagree, which is check 3 doing exactly its job and is a reminder that the
cheap checks fire first. And the `ambient_c → 40` row is a **confounded perturbation**: it
cannot reach the claim, because the arm's control and the sentence instructing it are pinned
to each other one test sooner. That is a real property rather than a miss, but it is the
reason the env split needed its own row — reverting the split is the only edit that reaches
the trajectory without tripping something in front of it.

## Deferred, with a price

* **`Tie::Derived` is still missing from the ledger**, and `slow-and-patient` still needs it
  along with a quotient tie for `C/20` and a table-span tie for its `180 mV`. Nothing here
  makes that step ledgerable on its own; it makes it one arm cheaper.
* **Reading `six` as an operand is not built here**, because no sentence this slice touches
  needs it. When `12 V` gets its arm, note in one line that reading one word numeral as an
  operand is *not* the general word-numeral scanner — otherwise a future author reads a green
  ledger as word coverage, which is the gap that widens every slice.
* **The rest of step 8's prose is still unchecked.** This slice claims one more sentence in
  an unledgered step. `3.5×`, `95 %`, `10 000×` and the rest are exactly as unguarded as
  before.
* **How far the hot arm runs is this file's own choice**, so accounting `200 000` as the
  since-mark reading of its claim's instant is the weaker statement the module docs already
  price for every continuation arm. What is not circular is that `2.84` is measured *at* that
  instant: run the arm to a different length and the fade no longer matches the sentence.
