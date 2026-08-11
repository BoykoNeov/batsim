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
//! This file used to pin v9 -> v10, then v10 -> v11, v11 -> v12, v12 -> v13 and v13 -> v14,
//! each time carrying an assertion that a later bump needs its own pair. This is the
//! v14 -> v15 pair, and it was re-argued rather than renamed.
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

/// A v14-tagged snapshot is rejected by the version check, and the **same bytes**
/// tagged v15 restore.
///
/// The pair is the test. Alone, the rejection is indistinguishable from
/// deserialization failing; alone, the acceptance says only that the fixture is
/// well-formed. Together they say the version field, and only the version field,
/// decided.
///
/// Read the module's v15 section before extending this: the bytes here are this build's,
/// wearing a v14 label. A snapshot a v14 build actually wrote does not reach the version
/// check at all.
#[test]
fn the_version_field_is_what_rejects_a_v14_snapshot() {
    assert_eq!(
        SNAPSHOT_VERSION, 15,
        "this test is written against the v14 -> v15 bump specifically. A later bump \
         needs its own pair rather than this one renumbered: what a stale blob does under \
         the new layout is a fact about that layout change, and at v15 the answer is \
         'it does not parse at all', which is not something a renumbered assertion can \
         inherit."
    );
    let bytes = snapshot_bytes();

    let stale = retagged(&bytes, 14);
    assert_eq!(
        Pack::restore(&stale),
        Err(RestoreError::VersionMismatch {
            found: 14,
            expected: SNAPSHOT_VERSION,
        }),
        "a v14-tagged snapshot must be refused"
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
    // Both blobs carry an inner `Pack.version` of 15, because that is what this
    // build wrote. Only the outer tag differs, and only the outer tag decides.
    assert!(Pack::restore(&retagged(&bytes, 14)).is_err());
    assert!(Pack::restore(&retagged(&bytes, SNAPSHOT_VERSION)).is_ok());
}
