//! Golden tests over `conformance/`; `UPDATE_EXPECT=1` rewrites the expectations.

use std::path::{Path, PathBuf};

fn cases() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance");
    let mut out: Vec<PathBuf> = std::fs::read_dir(root)
        .expect("the conformance directory exists")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

fn expect(path: &Path, actual: &str) {
    let updating = std::env::var("UPDATE_EXPECT").is_ok();
    match std::fs::read_to_string(path) {
        Ok(want) if want.trim_end() == actual.trim_end() => {}
        Ok(want) if updating => {
            std::fs::write(path, format!("{actual}\n")).unwrap();
            eprintln!("updated {}", path.display());
            let _ = want;
        }
        Ok(want) => panic!(
            "{} differs\n--- expected ---\n{want}\n--- actual ---\n{actual}",
            path.display()
        ),
        Err(_) if updating => {
            std::fs::write(path, format!("{actual}\n")).unwrap();
            eprintln!("wrote {}", path.display());
        }
        Err(e) => panic!(
            "{} is missing ({e}); run with UPDATE_EXPECT=1",
            path.display()
        ),
    }
}

#[test]
fn corpus_matches_the_golden_files() {
    for case in cases() {
        let name = case.file_name().unwrap().to_string_lossy().to_string();
        let yaml = std::fs::read_to_string(case.join("input.yaml"))
            .unwrap_or_else(|e| panic!("{name}: input.yaml unreadable: {e}"));

        let (tree, diags) = vedavid_dashboard_dsl::compile_with_diagnostics(&yaml);
        let rejected = name.starts_with("reject-");
        assert_eq!(
            tree.is_none(),
            rejected,
            "{name}: expected rejection={rejected}, got tree={}",
            tree.is_some()
        );

        if let Some(tree) = &tree {
            expect(
                &case.join("expected.json"),
                &vedavid_dashboard_dsl::to_json(tree),
            );
        }

        let diagnostics_file = case.join("expected-diagnostics.json");
        if !diags.is_empty() {
            let json = serde_json::to_string_pretty(&diags).unwrap();
            expect(&diagnostics_file, &json);
        } else {
            assert!(
                !diagnostics_file.exists(),
                "{name}: expected diagnostics but the compiler produced none"
            );
        }
    }
}

/// The hash is what tells a consumer whether a dashboard changed, so
/// recompiling the same bytes must not move it.
#[test]
fn compilation_is_deterministic() {
    for case in cases() {
        let yaml = std::fs::read_to_string(case.join("input.yaml")).unwrap();
        let first = vedavid_dashboard_dsl::compile(&yaml);
        let second = vedavid_dashboard_dsl::compile(&yaml);
        match (first, second) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a.hash, b.hash);
                assert_eq!(
                    vedavid_dashboard_dsl::to_json(&a),
                    vedavid_dashboard_dsl::to_json(&b)
                );
            }
            (Err(a), Err(b)) => assert_eq!(a, b),
            _ => panic!("{}: compiled inconsistently", case.display()),
        }
    }
}

/// A patch release that changes no output must not invalidate a cached dashboard.
#[test]
fn the_hash_ignores_the_compiler_version() {
    let yaml = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance/minimal-stat/input.yaml"),
    )
    .unwrap();
    let tree = vedavid_dashboard_dsl::compile(&yaml).unwrap();
    let canonical = vedavid_dashboard_dsl::canonical_json(&tree);
    assert!(!canonical.contains("compiler"));
    assert!(!canonical.contains(&tree.hash));
}
