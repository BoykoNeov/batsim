//! The bump where `Snapshot::version` finally does its job.
//!
//! # The caveat this file retires
//! Every `SNAPSHOT_VERSION` note from v3 to v9 carries the same sentence:
//!
//! > Note what actually rejects an older snapshot here: the layout change, at
//! > deserialization. **The version check never sees those bytes.**
//!
//! Under a self-describing-by-order format every one of those bumps came with a
//! structural change, so an older blob failed to *deserialize* long before
//! `Pack::restore` looked at its version field. The check was correct and untested,
//! and each note said so.
//!
//! v10 was different, and v11 was different in exactly the same way. Adding a variant to
//! an externally-tagged enum does not change how the *existing* variants serialize —
//! `CellModel::Spm` at v10, `CellModel::Dfn` at v11 — and the chemistry's `[spm]` and
//! `[dfn]` sections are both optional, so the older blob is **still structurally valid**
//! at the newer version. It would restore into a working pack whose semantics this build
//! no longer guarantees, and `Pack::restore`'s version check is the only thing standing
//! there.
//!
//! # The pair moves with the bump, rather than being renumbered
//! This file used to pin v9 -> v10, then v10 -> v11, v11 -> v12, v12 -> v13, v13 -> v14,
//! v14 -> v15, v15 -> v16, v16 -> v17, v17 -> v18, v18 -> v19 and v19 -> v20, each time
//! carrying an assertion that a later bump needs its own pair. This is the v20 -> v21 pair,
//! and it was re-argued rather than renamed.
//!
//! **The v19 -> v20 pair could not be kept alongside this one**, on the same terms every
//! retirement here has been made: [`retagged`] fabricates a stale blob by writing *this
//! build's* bytes under a fake tag, and the only tag those bytes can honestly wear is the
//! previous version's, not one two back. As at the last two bumps the retirement is **not**
//! structural — see the v21 section below, which is the third bump running where the
//! fixture's bytes do not change at all.
//!
//! # v21: one bare `f64` behind an `Option`, and the stale blob is quiet either way
//! v21 adds `ChemistryParams::charge_acceptance`, an optional section holding a single
//! `f64`, between `hysteresis` and `aging`. See `docs/plans/charge-acceptance.md`.
//!
//! This fixture's chemistry does not declare it, so its bytes do not move and the pair
//! below is the real case — the third bump running, and still a fact about the fixture.
//!
//! The other half is [`a_v20_shaped_chemistry_tail_misparses_at_v21`], and it is the first
//! sibling in this file whose outcome does **not** depend on a value. The reader takes
//! `aging`'s presence byte as the section's, as at v20; but where v20's payload was a table
//! whose length prefix came out of a `f64` and failed, v21's payload *is* a `f64`, and any
//! eight bytes are one. A chemistry with `[aging]` therefore parses its `cal_pre_exp` into
//! the taper onset and carries on displaced; one without slides as at v20. Quiet in both
//! directions, which is the v16 hazard reached by the v20 mechanism.
//!
//! # v19: the fixture's bytes do not move, and the version field is the only thing left
//! v19 changes `SafetyParams`, not the cell state: `t_plating_min_k` and
//! `plating_c_threshold` become `Option<f64>`, so a chemistry can say it has **no plating
//! mechanism** by omitting them rather than by spelling "never" as an absurdly low
//! temperature. See `docs/plans/plating-absence.md`.
//!
//! The chemistry is serialized inside every snapshot, so that is a layout change and it
//! takes the bump. But this file's fixture chemistry has **`safety: None`**, and an absent
//! `Option` writes one tag byte whatever is inside it — so a snapshot of this pack is
//! byte-for-byte identical at v18 and at v19. That is the v10/v11 situation returning after
//! three bumps of "the stale blob does not parse at all": a genuine v18 blob of *this* pack
//! is structurally valid here, would restore into a working pack, and the only thing
//! standing between it and a build it was not written for is `Pack::restore`'s version
//! check. The pair below is therefore not a fabrication standing in for the real case — for
//! this fixture it **is** the real case.
//!
//! A pack whose chemistry *does* carry `[safety]` is the other half, and it is a
//! field-level measurement rather than a claim:
//! [`a_v18_shaped_safety_section_does_not_parse_at_v19`]. What it found is that loudness
//! there is **value-dependent**, which is a property no earlier bump in this file had had.
//!
//! # v20: the same shape a second time, and the deciding byte is in the next field
//! v20 appends `width_over_soc` to `HysteresisParams`, so a cell whose resting-voltage
//! memory is wider at one end of its range than the other can say so. See
//! `docs/plans/hysteresis-width-over-soc.md`.
//!
//! This fixture's chemistry has **`hysteresis: None`**, so once again its bytes do not move
//! and the pair below is the real case rather than a fabrication — the second bump running
//! where that is true, which is a fact about the fixture and not a trend.
//!
//! The other half is [`a_v19_shaped_hysteresis_section_does_not_parse_at_v20`], and it is
//! value-dependent like v19's, for a **different** reason worth keeping the two apart for.
//! At v19 the deciding byte was the low mantissa byte of a number *inside* the section that
//! changed. At v20 the new field is appended at the section's end, so the byte the reader
//! takes as its presence tag belongs to the **next field of the enclosing struct** —
//! `ChemistryParams::aging`, itself an `Option`. Whether a stale blob is loud is therefore
//! decided by whether the chemistry ages, which is not a property of the section that
//! changed at all.
//!
//! # v16 is the first bump since v11 whose stale blob stays aligned, and the failure is
//! # silent rather than loud
//! v14 and v15 had no structurally-valid stale blob at all (below). v16's is the opposite
//! case and the more dangerous one: under `bincode` a one-pair `Vec<f64>` is an 8-byte
//! length followed by one 8-byte value, and `[f64; 2]` is two 8-byte values — **the same
//! sixteen bytes**. So a v15 field does not fail to parse at v16; it parses, into a
//! subnormal `5e-324` where the length was and the real overpotential shifted one slot
//! along. [`the_v15_v_rc_bytes_reinterpret_rather_than_fail`] demonstrates that at the
//! field level, which is the level it can honestly be demonstrated at: fabricating a whole
//! v15 *snapshot* would mean inserting a length prefix at four unlocated offsets, so this
//! file does not claim the whole-blob case and the version note does not either.
//!
//! That test is kept even though the bump it describes is now two behind, because the
//! [`SNAPSHOT_VERSION`] note for v16 still leans on it and because it is the only
//! demonstration in the tree of the quiet failure mode. It costs nothing: it constructs
//! its own bytes and touches no fixture.
//!
//! # v17 flipped the answer back, and v18 keeps it there — asserted, not described
//! v17 added a sixth `f64` to every `EcmState` and v18 adds a seventh. `bincode` writes
//! struct fields positionally with no framing, so a v17 cell state is eight bytes short of
//! what v18 reads and there is no arrangement of a v17 pack whose bytes parse here — the
//! v14/v15 situation, reached by the v14/v15 mechanism.
//! [`a_v17_shaped_cell_state_does_not_parse_at_v18`] is the sibling of the v15 field test
//! above and asserts the opposite outcome, so that "v18 is loud where v16 was quiet" is a
//! measurement in this file rather than a claim repeated from the version note. It also
//! keeps the v16-shaped case, which still fails and for the same reason at one more field
//! of distance — a free assertion, and the one that would notice a future edit reordering
//! the struct rather than appending to it.
//!
//! **The answer changed with three consecutive bumps and has now held for two**, which is
//! the standing reason a pair here is never renumbered: a held answer is still a fact about
//! one specific layout change, and v18's is not v17's.
//!
//! # What v18 does *not* inherit from v17, and it is the interesting half
//! Both bumps are semantic as well as structural, but their `#[serde(default)]` stories are
//! opposite at the one place it matters. v17's `depletion` defaults to `0.0`, which is the
//! *correct* reading for a pack that has been resting — the field decays in time. v18's
//! `hysteresis` decays in **charge moved**, so a rested pack is exactly where the default
//! is most wrong: a charged-and-rested pack restored at `0.0` comes back sourcing
//! `hysteresis.scale_v` below the one that was saved. The case v14 and v17 could point at
//! as safe is the case this bump cannot. That is argued at the field and at
//! [`SNAPSHOT_VERSION`], and it is why `hysteresis.rs` takes its snapshot **after a rest**
//! rather than under load.
//!
//! # v14 and v15 have no structurally-valid stale blob at all, and that is stated rather
//! # than papered over
//! v12 and v13 could each name a configuration — `bms: None` — whose old bytes were, field
//! for field, what the new build writes, so that the version field was demonstrably the
//! thing doing the refusing. **v14 could not**, and **v15 cannot either.** v14 added
//! `soc_deficit` to every `EcmState` *and* a required `[reversal]` section to
//! `ChemistryParams`; v15 adds a required `fade_per_ah` to that same section, plus two
//! accumulators to every cell's aging state. The chemistry is serialized inside every
//! snapshot whatever cell model the pack runs, and `bincode` writes struct fields in
//! declaration order with no framing (the same fact [`snapshot_bytes`] relies on to find
//! the version tag), so a v14 blob is one `f64` short of what a v15 build reads and there
//! is no arrangement of a v14 pack whose bytes parse here. A genuine v14 blob fails at
//! *deserialization* — the pre-v10 situation — and the version check never sees it.
//!
//! **The v15 argument is inherited on purpose, and the inheritance is checked rather than
//! assumed.** It is the same *mechanism* as v14's (a required field with no
//! `#[serde(default)]` inside a struct every snapshot carries), so the conclusion carries;
//! what does not carry is v14's reason for the field being required. There, a default would
//! have been an unlabeled physical constant. Here a default of `0.0` exists and is even
//! semantically harmless for a restore — a v14 pack accrued no over-discharge damage — and
//! the field is required anyway, because the value it would supply to a *fresh* chemistry
//! file is "over-discharge is free", which is the defect v15 exists to remove. Same
//! conclusion, different argument; see `docs/plans/reversal-damage.md`.
//!
//! **So what the pair below still proves, and what it no longer does.** It still proves
//! the check is wired and decides on the outer tag alone: the bytes are this build's own,
//! retagged, so the two arms differ in nothing else. It does **not** prove that a
//! real v14 snapshot meets the version check rather than the deserializer — at v15 it
//! meets the deserializer, and no fixture can change that. Recorded because the file's
//! earlier sections claim the stronger thing for the bumps that earned it, and a reader
//! renumbering this test for v16 needs to know which of the two claims they are
//! inheriting.
//!
//! # The fixture is an equivalent-circuit pack, deliberately
//! Not an SPM or DFN one. The hazard is a snapshot an **older build could actually have
//! written**, and the plainest such pack is the one whose state the bump is about.
//!
//! # Why one assertion is not enough
//! "A blob tagged 9 is rejected" proves nothing on its own: it is satisfied equally
//! by the version check and by deserialization failing for an unrelated reason, which
//! is exactly the ambiguity the notes above complain about. So the test below uses
//! the **same bytes** with the version field and nothing else different. One tag must
//! be rejected and the other must restore; together they isolate the field.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, Pack, PackConfig, RestoreError, Scatter, Snapshot, ThermalConfig,
    SNAPSHOT_VERSION,
};

