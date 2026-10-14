use outpost_core_coverage_classifier::{classify_source, classify_tree};
use serde_json::Value;

fn lines(file: &Value, name: &str) -> Vec<u64> {
    file[name]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_u64().unwrap())
        .collect()
}

// Production structs can gain observed test state. The initializer and its
// comma must share test ownership while the surrounding construction counts.
#[test]
fn test_field_and_initializer_own_their_separators() {
    let source = "struct State {\n    live: usize,\n    #[cfg(any(test, feature = \"test-helpers\"))]\n    calls: usize,\n}\nfn new() -> State {\n    State {\n        live: 1,\n        #[cfg(any(test, feature = \"test-helpers\"))]\n        calls: 0,\n    }\n}\n";
    let file = classify_source("src/lib.rs", source).unwrap();
    assert_eq!(lines(&file, "nonproduction_lines"), vec![3, 4, 9, 10]);
    assert!(!lines(&file, "production_token_lines").contains(&10));
    assert!(lines(&file, "production_token_lines").contains(&8));
    assert!(lines(&file, "production_token_lines").contains(&11));
}

// Inline test modules contain real Rust declarations and compiler metadata.
// Their bodies, comment-only lines, and braces must inherit module ownership.
#[test]
fn inline_modules_inherit_ownership_including_comments_and_blank_lines() {
    let source = "fn production() -> usize { 7 }\n#[cfg(test)]\nmod tests {\n    // Test documentation.\n\n    #[test]\n    fn observes() { assert_eq!(super::production(), 7); }\n}\n";
    let file = classify_source("src/lib.rs", source).unwrap();
    assert_eq!(lines(&file, "production_token_lines"), vec![1]);
    assert_eq!(
        lines(&file, "nonproduction_lines"),
        (2..=8).collect::<Vec<_>>()
    );
    assert!(!lines(&file, "nonproduction_token_lines").contains(&4));
}

// Associated accessors and observers use the same cfg forms as free functions.
// Excluding an accessor must preserve its adjacent production method.
#[test]
fn associated_methods_and_nested_attributes_preserve_adjacent_production() {
    let source = "impl State {\n    fn live(&self) -> usize { self.live }\n    #[allow(dead_code)]\n    #[cfg(\n        any(feature = \"test-helpers\", test)\n    )]\n    fn calls(&self) -> usize { self.calls }\n}\n";
    let file = classify_source("src/state.rs", source).unwrap();
    assert_eq!(lines(&file, "nonproduction_lines"), vec![3, 4, 5, 6, 7]);
    assert_eq!(lines(&file, "production_token_lines"), vec![1, 2, 8]);
}

// A cfg observation block can appear inside a production method. Its trailing
// statement separator must not falsely become a production token.
#[test]
fn statement_and_local_ownership_include_semicolons() {
    let source = "fn work() {\n    #[cfg(feature = \"test-helpers\")]\n    { record(); };\n    #[cfg(test)]\n    let observed = 1;\n    perform();\n}\n";
    let file = classify_source("src/lib.rs", source).unwrap();
    assert_eq!(lines(&file, "nonproduction_lines"), vec![2, 3, 4, 5]);
    assert_eq!(lines(&file, "production_token_lines"), vec![1, 6, 7]);
}

// A single LLVM line hit cannot distinguish production from test-only execution
// when both exist on one source line. Preserve both owners for gate rejection.
#[test]
fn mixed_lines_are_visible_in_both_token_sets() {
    let file = classify_source(
        "src/lib.rs",
        "fn work() { #[cfg(test)] observe(); perform(); }\n",
    )
    .unwrap();
    assert_eq!(lines(&file, "production_token_lines"), vec![1]);
    assert_eq!(lines(&file, "nonproduction_token_lines"), vec![1]);
    assert!(lines(&file, "nonproduction_lines").is_empty());
}

