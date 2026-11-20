//! Round-trips every `.org` file under `ORGIZE_CORPUS`, such as the pinned
//! org-mode and Worg checkouts from `scripts/fetch-corpus.sh`.

use orgize::Org;
use std::fs;
use std::path::{Path, PathBuf};

fn org_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            org_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "org") {
            files.push(path);
        }
    }
}

#[test]
#[ignore = "needs ORGIZE_CORPUS; see development.md"]
fn corpus_round_trips() {
    let root = std::env::var_os("ORGIZE_CORPUS").expect("ORGIZE_CORPUS is not set");
    let mut files = vec![];
    org_files(Path::new(&root), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .org files under {root:?}");

    let mut failed = vec![];
    for path in &files {
        let input = fs::read_to_string(path).unwrap();
        if Org::parse(&input).to_org() != input {
            failed.push(path.display().to_string());
        }
    }
    assert!(
        failed.is_empty(),
        "{} of {} files don't round-trip:\n{}",
        failed.len(),
        files.len(),
        failed.join("\n")
    );
}