/// A minimal equivalent-circuit chemistry. Nothing here is physical — the pack only
/// has to run far enough to produce a snapshot with something in it.
fn chem() -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            // Zero: this file's chemistry pays nothing for over-discharge, so its
            // trajectories are the ones this slice must not move. See
            // `docs/plans/reversal-damage.md`.
            fade_per_ah: 0.0,
        },
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "sv".into(),
            name: "Snapshot-version test cell".into(),
            provenance: "snapshot version test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: 2.5,
            v_max: 3.65,
            v_min: 2.0,
            max_charge_c: 2.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            t_ref_k: None,
            soc: vec![0.0, 1.0],
            volts: vec![3.0, 3.5],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
    }
}

/// Serialized bytes of a snapshot of a pack that has actually run.
///
/// bincode writes struct fields in declaration order with no framing, and
/// `Snapshot`'s first field is `version: u32`, so the tag is the first four bytes
/// little-endian. That is an assumption about the *format*, not about the type, and
/// [`retagged`] checks it by reading the value back rather than trusting it.
fn snapshot_bytes() -> Vec<u8> {
    let config = PackConfig {
        series: 2,
        parallel: 2,
        initial_soc: 0.6,
        initial_temp_k: 298.15,
        seed: 11,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: None,
        cell_model: CellModelConfig::Ecm,
    };
    let mut pack = Pack::new(&config, chem()).expect("the pack builds");
    let env = Env {
        t_ambient: 298.15,
        t_coolant: None,
    };
    for _ in 0..30 {
        pack.step(1.0, Demand::Current(4.0), &env);
    }
    bincode::serialize(&pack.snapshot()).expect("the snapshot serializes")
}