// Characters before the cfg construct can be non-ASCII. Span columns must use
// characters consistently rather than accidentally mixing byte offsets.
#[test]
fn non_ascii_columns_and_multiline_literals_keep_exact_ownership() {
    let source = "fn work() { let café = \"é\"; #[cfg(test)] observe(); }\n#[cfg(test)]\nfn fixture() { let text = \"α\nβ\"; }\n";
    let file = classify_source("src/lib.rs", source).unwrap();
    let first = &file["nonproduction"][0];
    let expected_column = source
        .lines()
        .next()
        .unwrap()
        .split("#[cfg")
        .next()
        .unwrap()
        .chars()
        .count();
    assert_eq!(first["start_line"], 1);
    assert_eq!(first["start_column"], expected_column as u64);
    assert_eq!(lines(&file, "nonproduction_lines"), vec![2, 3, 4]);
    assert!(lines(&file, "nonproduction_token_lines").contains(&4));
}

// Rust removes #[test] functions from normal compilation even when no enclosing
// cfg(test) module exists. Attribute semantics must establish that ownership.
#[test]
fn standalone_test_function_is_nonproduction() {
    let file = classify_source(
        "src/lib.rs",
        "fn work() {}\n#[test]\nfn checks_work() { work(); }\n",
    )
    .unwrap();
    assert_eq!(lines(&file, "nonproduction_lines"), vec![2, 3]);
    assert_eq!(lines(&file, "production_token_lines"), vec![1]);
}

// Platform/profile cfg conditions select production configurations, rather than
// granting exclusions. LLVM's chosen-target map decides executable membership.
#[test]
fn production_cfg_and_literal_inclusion_are_not_excluded() {
    let source = "#[cfg(unix)]\nfn live() -> &'static str { include_str!(\"text.txt\") }\n#[cfg(not(debug_assertions))]\nfn release() { if cfg!(unix) { perform(); } }\n";
    let file = classify_source("src/lib.rs", source).unwrap();
    assert!(lines(&file, "nonproduction_lines").is_empty());
    assert_eq!(lines(&file, "production_token_lines"), vec![1, 2, 3, 4]);
}

// New syntax must require review when its test ownership is not understood;
// silently accepting it could exclude production or retain test execution.
#[test]
fn unsupported_test_predicates_and_transformation_attributes_fail() {
    for source in [
        "#[cfg(all(test, unix))] fn work() {}",
        "#[cfg(test = \"true\")] fn work() {}",
        "#[cfg(test(unix))] fn work() {}",
        "#[cfg(not(test))] fn work() {}",
        "#[cfg(any(test, unix))] fn work() {}",
        "#[cfg(all(feature = \"test-helpers\", unix))] fn work() {}",
        "#[cfg_attr(test, allow(dead_code))] fn work() {}",
        "#[path = \"other.rs\"] mod work;",
        "#[coverage(off)] fn work() {}",
        "#[custom_transform] fn work() {}",
        "#[derive(CustomTransform)] struct Work;",
        "fn work() { if cfg!(test) { observe(); } }",
    ] {
        assert!(
            classify_source("src/lib.rs", source).is_err(),
            "accepted unsupported source: {source}"
        );
    }
}

// Macros may introduce source or hide ownership syntax from syn's AST. Accepting
// these without classifying their expansion would let code evade source review.
#[test]
fn source_introducing_macros_and_opaque_ownership_fail() {
    for source in [
        "include!(\"generated.rs\");",
        "macro_rules! generate { () => { fn work() {} } }",
        "generate!();",
        "fn work() { include!(\"expression.rs\"); }",
        "fn work() { std::include!(\"expression.rs\"); }",
        "fn work() { dependency::observed!(); }",
        "fn work() { wrapper!(); }",
        "fn work() { wrapper!(#[cfg(test)] observe()); }",
        "fn work() { assert!(cfg!(test)); }",
    ] {
        assert!(
            classify_source("src/lib.rs", source).is_err(),
            "accepted unclassified source: {source}"
        );
    }
    assert!(
        classify_source(
            "src/lib.rs",
            "fn work() { println!(\"#[cfg(test)] include!\"); }"
        )
        .is_ok()
    );
    assert!(classify_source("src/lib.rs", "fn work() { std::println!(\"observed\"); }").is_ok());
    assert!(
        classify_source(
            "src/lib.rs",
            "fn work() { let text = stringify!(#[cfg(test)] include!(\"x\")); }"
        )
        .is_ok()
    );
}

