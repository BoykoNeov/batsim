//! The demo scene's bundled data must stay identical to the repo's canonical files.
//!
//! # Why there are copies at all
//! `godot/demo.tscn` reads its scenario and chemistry from `res://assets/`. It cannot
//! read them from `scenarios/` and `chemistries/`, because `res://` does not escape the
//! project directory — and it *should* not, because that is exactly what a shipped game
//! looks like: a `.pck` contains what was bundled into it and nothing else. A demo that
//! reached outside `res://` would demonstrate a pattern that breaks on export, which is
//! the failure this crate's text-not-paths API exists to avoid.
//!
//! # Why that is safe
//! Duplicated data is only dangerous when it can drift silently. This test makes drift
//! loud: it runs in the ordinary `cargo test --workspace`, needs no Godot, and compares
//! bytes. Editing `chemistries/lfp_26650_generic.toml` without re-copying fails here with
//! a message naming the command that fixes it.
//!
//! `CLAUDE.md`'s provenance rule is the sharper reason: a chemistry file's numbers each
//! carry a provenance note, and a stale copy would be an unlabelled second set of physical
//! constants pretending to be the first.

use std::path::{Path, PathBuf};

/// Repo root, from this crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/sim-godot has two ancestors")
        .to_path_buf()
}

/// `(canonical path, bundled path)` for every file the demo scene bundles.
const BUNDLED: [(&str, &str); 2] = [
    (
        "scenarios/cc_discharge_lfp.toml",
        "godot/assets/cc_discharge_lfp.toml",
    ),
    (
        "chemistries/lfp_26650_generic.toml",
        "godot/assets/lfp_26650_generic.toml",
    ),
];

#[test]
fn the_demo_scenes_bundled_data_matches_the_canonical_files() {
    let root = repo_root();
    for (canonical, bundled) in BUNDLED {
        let source = std::fs::read(root.join(canonical))
            .unwrap_or_else(|e| panic!("cannot read {canonical}: {e}"));
        let copy = std::fs::read(root.join(bundled)).unwrap_or_else(|e| {
            panic!("cannot read {bundled}: {e}\ncreate it with: cp {canonical} {bundled}")
        });
        assert_eq!(
            source, copy,
            "{bundled} has drifted from {canonical}.\n\
             The demo scene would run different physics from the rest of the repo.\n\
             Fix with: cp {canonical} {bundled}"
        );
    }
}

/// The demo script must actually reference what the list above claims it bundles. Without
/// this, renaming an asset in `demo.gd` would leave the test above guarding a file nobody
/// reads — passing while the demo is broken.
#[test]
fn the_demo_script_reads_the_files_this_test_guards() {
    let root = repo_root();
    let script = std::fs::read_to_string(root.join("godot/demo.gd")).expect("demo.gd");
    for (_, bundled) in BUNDLED {
        let res_path = bundled.replace("godot/", "res://");
        assert!(
            script.contains(&res_path),
            "demo.gd does not mention {res_path}; either it stopped using that asset \
             (drop it from BUNDLED) or it was renamed (this test is now guarding nothing)"
        );
    }
}