/// Retag a serialized snapshot, and confirm the retag landed where it was aimed.
fn retagged(bytes: &[u8], version: u32) -> Snapshot {
    let mut bytes = bytes.to_vec();
    bytes[0..4].copy_from_slice(&version.to_le_bytes());
    let snapshot: Snapshot =
        bincode::deserialize(&bytes).expect("a retagged snapshot is still structurally valid");
    assert_eq!(
        snapshot.version, version,
        "the version field is not where this test thinks it is, so neither assertion \
         below means what it says — re-derive the offset before trusting either"
    );
    snapshot
}

/// A v20-tagged snapshot is rejected by the version check, and the **same bytes**
/// tagged v21 restore.
///
/// The pair is the test. Alone, the rejection is indistinguishable from
/// deserialization failing; alone, the acceptance says only that the fixture is
/// well-formed. Together they say the version field, and only the version field,
/// decided.
///
/// **At this bump the retag is not a stand-in, for the third time running.** Read the
/// module's v21 section: the fixture chemistry has no `[charge_acceptance]`, so the field
/// v21 adds is one absent `Option` tag that a v20 build never wrote, and a v20 build's
/// snapshot of this pack has exactly these bytes. The sibling
/// [`a_v20_shaped_chemistry_tail_misparses_at_v21`] answers the *other* case — a
/// chemistry whose bytes do change — and its answer is the quiet one, which is why both
/// exist.
#[test]
fn the_version_field_is_what_rejects_a_v20_snapshot() {
    assert_eq!(
        SNAPSHOT_VERSION, 21,
        "this test is written against the v20 -> v21 bump specifically. A later bump \
         needs its own pair rather than this one renumbered: what a stale blob does under \
         the new layout is a fact about that layout change, and the answer has flipped \
         across this file's history — v15 'it does not parse at all', v16 'it parses, \
         wrongly and silently', v17 and v18 back to 'it does not parse at all', and v19 \
         to v21 'for this fixture it parses fine and the version field is all there \
         is'. A renumbered assertion cannot inherit any of them, and a run of three \
         identical answers is not a rule — it is three layout changes that happened not \
         to touch this fixture, which is a fact about the fixture."
    );
    let bytes = snapshot_bytes();

    let stale = retagged(&bytes, 20);
    assert_eq!(
        Pack::restore(&stale),
        Err(RestoreError::VersionMismatch {
            found: 20,
            expected: SNAPSHOT_VERSION,
        }),
        "a v20-tagged snapshot must be refused"
    );

    let current = retagged(&bytes, SNAPSHOT_VERSION);
    let restored = Pack::restore(&current).expect(
        "the identical bytes at the current version must restore — if this fails, the \
         rejection above was deserialization rather than the version check, and the \
         bump is still untested",
    );
    assert_eq!(restored.series(), 2);
    assert_eq!(restored.parallel(), 2);
}

