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
//! v14 -> v15, v15 -> v16 and v16 -> v17, each time carrying an assertion that a later
//! bump needs its own pair. This is the v17 -> v18 pair, and it was re-argued rather than
//! renamed.
//!
//! **The v16 -> v17 pair could not be kept alongside this one, and the reason is not just
//! the assertion that says so.** [`retagged`] fabricates a stale blob by writing *this
//! build's* bytes under a fake tag, and at v18 this build no longer produces v17-shaped
//! bytes — every `EcmState` carries a `hysteresis` here and carried none there. There is
//! nothing for a v16 -> v17 pair to be built out of any more, so this is a replacement
//! rather than an addition. The previous pair predicted its own retirement in these words
//! — *"the retirement is structural, and it will happen again to this one"* — and this is
//! that happening.
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

/// A v17-tagged snapshot is rejected by the version check, and the **same bytes**
/// tagged v18 restore.
///
/// The pair is the test. Alone, the rejection is indistinguishable from
/// deserialization failing; alone, the acceptance says only that the fixture is
/// well-formed. Together they say the version field, and only the version field,
/// decided.
///
/// Read the module's v18 section before extending this: the bytes here are this build's,
/// wearing a v17 label. What a snapshot a v17 build actually wrote does at v18 is a
/// separate question, and it does not parse at all, which
/// [`a_v17_shaped_cell_state_does_not_parse_at_v18`] pins directly.
#[test]
fn the_version_field_is_what_rejects_a_v17_snapshot() {
    assert_eq!(
        SNAPSHOT_VERSION, 18,
        "this test is written against the v17 -> v18 bump specifically. A later bump \
         needs its own pair rather than this one renumbered: what a stale blob does under \
         the new layout is a fact about that layout change, and the answer has flipped \
         across this file's history — v15 'it does not parse at all', v16 'it parses, \
         wrongly and silently', v17 and v18 back to 'it does not parse at all'. A \
         renumbered assertion cannot inherit any of them, and a run of two identical \
         answers is not a rule."
    );
    let bytes = snapshot_bytes();

    let stale = retagged(&bytes, 17);
    assert_eq!(
        Pack::restore(&stale),
        Err(RestoreError::VersionMismatch {
            found: 17,
            expected: SNAPSHOT_VERSION,
        }),
        "a v17-tagged snapshot must be refused"
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
    // Both blobs carry an inner `Pack.version` of 18, because that is what this
    // build wrote. Only the outer tag differs, and only the outer tag decides.
    assert!(Pack::restore(&retagged(&bytes, 17)).is_err());
    assert!(Pack::restore(&retagged(&bytes, SNAPSHOT_VERSION)).is_ok());
}

/// A v17 cell state does not parse at v18 — and that is the *reassuring* direction.
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