// A reviewed outer macro still expands macros within its arguments. A custom
// inner expansion can hide test-dependent code, including under qualified names.
#[test]
fn nested_macro_paths_require_source_ownership_review() {
    for source in [
        "fn work() { format!(\"{}\", unreviewed!()); }",
        "fn work() { format!(\"{}\", dependency::println!()); }",
        "fn work() { vec![format!(\"{}\", ::dependency::unreviewed!())]; }",
        "fn work() { assert!(std::cfg!(test)); }",
        "fn work() { format!(\"{}\", std::include!(\"expression.rs\")); }",
    ] {
        let error = classify_source("src/lib.rs", source).unwrap_err();
        assert!(
            error.contains("ownership") || error.contains("unclassified source"),
            "{error}"
        );
    }
}

// Blocks within macro arguments can introduce bindings that shadow the trusted
// macro names or namespaces. Those opaque declarations need AST ownership review.
#[test]
fn opaque_macro_bindings_require_source_ownership_review() {
    for source in [
        "fn work() { format!(\"{}\", { use dependency::observed as println; println!(); 1 }); }",
        "fn work() { vec![{ use dependency as std; std::println!(); 1 }]; }",
        "fn work() { format!(\"{}\", { extern crate dependency as std; std::println!(); 1 }); }",
        "fn work() { format!(\"{}\", { mod std {} std::println!(); 1 }); }",
        "fn work() { format!(\"{}\", { type std = usize; 1 }); }",
        "fn work() { format!(\"{}\", { struct core; 1 }); }",
        "fn work() { format!(\"{}\", { fn hidden() {} 1 }); }",
    ] {
        let error = classify_source("src/lib.rs", source).unwrap_err();
        assert!(error.contains("declarations or imports"), "{error}");
    }
}

// Reviewed nested macros and ordinary unary negation must remain production
// source. Checking opaque arguments must not alter their executable ownership.
#[test]
fn reviewed_nested_macros_remain_production_source() {
    let source = "fn work() {\n    let text = format!(\"{}\", std::line!());\n    let values = vec![format!(\"{}\", core::column!())];\n    assert!(!matches!(Some(1), None));\n}\n";
    let file = classify_source("src/lib.rs", source).unwrap();
    assert!(lines(&file, "nonproduction_lines").is_empty());
    assert_eq!(lines(&file, "production_token_lines"), vec![1, 2, 3, 4, 5]);
}

// stringify! renders tokens without expanding them. Even nested literal input
// may spell attributes, unknown macros, or imports without creating any source.
#[test]
fn nested_stringify_inputs_are_literal_tokens() {
    let source = "fn work() {\n    let text = format!(\"{}\", std::stringify!(#[cfg(test)] include!(\"x\"); dependency::unknown!();));\n    let values = vec![stringify!(use dependency as std; mod core {} type alloc = usize;)];\n}\n";
    let file = classify_source("src/lib.rs", source).unwrap();
    assert!(lines(&file, "nonproduction_lines").is_empty());
    assert_eq!(lines(&file, "production_token_lines"), vec![1, 2, 3, 4]);
}