/// A v15 `v_rc` field does not fail to parse at v16 — it parses into the wrong numbers.
///
/// This is why v16's version check is load-bearing in a way v14's and v15's were not, and
/// it is asserted rather than reasoned about because the module doc and the
/// [`SNAPSHOT_VERSION`] note both lean on it. `bincode` writes a sequence as a `u64`
/// length followed by its elements, so a one-pair `v_rc` was `[1u64][x]` — sixteen bytes,
/// which is exactly what `[f64; 2]` reads. Nothing runs off the end and nothing errors;
/// the length is reinterpreted as a subnormal and the real overpotential lands in the
/// second slot.
///
/// **Scope, deliberately narrow.** This is the *field*, not a snapshot. Fabricating a
/// whole v15 blob would mean inserting a length prefix at every cell's `v_rc` offset, and
/// this file has no way to locate those offsets that is not a guess — so the wider claim
/// is not made anywhere. What is proven here is the mechanism: silent reinterpretation is
/// available at v16, which is the property that makes an unguarded restore dangerous.
///
/// The two-pair case is stated for contrast and not asserted: `[2u64][a][b]` is
/// twenty-four bytes against sixteen read, so it desynchronises everything after it and
/// fails loudly. It is the one-pair case — every chemistry this repo ships — that is
/// quiet.
#[test]
fn the_v15_v_rc_bytes_reinterpret_rather_than_fail() {
    // A v15 one-pair `v_rc`, in the bytes a v15 build would have written for it.
    let v15 = bincode::serialize(&vec![0.012_f64]).expect("a Vec<f64> serializes");
    assert_eq!(
        v15.len(),
        16,
        "8-byte length prefix plus one 8-byte element"
    );

    let v16: [f64; 2] = bincode::deserialize(&v15)
        .expect("the same sixteen bytes are a valid `[f64; 2]` — that is the whole hazard");

    assert_eq!(
        v16[0],
        f64::from_bits(1),
        "the length prefix becomes a subnormal overpotential, not an error"
    );
    assert_eq!(
        v16[1], 0.012,
        "and the real overpotential has slid one slot along"
    );
    assert_ne!(
        v16[0], 0.0,
        "if this ever becomes zero the reinterpretation is harmless and this test, the \
         module's v16 section and the SNAPSHOT_VERSION note all overstate the hazard"
    );
}

/// The check reads the snapshot's own tag, and nothing else.
///
/// `Pack` carries a `version` field too, and `Snapshot::version` is documented as
/// mirroring it — but [`Pack::restore`] consults only the outer one. That is the
/// right choice (it is the field an adapter can read before committing to a restore)
/// and it means the inner copy is redundant rather than a second line of defence.
/// Pinned here so nobody later reads the pair as belt-and-braces: retagging the
/// outer field alone is enough to change the verdict, which it would not be if the
/// inner copy were also checked.
#[test]
fn only_the_outer_version_tag_is_consulted() {
    let bytes = snapshot_bytes();
    // Both blobs carry an inner `Pack.version` of 19, because that is what this
    // build wrote. Only the outer tag differs, and only the outer tag decides.
    assert!(Pack::restore(&retagged(&bytes, 18)).is_err());
    assert!(Pack::restore(&retagged(&bytes, SNAPSHOT_VERSION)).is_ok());
}

