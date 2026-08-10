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
//! This file used to pin v9 -> v10, then v10 -> v11, then v11 -> v12, each time carrying
//! an assertion that a later bump needs its own pair. This is the v12 -> v13 pair, and it
//! was re-argued rather than renamed — because from v12 on the argument is **not** the
//! one above.
//!
//! # v13's structural validity is conditional, and the fixture is what makes it hold
//! v13 added one `#[serde(default)]` field to `BalancingConfig` and a `Vec<bool>` to
//! `Bms`. Under a self-describing format those defaults would carry a stale blob through
//! exactly as the `[dfn]` section did. **`bincode` is not self-describing**: it writes
//! struct fields in declaration order with no framing (the same fact [`snapshot_bytes`]
//! relies on to find the version tag), so `serde(default)` buys nothing and a v12 blob
//! carrying `bms: Some(..)` is short by a scalar and a sequence. That blob fails at
//! *deserialization* — the pre-v10 situation — and the version check never sees it.
//!
//! This is the second bump in a row where that holds, and the field set is different
//! each time, so it is derived here rather than inherited: v12's shortfall was six
//! values on `ProtectionConfig` and `Bms`; v13's is one `f64` inside the *optional*
//! `BalancingConfig` plus one sequence on `Bms`. Same conclusion, different arithmetic.
//!
//! The fixture below has **`bms: None`**, where none of the new fields is emitted at all,
//! so its v12 bytes are byte-for-byte what v13 writes and the version field really is the
//! only thing that can decide. That is a narrower claim than v10's and v11's, and it is
//! the whole claim: this file proves the check works for the packs whose bytes did not
//! change, not for every v12 pack.
//!
//! # The fixture is an equivalent-circuit pack, deliberately
//! Not an SPM or DFN one. The hazard is a snapshot an **older build could actually have
//! written**, and no v11 build could write a pack this one cannot — but an all-ECM,
//! BMS-less pack's v11 bytes are, field for field, what v12 emits, so retagging them to
//! 11 produces a genuine article rather than a v12 blob wearing a v11 label.
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

/// A v12-tagged snapshot is rejected by the version check, and the **same bytes**
/// tagged v13 restore.
///
/// The pair is the test. Alone, the rejection is indistinguishable from
/// deserialization failing; alone, the acceptance says only that the fixture is
/// well-formed. Together they say the version field, and only the version field,
/// decided — which the bumps before v10 could not claim.
#[test]
fn the_version_field_is_what_rejects_a_v12_snapshot() {
    assert_eq!(
        SNAPSHOT_VERSION, 13,
        "this test is written against the v12 -> v13 bump specifically. A later bump \
         needs its own pair rather than this one renumbered: whether a v12 blob stays \
         structurally valid under v13 is a fact about v13's layout change — and at v13 \
         it is true only for the BMS-less fixture below, which is itself something this \
         assertion cannot inherit."
    );
    let bytes = snapshot_bytes();

    let stale = retagged(&bytes, 12);
    assert_eq!(
        Pack::restore(&stale),
        Err(RestoreError::VersionMismatch {
            found: 12,
            expected: SNAPSHOT_VERSION,
        }),
        "a v12-tagged snapshot must be refused"
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
    // Both blobs carry an inner `Pack.version` of 13, because that is what this
    // build wrote. Only the outer tag differs, and only the outer tag decides.
    assert!(Pack::restore(&retagged(&bytes, 12)).is_err());
    assert!(Pack::restore(&retagged(&bytes, SNAPSHOT_VERSION)).is_ok());
}
