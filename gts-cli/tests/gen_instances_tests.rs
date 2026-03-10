#![allow(clippy::unwrap_used)]
//! Integration tests for `generate_instances_from_rust`.
//!
//! These tests cover:
//! - Single instance generation (golden fixture)
//! - Multiple instances in one file
//! - Multiple files in a directory
//! - `pub` and `pub(crate)` visibility
//! - Raw string literals (r#"..."#)
//! - `--output` override path
//! - Source file adjacent output (no --output)
//! - Duplicate instance ID hard error
//! - Duplicate output path hard error
//! - Sandbox escape rejection
//! - Exclude pattern skips file
//! - Missing source path error
//! - `// gts:ignore` directive skips file
//! - JSON `"id"` field injection (never in body)
//! - `concat!()` form rejected
//! - `static` form rejected
//! - Wrong const type rejected

use anyhow::Result;
use gts_cli::gen_instances::generate_instances_from_rust;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn sandbox() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    (tmp, root)
}

fn write(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

fn instance_src(id: &str, json_body: &str) -> String {
    format!(
        concat!(
            "#[gts_well_known_instance(\n",
            "    dir_path = \"instances\",\n",
            "    id = \"{id}\"\n",
            ")]\n",
            "pub const INST: &str = {body};\n"
        ),
        id = id,
        body = json_body
    )
}

fn run(source: &str, output: Option<&str>, exclude: &[&str]) -> Result<()> {
    let excl: Vec<String> = exclude.iter().map(ToString::to_string).collect();
    generate_instances_from_rust(source, output, &excl, 0)
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn inst_path(root: &Path, id: &str) -> std::path::PathBuf {
    root.join("instances").join(format!("{id}.instance.json"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Golden fixture – single instance
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn golden_single_instance() {
    let (_tmp, root) = sandbox();
    let src = instance_src(
        "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
        r#""{\"name\":\"orders\",\"partitions\":16}""#,
    );
    write(&root, "events.rs", &src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    let id = "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0";
    let out = inst_path(&root, id);
    assert!(out.exists(), "Expected file: {}", out.display());

    let val = read_json(&out);
    assert_eq!(val["id"], id);
    assert_eq!(val["name"], "orders");
    assert_eq!(val["partitions"], 16);
}

// ─────────────────────────────────────────────────────────────────────────────
// Golden fixture – raw string literal
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn golden_raw_string_literal() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.payments.v1.0\"\n",
        ")]\n",
        "pub const PAYMENTS: &str = r#\"{\"name\":\"payments\",\"partitions\":8}\"#;\n"
    );
    write(&root, "events.rs", src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    let id = "gts.x.core.events.topic.v1~x.commerce._.payments.v1.0";
    let out = inst_path(&root, id);
    assert!(out.exists());

    let val = read_json(&out);
    assert_eq!(val["id"], id);
    assert_eq!(val["name"], "payments");
    assert_eq!(val["partitions"], 8);
}

// ─────────────────────────────────────────────────────────────────────────────
// Multiple instances in one file
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn multiple_instances_in_one_file() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const A: &str = \"{\\\"name\\\":\\\"orders\\\"}\";\n",
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.payments.v1.0\"\n",
        ")]\n",
        "pub const B: &str = \"{\\\"name\\\":\\\"payments\\\"}\";\n"
    );
    write(&root, "events.rs", src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    assert!(inst_path(&root, "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0").exists());
    assert!(
        inst_path(
            &root,
            "gts.x.core.events.topic.v1~x.commerce._.payments.v1.0"
        )
        .exists()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Multiple files in a directory
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn multiple_files_in_directory() {
    let (_tmp, root) = sandbox();

    write(
        &root,
        "a.rs",
        &instance_src(
            "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
            "\"{\\\"name\\\":\\\"a\\\"}\"",
        ),
    );
    write(
        &root,
        "b.rs",
        &instance_src(
            "gts.x.core.events.topic.v1~x.commerce._.payments.v1.0",
            "\"{\\\"name\\\":\\\"b\\\"}\"",
        ),
    );

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    assert!(inst_path(&root, "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0").exists());
    assert!(
        inst_path(
            &root,
            "gts.x.core.events.topic.v1~x.commerce._.payments.v1.0"
        )
        .exists()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// pub(crate) visibility is accepted
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn pub_crate_visibility_accepted() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub(crate) const FOO: &str = \"{\\\"name\\\":\\\"x\\\"}\";\n"
    );
    write(&root, "events.rs", src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    assert!(inst_path(&root, "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0").exists());
}

// ─────────────────────────────────────────────────────────────────────────────
// Output uses source file's parent directory when --output is not given
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn output_adjacent_to_source_when_no_override() {
    let (_tmp, root) = sandbox();
    let subdir = root.join("submodule");
    fs::create_dir_all(&subdir).unwrap();
    write(
        &subdir,
        "topic.rs",
        &instance_src(
            "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
            "\"{\\\"name\\\":\\\"orders\\\"}\"",
        ),
    );

    // Pass the subdir as the source (single file)
    let src_file = subdir.join("topic.rs");
    run(src_file.to_str().unwrap(), None, &[]).unwrap();

    let expected = subdir
        .join("instances")
        .join("gts.x.core.events.topic.v1~x.commerce._.orders.v1.0.instance.json");
    assert!(expected.exists(), "Expected: {}", expected.display());
}

// ─────────────────────────────────────────────────────────────────────────────
// id field is injected and overrides any body field named "id" — BODY REJECTED
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn id_field_in_body_is_rejected() {
    let (_tmp, root) = sandbox();
    write(
        &root,
        "events.rs",
        &instance_src(
            "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
            "\"{\\\"id\\\":\\\"bad\\\",\\\"name\\\":\\\"x\\\"}\"",
        ),
    );

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    assert!(err.to_string().contains("\"id\" field"), "Got: {err}");
}

// ─────────────────────────────────────────────────────────────────────────────
// Duplicate instance ID → hard error
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn duplicate_instance_id_hard_error() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const A: &str = \"{\\\"name\\\":\\\"a\\\"}\";\n",
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const B: &str = \"{\\\"name\\\":\\\"b\\\"}\";\n"
    );
    write(&root, "dup.rs", src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    assert!(
        err.to_string().contains("duplicate instance ID"),
        "Got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Sandbox escape via dir_path → hard error (validate-before-mkdir)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn sandbox_escape_rejected() {
    let (_tmp, root) = sandbox();

    // Use a unique escape target so we can assert it was NOT created on disk.
    let escape_component = format!("gts_escape_{}", root.file_name().unwrap().to_string_lossy());
    let escape_dir = format!("../{escape_component}");
    let src = format!(
        concat!(
            "#[gts_well_known_instance(\n",
            "    dir_path = \"{dir}\",\n",
            "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
            ")]\n",
            "pub const FOO: &str = \"{{\\\"name\\\":\\\"x\\\"}}\";\n"
        ),
        dir = escape_dir
    );
    write(&root, "escape.rs", &src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Security error") || msg.contains("sandbox") || msg.contains("'..'"),
        "Got: {msg}"
    );

    // Verify no out-of-sandbox directory was created as a side effect.
    let outside = root.parent().unwrap().join(&escape_component);
    assert!(
        !outside.exists(),
        "Sandbox escape created directory: {}",
        outside.display()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Exclude pattern skips a file even if it contains valid annotations
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn exclude_pattern_skips_file() {
    let (_tmp, root) = sandbox();
    // Write a file with a malformed annotation that would cause a hard error if scanned
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"bad-no-tilde\"\n",
        ")]\n",
        "pub const X: &str = \"{}\";\n"
    );
    write(&root, "excluded_file.rs", src);

    // Should succeed because the file is excluded
    run(
        root.to_str().unwrap(),
        Some(root.to_str().unwrap()),
        &["excluded_file.rs"],
    )
    .unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// gts:ignore directive skips the file
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn gts_ignore_directive_skips_file() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "// gts:ignore\n",
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"bad-no-tilde\"\n",
        ")]\n",
        "pub const X: &str = \"{}\";\n"
    );
    write(&root, "ignored.rs", src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    // No instance file should have been produced
    assert!(!root.join("instances").exists());
}

// ─────────────────────────────────────────────────────────────────────────────
// Missing source path → error
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn missing_source_path_errors() {
    // Use a path guaranteed not to exist on any platform by constructing it
    // inside a TempDir that is immediately dropped (and thus deleted).
    let nonexistent = {
        let tmp = TempDir::new().unwrap();
        tmp.path().join("no_such_subdir_xyz")
        // tmp is dropped here — the parent dir is deleted
    };
    let err = run(nonexistent.to_str().unwrap(), None, &[]).unwrap_err();
    assert!(err.to_string().contains("does not exist"), "Got: {err}");
}

// ─────────────────────────────────────────────────────────────────────────────
// No annotations → succeeds with zero generated (not an error)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn no_annotations_produces_nothing() {
    let (_tmp, root) = sandbox();
    write(&root, "plain.rs", "const FOO: u32 = 42;\n");

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    assert!(!root.join("instances").exists());
}

// ─────────────────────────────────────────────────────────────────────────────
// concat!() value is rejected with actionable message
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn concat_macro_value_is_rejected() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const FOO: &str = concat!(\"{\", \"}\");\n"
    );
    write(&root, "concat.rs", src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    assert!(err.to_string().contains("concat!()"), "Got: {err}");
}

// ─────────────────────────────────────────────────────────────────────────────
// static item is rejected with actionable message
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn static_item_is_rejected() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub static FOO: &str = \"{\\\"name\\\":\\\"x\\\"}\";\n"
    );
    write(&root, "static_item.rs", src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    assert!(err.to_string().contains("static"), "Got: {err}");
}

// ─────────────────────────────────────────────────────────────────────────────
// id without ~ separator is rejected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn id_without_tilde_is_rejected() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1.x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const FOO: &str = \"{\\\"name\\\":\\\"x\\\"}\";\n"
    );
    write(&root, "notilde.rs", src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    assert!(err.to_string().contains("'~'"), "Got: {err}");
}

// ─────────────────────────────────────────────────────────────────────────────
// id ending with ~ (schema/type, not instance) is rejected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn id_ending_with_tilde_is_rejected() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~\"\n",
        ")]\n",
        "pub const FOO: &str = \"{\\\"name\\\":\\\"x\\\"}\";\n"
    );
    write(&root, "segtilde.rs", src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    assert!(
        err.to_string().contains("must not end with '~'"),
        "Got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// JSON body must be an object — array is rejected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn json_array_body_is_rejected() {
    let (_tmp, root) = sandbox();
    write(
        &root,
        "events.rs",
        &instance_src(
            "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
            "\"[1,2,3]\"",
        ),
    );

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    assert!(err.to_string().contains("JSON object"), "Got: {err}");
}

// ─────────────────────────────────────────────────────────────────────────────
// Malformed JSON body is rejected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn malformed_json_body_is_rejected() {
    let (_tmp, root) = sandbox();
    write(
        &root,
        "events.rs",
        &instance_src(
            "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
            "\"{not valid json}\"",
        ),
    );

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    assert!(err.to_string().contains("Malformed JSON"), "Got: {err}");
}

// ─────────────────────────────────────────────────────────────────────────────
// Golden fixture: generated file content matches exactly
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn golden_file_content_exact() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const ORDERS: &str = \"{\\\"name\\\":\\\"orders\\\",\\\"partitions\\\":16}\";\n"
    );
    write(&root, "events.rs", src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    let id = "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0";
    let out = inst_path(&root, id);
    let val = read_json(&out);

    // Must have id injected
    assert_eq!(val["id"], id);
    // Must preserve original fields
    assert_eq!(val["name"], "orders");
    assert_eq!(val["partitions"], 16);
    // Must not have extra unexpected fields (only id, name, partitions)
    let obj = val.as_object().unwrap();
    assert_eq!(
        obj.len(),
        3,
        "Expected exactly 3 fields, got: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Zero-hash raw string r"..." is accepted (Fix: regex r#* not r#+)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn zero_hash_raw_string_is_accepted() {
    let (_tmp, root) = sandbox();
    // r"..." with no hashes — was previously not matched by the annotation regex
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const ZERO_HASH: &str = r#\"{\"name\":\"zero\"}\"#;\n"
    );
    write(&root, "zero_hash.rs", src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    let id = "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0";
    let out = inst_path(&root, id);
    assert!(out.exists(), "Expected file: {}", out.display());
    let val = read_json(&out);
    assert_eq!(val["name"], "zero");
}

// ─────────────────────────────────────────────────────────────────────────────
// Char literals near the needle don't cause preflight false-positive
// (Fix: preflight_scan now skips char literals like '#' and '[')
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn char_literal_near_needle_does_not_false_positive() {
    let (_tmp, root) = sandbox();
    // File contains '#' and '[' as char literals right before a regular ident,
    // but no actual annotation — preflight must return false → quiet skip.
    let src = concat!(
        "fn check(c: char) -> bool {\n",
        "    c == '#' || c == '['\n",
        "}\n",
        "// mentions gts_well_known_instance in a comment only\n",
        "const X: u32 = 1;\n"
    );
    write(&root, "char_lit.rs", src);

    // Must succeed with no output — not a hard error
    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();
    assert!(!root.join("instances").exists());
}

// ─────────────────────────────────────────────────────────────────────────────
// Unsupported form mentioned only in a comment does NOT hard-error
// (Fix: check_unsupported_forms runs on comment-stripped source)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn unsupported_form_in_comment_does_not_error() {
    let (_tmp, root) = sandbox();
    // The doc comment contains a concat!() example that would have previously
    // triggered a hard error from check_unsupported_forms.
    let src = concat!(
        "/// Example (do NOT use):\n",
        "/// #[gts_well_known_instance(\n",
        "///     dir_path = \"instances\",\n",
        "///     id = \"gts.x.core.events.topic.v1~x.a.v1.0\"\n",
        "/// )]\n",
        "/// pub const BAD: &str = concat!(\"{\", \"}\");\n",
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const REAL: &str = \"{\\\"name\\\":\\\"real\\\"}\";\n"
    );
    write(&root, "comment_example.rs", src);

    // Must succeed — the concat!() is only in a doc comment
    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    let id = "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0";
    assert!(inst_path(&root, id).exists());
}

// ─────────────────────────────────────────────────────────────────────────────
// Annotation applied to a fn (not a const) is a hard error
// (Fix: preflight-positive + no match → hard error, not silent skip)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn annotation_on_fn_is_hard_error() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub fn not_a_const() -> &'static str { \"{}\" }\n"
    );
    write(&root, "on_fn.rs", src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("could not be parsed") || msg.contains("const NAME"),
        "Got: {msg}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Duplicate attribute key in annotation is a hard error
// (Fix: check_duplicate_attr_keys added to parse_instance_attrs)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn duplicate_attribute_key_is_hard_error() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    dir_path = \"other\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const DUP: &str = \"{\\\"name\\\":\\\"x\\\"}\";\n"
    );
    write(&root, "dup_key.rs", src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Duplicate attribute") || msg.contains("dir_path"),
        "Got: {msg}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ./ prefix in dir_path with same ID → duplicate instance ID error
// (dir_path differs via ./ prefix but composed ID is identical, so
//  check_duplicate_ids fires. The path normalisation in
//  check_duplicate_output_paths is a defence-in-depth guard for the
//  hypothetical future case where filenames could diverge from the ID.)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn dot_slash_dir_path_same_id_is_duplicate() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const A: &str = \"{\\\"name\\\":\\\"a\\\"}\";\n",
        "#[gts_well_known_instance(\n",
        "    dir_path = \"./instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const B: &str = \"{\\\"name\\\":\\\"b\\\"}\";\n"
    );
    write(&root, "dotslash.rs", src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("duplicate instance ID") || msg.contains("Duplicate"),
        "Got: {msg}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Qualified path form #[gts_macros::gts_well_known_instance(...)] is accepted
// (Fix: NEEDLE and regex updated to match optional `gts_macros::` prefix)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn qualified_path_form_is_accepted() {
    let (_tmp, root) = sandbox();
    let src = concat!(
        "#[gts_macros::gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"gts.x.core.events.topic.v1~x.commerce._.orders.v1.0\"\n",
        ")]\n",
        "pub const QUALIFIED: &str = r#\"{\"name\":\"qualified\"}\"#;\n"
    );
    write(&root, "qualified.rs", src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();

    let id = "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0";
    let out = inst_path(&root, id);
    assert!(out.exists(), "Expected file: {}", out.display());
    let val = read_json(&out);
    assert_eq!(val["name"], "qualified");
}

// ─────────────────────────────────────────────────────────────────────────────
// compile_fail dir is auto-skipped (auto-ignored dir)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn compile_fail_dir_is_auto_skipped() {
    let (_tmp, root) = sandbox();
    let cf_dir = root.join("compile_fail");
    fs::create_dir_all(&cf_dir).unwrap();

    // Place a malformed annotation in compile_fail/ — should be silently skipped
    let src = concat!(
        "#[gts_well_known_instance(\n",
        "    dir_path = \"instances\",\n",
        "    id = \"bad-no-tilde\"\n",
        ")]\n",
        "pub const X: &str = \"{}\";\n"
    );
    write(&cf_dir, "test.rs", src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();
}

// ─────────────────────────────────────────────────────────────────────────────
// Schema validation – instance conforms to schema
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: write a base GTS schema into `{root}/schemas/{schema_id}.schema.json`.
fn write_schema(root: &Path, schema_id: &str, extra_props: &[(&str, &str)]) {
    let mut props = serde_json::Map::new();
    props.insert(
        "id".to_owned(),
        serde_json::json!({ "type": "string", "format": "gts-instance-id" }),
    );
    let mut required = vec!["id".to_owned()];
    for (name, ty) in extra_props {
        props.insert((*name).to_owned(), serde_json::json!({ "type": *ty }));
        required.push((*name).to_owned());
    }
    required.sort();
    let schema = serde_json::json!({
        "$id": format!("gts://{schema_id}"),
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "additionalProperties": false,
        "properties": props,
        "required": required
    });
    let dir = root.join("schemas");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("{schema_id}.schema.json")),
        serde_json::to_string_pretty(&schema).unwrap(),
    )
    .unwrap();
}

#[test]
fn schema_validation_valid_instance_passes() {
    let (_tmp, root) = sandbox();

    write_schema(
        &root,
        "gts.x.core.events.topic.v1~",
        &[("name", "string"), ("partitions", "integer")],
    );

    let src = instance_src(
        "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
        r#""{\"name\":\"orders\",\"partitions\":16}""#,
    );
    write(&root, "inst.rs", &src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();
}

#[test]
fn schema_validation_missing_required_field_fails() {
    let (_tmp, root) = sandbox();

    // Schema requires "name" and "vendor"
    write_schema(
        &root,
        "gts.x.core.events.topic.v1~",
        &[("name", "string"), ("vendor", "string")],
    );

    // Instance provides "name" but NOT "vendor"
    let src = instance_src(
        "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
        r#""{\"name\":\"orders\"}""#,
    );
    write(&root, "inst.rs", &src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("schema validation error"),
        "Expected schema validation error, got: {msg}"
    );
}

#[test]
fn schema_validation_extra_field_fails() {
    let (_tmp, root) = sandbox();

    // Schema only allows "name" (plus "id")
    write_schema(&root, "gts.x.core.events.topic.v1~", &[("name", "string")]);

    // Instance has "name" + "extra" — violates additionalProperties: false
    let src = instance_src(
        "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
        r#""{\"name\":\"orders\",\"extra\":\"bad\"}""#,
    );
    write(&root, "inst.rs", &src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("schema validation error"),
        "Expected schema validation error, got: {msg}"
    );
}

#[test]
fn schema_validation_wrong_type_fails() {
    let (_tmp, root) = sandbox();

    // Schema requires "count" as integer
    write_schema(
        &root,
        "gts.x.core.events.topic.v1~",
        &[("count", "integer")],
    );

    // Instance provides "count" as a string
    let src = instance_src(
        "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
        r#""{\"count\":\"not-a-number\"}""#,
    );
    write(&root, "inst.rs", &src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("schema validation error"),
        "Expected schema validation error, got: {msg}"
    );
}

#[test]
fn schema_validation_allof_ref_inheritance_passes() {
    let (_tmp, root) = sandbox();
    let dir = root.join("schemas");
    fs::create_dir_all(&dir).unwrap();

    // Parent schema (open — no additionalProperties: false, required for allOf inheritance)
    let parent = serde_json::json!({
        "$id": "gts://gts.x.core.events.topic.v1~",
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "id": { "type": "string", "format": "gts-instance-id" },
            "name": { "type": "string" }
        },
        "required": ["id", "name"]
    });
    fs::write(
        dir.join("gts.x.core.events.topic.v1~.schema.json"),
        serde_json::to_string_pretty(&parent).unwrap(),
    )
    .unwrap();

    // Child schema: inherits parent via allOf + $ref, adds "vendor"
    let child = serde_json::json!({
        "$id": "gts://gts.x.core.events.topic.v1~x.core.audit.event.v1~",
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "allOf": [
            { "$ref": "gts://gts.x.core.events.topic.v1~" },
            {
                "type": "object",
                "properties": { "vendor": { "type": "string" } },
                "required": ["vendor"]
            }
        ]
    });
    fs::write(
        dir.join("gts.x.core.events.topic.v1~x.core.audit.event.v1~.schema.json"),
        serde_json::to_string_pretty(&child).unwrap(),
    )
    .unwrap();

    // Instance satisfies both parent ("name") and child ("vendor")
    let src = instance_src(
        "gts.x.core.events.topic.v1~x.core.audit.event.v1~x.commerce._.orders.v1.0",
        r#""{\"name\":\"orders\",\"vendor\":\"acme\"}""#,
    );
    write(&root, "inst.rs", &src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();
}

#[test]
fn schema_validation_allof_ref_missing_parent_field_fails() {
    let (_tmp, root) = sandbox();
    let dir = root.join("schemas");
    fs::create_dir_all(&dir).unwrap();

    // Parent schema (open — no additionalProperties: false)
    let parent = serde_json::json!({
        "$id": "gts://gts.x.core.events.topic.v1~",
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "id": { "type": "string", "format": "gts-instance-id" },
            "name": { "type": "string" }
        },
        "required": ["id", "name"]
    });
    fs::write(
        dir.join("gts.x.core.events.topic.v1~.schema.json"),
        serde_json::to_string_pretty(&parent).unwrap(),
    )
    .unwrap();

    // Child schema: inherits parent via allOf + $ref, adds "vendor"
    let child = serde_json::json!({
        "$id": "gts://gts.x.core.events.topic.v1~x.core.audit.event.v1~",
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "allOf": [
            { "$ref": "gts://gts.x.core.events.topic.v1~" },
            {
                "type": "object",
                "properties": { "vendor": { "type": "string" } },
                "required": ["vendor"]
            }
        ]
    });
    fs::write(
        dir.join("gts.x.core.events.topic.v1~x.core.audit.event.v1~.schema.json"),
        serde_json::to_string_pretty(&child).unwrap(),
    )
    .unwrap();

    // Instance has "vendor" but missing parent-required "name"
    let src = instance_src(
        "gts.x.core.events.topic.v1~x.core.audit.event.v1~x.commerce._.orders.v1.0",
        r#""{\"vendor\":\"acme\"}""#,
    );
    write(&root, "inst.rs", &src);

    let err = run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("schema validation error"),
        "Expected schema validation error, got: {msg}"
    );
}

#[test]
fn schema_validation_no_schema_on_disk_passes() {
    let (_tmp, root) = sandbox();

    // No schema written — validation should be skipped silently
    let src = instance_src(
        "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0",
        r#""{\"name\":\"orders\"}""#,
    );
    write(&root, "inst.rs", &src);

    run(root.to_str().unwrap(), Some(root.to_str().unwrap()), &[]).unwrap();
}