/// A v17 cell state does not parse at v18 — and that is the *reassuring* direction.
///
/// **v19 did not touch `EcmState`**, so this test's subject is the previous bump and its
/// assertions are unchanged: what it deserializes into is still the current cell state, and
/// a v17-shaped one is still eight bytes short of it. It is kept for the reason the v16
/// shape below is kept — it is the assertion that would notice a future edit *reordering*
/// the struct — and it is deliberately not renumbered, because renaming it to v19 would
/// claim it measures a layout change it has nothing to do with.
///
/// The sibling of [`the_v15_v_rc_bytes_reinterpret_rather_than_fail`], asserting the
/// opposite outcome, because the two bumps are opposite cases and the module's claim that
/// v18 is "the v14/v15 situation" should be checked rather than repeated. v16's stale field
/// stayed byte-aligned and was silently reinterpreted; v18 adds an `f64` to a struct
/// `bincode` writes positionally with no framing, so a v17 `EcmState` is eight bytes short
/// of what v18 reads. Nothing can be misread — only refused.
///
/// **This is the field, not a snapshot**, on the same terms and for the same reason its
/// sibling is: there is no non-guessing way to locate every cell's state inside a blob.
/// What is proven is the mechanism — a v17-shaped cell state cannot be mistaken for a v18
/// one — which is what makes the version check belt to the deserializer's braces here
/// rather than the only line of defence it was at v16.
///
/// The v16 shape is kept alongside, one field further away and still failing. It costs a
/// line and it is the assertion that would notice a future edit *reordering* `EcmState`
/// rather than appending to it, which is the one edit that could quietly restore v16's
/// hazard.
#[test]
fn a_v17_shaped_cell_state_does_not_parse_at_v18() {
    // A v17 `EcmState`, in the fields a v17 build wrote and in declaration order: `soc`,
    // `soc_deficit`, `v_rc`, `depletion`, `temp_k`. No `hysteresis`.
    let v17 = bincode::serialize(&(0.6_f64, 0.0_f64, [0.012_f64, 0.0_f64], 0.35_f64, 298.15_f64))
        .expect("a v17-shaped cell state serializes");
    assert_eq!(v17.len(), 48, "six f64, positional, no framing");

    let parsed: Result<sim_core::EcmState, _> = bincode::deserialize(&v17);
    assert!(
        parsed.is_err(),
        "a v17 cell state must FAIL to parse at v18, not parse into the wrong numbers. If \
         this ever passes, v18 has v16's silent-reinterpretation hazard and the \
         SNAPSHOT_VERSION note — which claims the loud one — is wrong."
    );

    // And the v16 shape, which is shorter still and fails for the same reason. This is the
    // line that would go green if someone ever inserted a field in the middle rather than
    // appending one at the end.
    let v16 = bincode::serialize(&(0.6_f64, 0.0_f64, [0.012_f64, 0.0_f64], 298.15_f64))
        .expect("a v16-shaped cell state serializes");
    assert_eq!(v16.len(), 40, "five f64, positional, no framing");
    let parsed: Result<sim_core::EcmState, _> = bincode::deserialize(&v16);
    assert!(parsed.is_err(), "a v16 cell state must fail at v18 too");

    // The positive control, so the two failures above are missing fields rather than a
    // broken fixture: those six values plus a seventh are exactly a v18 `EcmState`.
    let v18 = bincode::serialize(&(
        0.6_f64,
        0.0_f64,
        [0.012_f64, 0.0_f64],
        0.35_f64,
        -0.75_f64,
        298.15_f64,
    ))
    .expect("a v18-shaped cell state serializes");
    let state: sim_core::EcmState =
        bincode::deserialize(&v18).expect("seven f64 in declaration order are a v18 EcmState");
    assert_eq!(
        state.depletion, 0.35,
        "the v17 field is the fifth, and the new one did not displace it"
    );
    assert_eq!(
        state.hysteresis, -0.75,
        "the new field is the sixth, not the last"
    );
    assert_eq!(state.temp_k, 298.15);
}

