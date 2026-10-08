//! The guided path's chapters (`const CHAPTERS` in `web/app.js`) against its lessons.
//!
//! A chapter is a heading over lessons that already run in order, named by its first
//! lesson's `id`. Nothing in the engine reads it, so nothing but this file notices when a
//! lesson is renamed, moved or inserted under it. What is pinned:
//!
//! * every chapter's `first` names a lesson in `const LESSONS`;
//! * the first chapter opens the path, and the chapters run in lesson order, none empty;
//! * every chapter after the first opens on a step that **builds its own pack** when the
//!   reader arrives by Next — a different scenario from the step before it, or
//!   `reload: true`. `applyStep` reloads on exactly those, so arriving by Next and
//!   arriving by the chapter menu (which always reloads) start the same run from t = 0;
//! * titles carry no digits and no number words, since nothing derives them.
//!
//! See `docs/plans/path-chapters.md`.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root resolves from CARGO_MANIFEST_DIR")
}

fn app_js() -> String {
    std::fs::read_to_string(repo_root().join("web").join("app.js")).expect("web/app.js reads")
}

/// The text between `const NAME = [` and the `];` that closes it at the start of a line.
fn array_body<'a>(src: &'a str, name: &str) -> &'a str {
    let open = format!("const {name} = [");
    let start = src
        .find(&open)
        .unwrap_or_else(|| panic!("web/app.js still declares `{open}`"));
    let close = src[start..].find("\n];").unwrap_or_else(|| {
        panic!("`const {name}` is still closed by a `];` at the start of a line")
    });
    &src[start..start + close]
}

/// The quoted value after the first `key: "` in `text`.
fn quoted_after(text: &str, key: &str) -> Option<String> {
    let marker = format!("{key}: \"");
    let start = text.find(&marker)? + marker.len();
    let end = text[start..].find('"')?;
    Some(text[start..start + end].to_string())
}

#[derive(Debug)]
struct Lesson {
    id: String,
    scenario: String,
    reload: bool,
}

/// One record per lesson, split on the same `\n    id: "` marker the claims test uses.
fn lessons(src: &str) -> Vec<Lesson> {
    let body = array_body(src, "LESSONS");
    let marker = "\n    id: \"";
    let starts: Vec<usize> = body.match_indices(marker).map(|(i, _)| i).collect();
    assert!(!starts.is_empty(), "no `id:` fields inside const LESSONS");
    starts
        .iter()
        .enumerate()
        .map(|(n, &s)| {
            let block = &body[s..starts.get(n + 1).copied().unwrap_or(body.len())];
            let id = quoted_after(block, "\n    id").expect("lesson has an id");
            Lesson {
                scenario: quoted_after(block, "\n    scenario")
                    .unwrap_or_else(|| panic!("lesson `{id}` has no scenario")),
                reload: block.contains("\n    reload: true"),
                id,
            }
        })
        .collect()
}

#[derive(Debug)]
struct Chapter {
    first: String,
    title: String,
}

/// One record per `{ first: "…", title: "…" }` line of `const CHAPTERS`.
fn chapters(src: &str) -> Vec<Chapter> {
    let body = array_body(src, "CHAPTERS");
    let out: Vec<Chapter> = body
        .lines()
        .filter(|l| l.trim_start().starts_with("{ first: "))
        .map(|l| Chapter {
            first: quoted_after(l, "first").expect("chapter line has `first`"),
            title: quoted_after(l, "title").expect("chapter line has `title`"),
        })
        .collect();
    assert!(
        !out.is_empty(),
        "no `{{ first: …, title: … }}` lines inside const CHAPTERS — the formatting changed \
         and this scraper needs updating"
    );
    out
}

#[test]
fn every_chapter_names_a_lesson_and_they_run_in_order() {
    let src = app_js();
    let lessons = lessons(&src);
    let chapters = chapters(&src);
    let starts: Vec<usize> = chapters
        .iter()
        .map(|c| {
            lessons
                .iter()
                .position(|l| l.id == c.first)
                .unwrap_or_else(|| {
                    panic!(
                        "chapter \"{}\" names `{}`, which is no lesson id",
                        c.title, c.first
                    )
                })
        })
        .collect();
    assert_eq!(
        starts[0], 0,
        "the first chapter must open the path, at lesson `{}`",
        lessons[0].id
    );
    for w in starts.windows(2) {
        assert!(
            w[0] < w[1],
            "chapters out of lesson order or empty: one starts at step {} and the next at \
             step {}",
            w[0] + 1,
            w[1] + 1
        );
    }
}

#[test]
fn every_chapter_opens_on_a_step_that_builds_its_own_pack() {
    let src = app_js();
    let lessons = lessons(&src);
    for c in chapters(&src).iter().skip(1) {
        let i = lessons
            .iter()
            .position(|l| l.id == c.first)
            .expect("checked by the ordering test");
        let (prev, here) = (&lessons[i - 1], &lessons[i]);
        assert!(
            here.reload || here.scenario != prev.scenario,
            "chapter \"{}\" opens on step {} (`{}`), which shares `{}` with step {} (`{}`) \
             and has no `reload: true` — so Next into it inherits the previous step's pack \
             while the chapter menu starts it from t = 0. Move the boundary to a step with \
             its own scenario, or give that step `reload: true`.",
            c.title,
            i + 1,
            here.id,
            here.scenario,
            i,
            prev.id
        );
    }
}

#[test]
fn chapter_titles_state_no_count() {
    // Nothing derives a count written into a title, and stale self-counts in words are a
    // recorded failure of this path. The page computes "chapter N of M" itself.
    const NUMBER_WORDS: &[&str] = &[
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
        "twelve", "first", "second", "third", "last", "only", "dozen",
    ];
    for c in chapters(&app_js()) {
        assert!(
            !c.title.chars().any(|ch| ch.is_ascii_digit()),
            "chapter title \"{}\" contains a digit",
            c.title
        );
        let lowered = c.title.to_lowercase();
        for w in lowered.split(|ch: char| !ch.is_ascii_alphabetic()) {
            assert!(
                !NUMBER_WORDS.contains(&w),
                "chapter title \"{}\" contains the count word `{w}`",
                c.title
            );
        }
    }
}