// Imported macro names are resolved by Rust's namespaces, not by their spelling.
// An alias or glob could replace a reviewed macro with code that changes under
// cfg(test). The classifier must require review before trusting such expansion.
#[test]
fn production_imports_cannot_shadow_reviewed_macros_or_derives() {
    for source in [
        "use dependency::observed as println; fn work() { println!(); }",
        "pub use dependency::println; fn work() { println!(); }",
        "use dependency::{other, nested::{observed as format}}; fn work() { format!(); }",
        "use dependency::CustomDerive as Serialize; #[derive(Serialize)] struct Work;",
        "use dependency::Error; #[derive(Error)] enum Work {}",
        "use dependency::*; fn work() { println!(); }",
        "#[cfg(test)] use dependency::*; fn work() { println!(); }",
        "#[cfg(test)] use dependency::observed as println; fn work() { println!(); }",
        "use std::cfg as println; fn work() { println!(test); }",
        "use dependency::observed as r#println; fn work() { println!(); }",
        "fn work() { if r#cfg!(test) { observe(); } }",
        "fn work() { assert!(r#cfg!(test)); }",
    ] {
        assert!(
            classify_source("src/lib.rs", source).is_err(),
            "accepted shadowing source: {source}"
        );
    }
}

// Qualified spellings are only meaningful while their namespace remains the
// reviewed standard crate or derive provider. Source-level namespace bindings
// otherwise let an unrelated expansion masquerade as std::println or serde.
#[test]
fn production_namespace_bindings_require_ownership_review() {
    for source in [
        "use dependency as std; fn work() { std::println!(); }",
        "use dependency::{self as serde}; #[derive(serde::Serialize)] struct Work;",
        "mod std {}",
        "#[cfg(test)] mod std {} fn work() { std::println!(); }",
        "struct core;",
        "enum alloc { Entry }",
        "type serde = usize;",
        "trait thiserror {}",
        "extern crate dependency as std;",
        "fn work<core>() {}",
        "use dependency as r#std; fn work() { std::println!(); }",
    ] {
        assert!(
            classify_source("src/lib.rs", source).is_err(),
            "accepted namespace shadowing: {source}"
        );
    }
}

// Explicit imports of reviewed providers are ordinary production syntax. A
// wholly test-owned module may use globs without changing sibling production
// bindings, as the repository's existing inline unit tests do.
#[test]
fn canonical_imports_and_inherited_test_globs_remain_supported() {
    let source = "use std::{self, println, format};\nuse core::clone::Clone;\nuse std::fmt::{Debug, Write};\nuse serde::{Deserialize, Serialize};\nuse thiserror::Error;\n#[derive(Clone, Debug, Serialize, Deserialize)] struct Work;\nfn work() { println!(\"ordinary production\"); }\n#[cfg(test)] mod tests { use super::*; use dependency::*; mod std {} #[test] fn checks() { println!(\"test\"); } }\n";
    let file = classify_source("src/lib.rs", source).unwrap();
    assert_eq!(
        lines(&file, "production_token_lines"),
        (1..=7).collect::<Vec<_>>()
    );
    assert_eq!(lines(&file, "nonproduction_lines"), vec![8]);
}

// The source itself can prove a glob imports only enum variants. The enum must
// exist in this module in both builds, without another binding redirecting it.
#[test]
fn unconditional_local_enum_variant_globs_remain_supported() {
    let source = "#[derive(Debug)] enum ResultKind { Good, Bad }\nimpl ResultKind { fn code(&self) -> u8 { use ResultKind::*; match self { Good => 0, Bad => 1 } } }\nmod nested { enum Choice { First } fn work() { use Choice::*; let choice = First; } }\n";
    let file = classify_source("src/lib.rs", source).unwrap();
    assert!(lines(&file, "nonproduction_lines").is_empty());
    assert_eq!(lines(&file, "production_token_lines"), vec![1, 2, 3]);
}