/// A v18-shaped `[safety]` section at v19: **loud for the values every shipped file
/// carries, and quiet for values none of them do.**
///
/// The sibling of [`the_version_field_is_what_rejects_a_v19_snapshot`], which answers this
/// question for a chemistry with **no** `[safety]` section — there the bytes do not change
/// at all and the version check is the only refusal available. This is the other case, and
/// the answer is different, so both are measured rather than one being inferred from the
/// other.
///
/// v19 turns two `f64` fields into `Option<f64>`. `bincode` writes an `Option` as a one-byte
/// tag followed by the payload, and writes struct fields positionally with no framing, so a
/// v18 reader's eight raw bytes are read here as *tag, then something else*. Whether that
/// is loud or quiet depends on **the first byte of the stale `f64`**, which is the low byte
/// of its mantissa:
///
/// * `273.15` — the gate in all three shipped lithium files — begins `0x66`, which is not a
///   valid `Option` tag, so the parse fails immediately. Loud.
/// * `273.0` — a rounder number, and the kind a future file might well carry — begins
///   `0x00`, a valid `None` tag. The parse then consumes one byte where eight were written
///   and everything after it slides. Quiet, in the v16 sense.
///
/// **That value-dependence is the finding**, and it is why the `SNAPSHOT_VERSION` note for
/// v19 does not claim the loud direction the way v17's and v18's do. Nothing shipped is at
/// risk — the check refuses every stale blob regardless — but a reader who takes "a stale
/// safety section cannot be misread" from this bump would be taking it from the values that
/// happen to be in the files today.
///
/// **This is the field, not a snapshot**, on the same terms as its two siblings above:
/// there is no non-guessing way to locate the chemistry's offset inside a blob, so the
/// wider claim is not made here or in the version note.
#[test]
fn a_v18_shaped_safety_section_does_not_parse_at_v19() {
    // A v18 `SafetyParams`, in declaration order and all `f64`: `t_onset_k`, `t_vent_k`,
    // `runaway_energy_j`, `runaway_power_w_at_onset`, `runaway_ea_j_per_mol`,
    // `t_plating_min_k`, `plating_c_threshold`, `plating_fade_per_ah`,
    // `plating_short_hazard_per_ah`, `plating_short_ohms`.
    let v18 = |gate: f64| {
        bincode::serialize(&(
            423.15_f64, 453.15_f64, 60.0e3_f64, 8.0_f64, 1.0e5_f64, gate, 0.4_f64, 2.0e-4_f64,
            1.0e-4_f64, 5.0_f64,
        ))
        .expect("a v18-shaped safety section serializes")
    };

    // The shipped gate. Its low mantissa byte is not 0 or 1, so it cannot be an `Option`
    // tag and the parse dies on it.
    let shipped = v18(273.15);
    assert_eq!(shipped.len(), 80, "ten f64, positional, no framing");
    assert_eq!(
        273.15_f64.to_le_bytes()[0],
        0x66,
        "the loudness below is a fact about this byte; if the constant ever changes so \
         does the verdict"
    );
    let parsed: Result<sim_core::chem::SafetyParams, _> = bincode::deserialize(&shipped);
    assert!(
        parsed.is_err(),
        "a v18 safety section carrying the shipped 273.15 gate must fail to parse at v19"
    );

    // And the quiet case, which is the one worth writing down. A round temperature's low
    // byte is zero — a valid `None` tag — so the same layout change is silently
    // reinterpreted rather than refused.
    assert_eq!(273.0_f64.to_le_bytes()[0], 0x00, "a valid `None` tag");
    let round: sim_core::chem::SafetyParams = bincode::deserialize(&v18(273.0)).expect(
        "a round gate value makes the stale section parse — if this ever errors, the \
         value-dependence documented above and in the v19 SNAPSHOT_VERSION note has gone \
         away and both should be corrected rather than quietly left",
    );
    assert!(
        round.t_plating_min_k.is_none(),
        "the stale temperature has been read as an absent gate: the cell now claims it \
         cannot plate"
    );
    assert_ne!(
        round.plating_fade_per_ah, 2.0e-4,
        "and everything after the gate has slid, which is what makes this the v16 hazard \
         rather than a harmless default"
    );

    // The positive control, so the failure above is the layout and not a broken fixture:
    // the same ten values with the gate written as an `Option` are a v19 section.
    let v19 = bincode::serialize(&(
        423.15_f64,
        453.15_f64,
        60.0e3_f64,
        8.0_f64,
        1.0e5_f64,
        Some(273.15_f64),
        Some(0.4_f64),
        2.0e-4_f64,
        1.0e-4_f64,
        5.0_f64,
    ))
    .expect("a v19-shaped safety section serializes");
    let safety: sim_core::chem::SafetyParams =
        bincode::deserialize(&v19).expect("that is exactly a v19 SafetyParams");
    assert_eq!(safety.t_plating_min_k, Some(273.15));
    assert_eq!(safety.plating_c_threshold, Some(0.4));
    assert_eq!(safety.plating_fade_per_ah, 2.0e-4);
}

/// A v19-shaped `[hysteresis]` section at v20: **loud for a chemistry that declares
/// `[aging]`, quiet for one that does not** — and the byte that decides is not inside the
/// hysteresis section at all.
///
/// The sibling of [`the_version_field_is_what_rejects_a_v19_snapshot`], which answers the
/// case where the chemistry has no `[hysteresis]` and the bytes therefore do not change.
/// This is the other case, and as at v19 the answer differs, so both are measured.
///
/// v20 appends `width_over_soc: Option<HysteresisWidth>` to a struct that was two bare
/// `f64`. `bincode` writes struct fields positionally with no framing, so the tag the v20
/// reader looks for immediately after `gamma` is whatever the v19 writer put there — and
/// that is [`sim_core::ChemistryParams::aging`]'s own presence tag, because `aging` is the
/// very next field of the enclosing struct. So:
///
/// * **`[aging]` present** writes `1`. The reader takes it as "there is a width table" and
///   tries to read a `Vec<f64>` length out of the first eight bytes of the aging section,
///   which is a `f64` and therefore an absurd count. Loud.
/// * **`[aging]` absent** writes `0`. The reader takes it as "no width table" and moves on,
///   having consumed a byte that belonged to the field after it — so every remaining field
///   of the chemistry has slid by one. Quiet, in the v16 sense.
///
/// Of the two shipped files that declare `[hysteresis]`, `na_ion_18650_generic` is the first
/// case and `nimh_subc_3ah_generic` the second, which is why neither direction is a
/// hypothetical.
///
/// **This is the field, not a snapshot**, on the same terms as its siblings above: there is
/// no non-guessing way to locate the chemistry's offset inside a blob, so no wider claim is
/// made here or in the version note.
#[test]
fn a_v19_shaped_hysteresis_section_does_not_parse_at_v20() {
    // The tail of a v19 chemistry from `hysteresis` onward, in declaration order:
    // `scale_v`, `gamma`, then `aging`, then `safety`. Everything after `gamma` belongs to
    // the enclosing struct and is exactly what makes this test interesting.
    let v19 = |aging: Option<sim_core::AgingParams>| {
        bincode::serialize(&(
            0.010_f64,
            25.0_f64,
            aging,
            None::<sim_core::chem::SafetyParams>,
        ))
        .expect("a v19-shaped chemistry tail serializes")
    };
    // What the reader expects at v20: the same tail with one more `Option` at the front of
    // it. Note there is no `safety` here — the reader has one field fewer to find because
    // the extra tag ate one.
    type V20Tail = (sim_core::HysteresisParams, Option<sim_core::AgingParams>);

    // --- the loud case: a chemistry that ages.
    let aging = sim_core::AgingParams {
        cal_pre_exp: 1.0e4,
        cal_ea_j_per_mol: 5.0e4,
        cal_soc_stress: vec![1.0, 1.0, 1.4],
        cyc_fade_per_ah: 2.0e-5,
        cyc_dod_stress_exp: 1.1,
        r_growth_per_capacity_loss: 1.5,
    };
    let parsed: Result<V20Tail, _> = bincode::deserialize(&v19(Some(aging)));
    assert!(
        parsed.is_err(),
        "a v19 hysteresis section followed by an [aging] section must fail at v20: the \
         aging tag is read as 'there is a width table' and the table's length prefix comes \
         out of a f64"
    );

    // --- the quiet case: a chemistry with no [aging], which is the NiMH file. The parse
    // succeeds, reports no width table, and has silently eaten the tag belonging to the
    // field after it.
    let quiet: V20Tail = bincode::deserialize(&v19(None)).expect(
        "with no [aging] the stale tail parses — if this ever errors, the value-dependence \
         documented above and in the v20 SNAPSHOT_VERSION note has gone away and both \
         should be corrected rather than quietly left",
    );
    assert!(
        quiet.0.width_over_soc.is_none(),
        "the aging tag has been read as an absent width table"
    );
    assert_eq!(quiet.0.scale_v, 0.010, "the two old fields are unmoved");
    assert_eq!(quiet.0.gamma, 25.0);
    assert!(
        quiet.1.is_none(),
        "and `aging` has been filled from `safety`'s tag, which is the slide that makes \
         this the v16 hazard rather than a harmless default"
    );

    // The positive control, so the failure above is a layout change rather than a broken
    // fixture: the same two numbers plus an explicit absent table are a v20 section.
    let v20 = bincode::serialize(&(0.010_f64, 25.0_f64, None::<sim_core::HysteresisWidth>))
        .expect("a v20-shaped hysteresis section serializes");
    let params: sim_core::HysteresisParams =
        bincode::deserialize(&v20).expect("that is exactly a v20 HysteresisParams");
    assert_eq!(params.scale_v, 0.010);
    assert!(params.width_over_soc.is_none());
}