// Names alone do not prove glob ownership: aliases, conditional declarations,
// child modules, and type bindings can change the path's meaning between builds.
#[test]
fn unproven_enum_variant_globs_require_source_ownership_review() {
    for source in [
        "fn work() { use Unknown::*; }",
        "use dependency::module as Choice; fn work() { use Choice::*; }",
        "enum Choice { First } use dependency::module as Alias; fn work() { use Alias::*; }",
        "#[cfg(unix)] enum Choice { First } fn work() { use Choice::*; }",
        "#[cfg(test)] enum Choice { First } fn work() { use Choice::*; }",
        "enum Choice { First } fn work() { #[cfg(test)] use dependency::module as Choice; use Choice::*; }",
        "enum Choice { First } fn work() { #[cfg(test)] mod Choice {} use Choice::*; }",
        "enum Choice { First } fn work() { #[cfg(test)] extern crate dependency as Choice; use Choice::*; }",
        "enum Choice { First } fn work() { type Choice = usize; use Choice::*; }",
        "enum Choice { First } fn work<Choice>() { use Choice::*; }",
        "enum Choice { First } mod child { fn work() { use Choice::*; } }",
        "enum Choice { First } mod child { use super::Choice; fn work() { use Choice::*; } }",
        "enum Choice { First } fn work() { use self::Choice::*; }",
        "enum Choice { First } fn work() { use ::Choice::*; }",
        "#[cfg(unix)] mod conditional { enum Choice { First } fn work() { use Choice::*; } }",
    ] {
        let error = classify_source("src/lib.rs", source).unwrap_err();
        assert!(error.contains("not a proven local enum"), "{error}");
    }
}

// A test-only external declaration changes the ownership of another file. The
// current classifier deliberately refuses that construct instead of guessing.
#[test]
fn external_test_module_fails_even_when_ownership_is_inherited() {
    for source in [
        "#[cfg(test)] mod tests;",
        "#[cfg(test)] mod tests { mod helpers; }",
        "#![cfg(test)]\nmod helpers;",
    ] {
        let error = classify_source("src/lib.rs", source).unwrap_err();
        assert!(error.contains("inherited file ownership"), "{error}");
    }
    assert!(classify_source("src/lib.rs", "mod live;").is_ok());
}

struct SourceTree(std::path::PathBuf);

impl SourceTree {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("outpost-ownership-{}-{serial}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        std::fs::create_dir(path.join("src")).unwrap();
        Self(path)
    }
}

impl Drop for SourceTree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

// New or untracked source cannot depend on a hand-maintained allowlist. Every
// Rust file must appear, including production files with test-looking names.
#[test]
fn inventory_includes_every_rust_file_and_rejects_bad_source() {
    let tree = SourceTree::new();
    std::fs::write(tree.0.join("src/lib.rs"), "mod test_support;").unwrap();
    std::fs::write(tree.0.join("src/test_support.rs"), "pub fn production() {}").unwrap();
    std::fs::create_dir(tree.0.join("src/nested")).unwrap();
    std::fs::write(tree.0.join("src/nested/new.rs"), "fn new() {}").unwrap();
    let report = classify_tree(&tree.0, std::path::Path::new("src")).unwrap();
    let paths: Vec<_> = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["src/lib.rs", "src/nested/new.rs", "src/test_support.rs"]
    );
    assert!(
        report["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|file| lines(file, "nonproduction_lines").is_empty())
    );
    std::fs::write(tree.0.join("src/nested/new.rs"), "invalid Rust } ").unwrap();
    assert!(classify_tree(&tree.0, std::path::Path::new("src")).is_err());
}

// Empty, missing, or escaped scope is incomplete measurement evidence. The
// classifier must not let a caller manufacture a successful empty inventory.
#[test]
fn missing_empty_and_traversing_source_scopes_fail() {
    let tree = SourceTree::new();
    for source in ["src", "missing", "", "../src", "/tmp"] {
        assert!(
            classify_tree(&tree.0, std::path::Path::new(source)).is_err(),
            "accepted scope {source}"
        );
    }
}

#[cfg(unix)]
#[test]
fn symlinks_cannot_redirect_the_source_inventory() {
    use std::os::unix::fs::symlink;
    let tree = SourceTree::new();
    std::fs::write(tree.0.join("src/lib.rs"), "fn production() {}").unwrap();
    symlink("lib.rs", tree.0.join("src/alias.rs")).unwrap();
    assert!(
        classify_tree(&tree.0, std::path::Path::new("src"))
            .unwrap_err()
            .contains("symlinks")
    );
}