/// A v20-shaped chemistry tail at v21: **quiet, and value-independent** — the first bump
/// in this file whose stale-blob hazard does not depend on a byte's value at all.
///
/// The sibling of [`the_version_field_is_what_rejects_a_v20_snapshot`], which answers the
/// case where the chemistry has no `[charge_acceptance]` and the bytes therefore do not
/// change. This is the other case: a v20 writer never wrote the new `Option`, so the tag
/// the v21 reader looks for immediately after `hysteresis` is
/// [`sim_core::ChemistryParams::aging`]'s own presence byte, exactly the position the v20
/// test above analyses. What differs is the payload. v20's new field was a table, so a
/// `1` sent the reader looking for a `Vec` length inside a `f64` and failed loudly; v21's
/// is a single bare `f64`, and **any eight bytes are a valid `f64`**. So:
///
/// * **`[aging]` present** writes `1`. The reader takes `cal_pre_exp` as the onset and
///   reads on, one field displaced. The section parses, into a number.
/// * **`[aging]` absent** writes `0`. The reader takes it as "no section", and every field
///   after it slides by one — the v20 quiet case again.
///
/// Neither direction errors inside the section itself. This is v16's hazard in its purest
/// form and is why the version check, not the parse, is what stands between a v20 blob and
/// a cell that claims a taper onset of ten thousand.
///
/// **This is the field, not a snapshot**, on the same terms as its siblings above: there is
/// no non-guessing way to locate the chemistry's offset inside a blob, so no wider claim is
/// made here or in the version note.
#[test]
fn a_v20_shaped_chemistry_tail_misparses_at_v21() {
    // The tail of a v20 chemistry from `hysteresis` onward, in declaration order:
    // `hysteresis`, `aging`, `safety`. No `charge_acceptance` — a v20 writer had none.
    let v20 = |aging: Option<sim_core::AgingParams>| {
        bincode::serialize(&(
            None::<sim_core::HysteresisParams>,
            aging,
            None::<sim_core::chem::SafetyParams>,
        ))
        .expect("a v20-shaped chemistry tail serializes")
    };
    // What the reader expects at v21: one more `Option` between the two. There is no
    // `safety` here — the reader has one field fewer to find because the extra tag ate one.
    type V21Tail = (
        Option<sim_core::HysteresisParams>,
        Option<sim_core::ChargeAcceptanceParams>,
        Option<sim_core::AgingParams>,
    );

    let aging = sim_core::AgingParams {
        cal_pre_exp: 1.0e4,
        cal_ea_j_per_mol: 5.0e4,
        cal_soc_stress: vec![1.0, 1.0, 1.4],
        cyc_fade_per_ah: 2.0e-5,
        cyc_dod_stress_exp: 1.1,
        r_growth_per_capacity_loss: 1.5,
    };
    // --- a chemistry that ages: the tag is `1` and the payload is whatever came next.
    let parsed: Result<V21Tail, _> = bincode::deserialize(&v20(Some(aging)));
    match parsed {
        Ok((hyst, Some(ca), _)) => {
            assert!(hyst.is_none());
            assert_eq!(
                ca.soc_onset, 1.0e4,
                "the stale aging section's first f64 has been read as the taper onset — \
                 the parse is quiet and the number is nonsense, which is the hazard"
            );
        }
        Ok((_, None, _)) => panic!(
            "an [aging] section's presence tag was read as an absent taper; the v21 note \
             says the opposite and one of them must be corrected"
        ),
        Err(e) => panic!(
            "the v21 note claims the section itself parses quietly out of an aging \
             section and it did not: {e}. Correct the note rather than this test"
        ),
    }

    // --- a chemistry with no [aging], which is the NiMH file: the section reads as absent
    // and the slide begins one field later.
    let quiet: V21Tail = bincode::deserialize(&v20(None)).expect(
        "with no [aging] the stale tail parses — if this ever errors, the value-independence \
         documented above and in the v21 SNAPSHOT_VERSION note has gone away and both \
         should be corrected rather than quietly left",
    );
    assert!(
        quiet.1.is_none(),
        "the aging tag has been read as an absent taper"
    );
    assert!(
        quiet.2.is_none(),
        "and `aging` has been filled from `safety`'s tag, which is the slide that makes \
         this the v16 hazard rather than a harmless default"
    );

    // The positive control, so the readings above are a layout change rather than a broken
    // fixture: an explicit section round-trips to itself.
    let v21 = bincode::serialize(&Some(sim_core::ChargeAcceptanceParams { soc_onset: 0.9 }))
        .expect("a v21-shaped section serializes");
    let params: Option<sim_core::ChargeAcceptanceParams> =
        bincode::deserialize(&v21).expect("that is exactly a v21 section");
    assert_eq!(
        params,
        Some(sim_core::ChargeAcceptanceParams { soc_onset: 0.9 })
    );
}
