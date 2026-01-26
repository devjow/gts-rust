use anyhow::{Result, bail};
use gts::{GtsInstanceId, GtsSchemaId};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Directories that are automatically ignored (e.g., trybuild `compile_fail` tests)
const AUTO_IGNORE_DIRS: &[&str] = &["compile_fail"];

/// Reason why a file was skipped
#[derive(Debug, Clone, Copy)]
enum SkipReason {
    ExcludePattern,
    AutoIgnoredDir,
    IgnoreDirective,
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExcludePattern => write!(f, "matched --exclude pattern"),
            Self::AutoIgnoredDir => write!(f, "in auto-ignored directory (compile_fail)"),
            Self::IgnoreDirective => write!(f, "has // gts:ignore directive"),
        }
    }
}

/// Parsed macro attributes from `#[struct_to_gts_schema(...)]`
#[derive(Debug, Clone)]
struct MacroAttrs {
    dir_path: String,
    schema_id: String,
    description: Option<String>,
    properties: Option<String>,
    base: BaseAttr,
}

/// Base attribute type
#[derive(Debug, Clone)]
enum BaseAttr {
    /// `base = true` - this is a base type
    IsBase,
    /// `base = ParentStruct` - this type inherits from `ParentStruct`
    Parent(String),
}

/// Generate GTS schemas from Rust source code with `#[struct_to_gts_schema]` annotations
///
/// # Arguments
/// * `source` - Source directory or file to scan
/// * `output` - Optional output directory override
/// * `exclude_patterns` - Patterns to exclude (supports simple glob matching)
/// * `verbose` - Verbosity level (0 = normal, 1+ = show skipped files)
pub fn generate_schemas_from_rust(
    source: &str,
    output: Option<&str>,
    exclude_patterns: &[String],
    verbose: u8,
) -> Result<()> {
    println!("Scanning Rust source files in: {source}");

    let source_path = Path::new(source);
    if !source_path.exists() {
        bail!("Source path does not exist: {source}");
    }

    // Canonicalize source path to detect path traversal attempts
    let source_canonical = source_path.canonicalize()?;

    let mut schemas_generated = 0;
    let mut files_scanned = 0;
    let mut files_skipped = 0;

    // Walk through all .rs files
    for entry in WalkDir::new(source_path)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }

        // Check if path should be excluded
        if should_exclude_path(path, exclude_patterns) {
            files_skipped += 1;
            if verbose > 0 {
                println!(
                    "  Skipped: {} ({})",
                    path.display(),
                    SkipReason::ExcludePattern
                );
            }
            continue;
        }

        // Check for auto-ignored directories (e.g., compile_fail)
        if is_in_auto_ignored_dir(path) {
            files_skipped += 1;
            if verbose > 0 {
                println!(
                    "  Skipped: {} ({})",
                    path.display(),
                    SkipReason::AutoIgnoredDir
                );
            }
            continue;
        }

        files_scanned += 1;
        if let Ok(content) = fs::read_to_string(path) {
            // Check for gts:ignore directive
            if has_ignore_directive(&content) {
                files_skipped += 1;
                if verbose > 0 {
                    println!(
                        "  Skipped: {} ({})",
                        path.display(),
                        SkipReason::IgnoreDirective
                    );
                }
                continue;
            }

            // Parse the file and extract schema information
            let results = extract_and_generate_schemas(&content, output, &source_canonical, path)?;
            schemas_generated += results.len();
            for (schema_id, file_path) in results {
                println!("  Generated schema: {schema_id} @ {file_path}");
            }
        }
    }

    println!("\nSummary:");
    println!("  Files scanned: {files_scanned}");
    println!("  Files skipped: {files_skipped}");
    println!("  Schemas generated: {schemas_generated}");

    if schemas_generated == 0 {
        println!(
            "\n- No schemas found. Make sure your structs are annotated with `#[struct_to_gts_schema(...)]`"
        );
    }

    Ok(())
}

/// Check if a path matches any of the exclude patterns
fn should_exclude_path(path: &Path, patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();

    for pattern in patterns {
        if matches_glob_pattern(&path_str, pattern) {
            return true;
        }
    }

    false
}

/// Simple glob pattern matching
/// Supports: * (any characters), ** (any path segments)
fn matches_glob_pattern(path: &str, pattern: &str) -> bool {
    // Convert glob pattern to regex
    let regex_pattern = pattern
        .replace('.', r"\.")
        .replace("**", "<<DOUBLESTAR>>")
        .replace('*', "[^/]*")
        .replace("<<DOUBLESTAR>>", ".*");

    if let Ok(re) = Regex::new(&format!("(^|/){regex_pattern}($|/)")) {
        re.is_match(path)
    } else {
        // Fallback to simple contains check
        path.contains(pattern)
    }
}

/// Check if path is in an auto-ignored directory (e.g., `compile_fail`)
fn is_in_auto_ignored_dir(path: &Path) -> bool {
    path.components().any(|component| {
        if let Some(name) = component.as_os_str().to_str() {
            AUTO_IGNORE_DIRS.contains(&name)
        } else {
            false
        }
    })
}

/// Check if file content starts with the gts:ignore directive
fn has_ignore_directive(content: &str) -> bool {
    // Check first few lines for the directive
    for line in content.lines().take(10) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Check for the directive (case-insensitive)
        if trimmed.to_lowercase().starts_with("// gts:ignore") {
            return true;
        }
        // If we hit a non-comment, non-empty line, stop looking
        if !trimmed.starts_with("//") && !trimmed.starts_with("#!") {
            break;
        }
    }
    false
}

/// Parse the attribute body of `#[struct_to_gts_schema(...)]` to extract individual attributes
fn parse_macro_attrs(attr_body: &str) -> Option<MacroAttrs> {
    // Patterns for extracting individual attributes
    let dir_path_re = Regex::new(r#"dir_path\s*=\s*"([^"]+)""#).ok()?;
    let schema_id_re = Regex::new(r#"schema_id\s*=\s*"([^"]+)""#).ok()?;
    let description_re = Regex::new(r#"description\s*=\s*"([^"]+)""#).ok()?;
    let properties_re = Regex::new(r#"properties\s*=\s*"([^"]+)""#).ok()?;
    let base_true_re = Regex::new(r"\bbase\s*=\s*true\b").ok()?;
    let base_parent_re = Regex::new(r"\bbase\s*=\s*([A-Z]\w*)").ok()?;

    // Extract required fields
    let dir_path = dir_path_re.captures(attr_body)?.get(1)?.as_str().to_owned();
    let schema_id = schema_id_re
        .captures(attr_body)?
        .get(1)?
        .as_str()
        .to_owned();

    // Extract optional fields
    let description = description_re
        .captures(attr_body)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()));
    let properties = properties_re
        .captures(attr_body)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()));

    // Parse base attribute
    let base = if base_true_re.is_match(attr_body) {
        BaseAttr::IsBase
    } else if let Some(cap) = base_parent_re.captures(attr_body) {
        BaseAttr::Parent(cap.get(1)?.as_str().to_owned())
    } else {
        // base is required but not found
        return None;
    };

    Some(MacroAttrs {
        dir_path,
        schema_id,
        description,
        properties,
        base,
    })
}

/// Extract schema metadata from Rust source and generate JSON files
/// Returns a vector of (`schema_id`, `file_path`) tuples for each generated schema
fn extract_and_generate_schemas(
    content: &str,
    output_override: Option<&str>,
    source_root: &Path,
    source_file: &Path,
) -> Result<Vec<(String, String)>> {
    // Match #[struct_to_gts_schema(...)] followed by struct definition
    // Captures: (1) attribute body, (2) struct name, (3) optional generics, (4) struct body or semicolon for unit structs
    let re = Regex::new(
        r"(?s)#\[struct_to_gts_schema\(([^)]+)\)\]\s*(?:#\[[^\]]+\]\s*)*(?:pub\s+)?struct\s+(\w+)(?:<([^>]+)>)?\s*(?:\{([^}]*)\}|;)",
    )?;

    // Pre-compile field regex outside the loop
    let field_re = Regex::new(r"(?m)^\s*(?:pub\s+)?(\w+)\s*:\s*([^,\n]+)")?;

    let mut results = Vec::new();

    for cap in re.captures_iter(content) {
        let attr_body = &cap[1];
        let struct_name = &cap[2];
        let _generics = cap.get(3).map(|m| m.as_str());
        let struct_body = cap.get(4).map_or("", |m| m.as_str());

        // Parse macro attributes
        let Some(attrs) = parse_macro_attrs(attr_body) else {
            continue;
        };

        // Convert schema_id to filename-safe format
        // e.g., "gts.x.core.events.type.v1~" -> "gts.x.core.events.type.v1~"
        let schema_file_rel = format!("{}/{}.schema.json", attrs.dir_path, attrs.schema_id);

        // Determine output path
        let output_path = if let Some(output_dir) = output_override {
            // Use CLI-provided output directory
            Path::new(output_dir).join(&schema_file_rel)
        } else {
            // Use path from macro (relative to source file's directory)
            let source_dir = source_file.parent().unwrap_or(source_root);
            source_dir.join(&schema_file_rel)
        };

        // Security check: ensure output path doesn't escape source repository
        let output_canonical = if output_path.exists() {
            output_path.canonicalize()?
        } else {
            // For non-existent files, canonicalize the parent directory
            let parent = output_path.parent().unwrap_or(Path::new("."));
            fs::create_dir_all(parent)?;
            let parent_canonical = parent.canonicalize()?;
            parent_canonical.join(output_path.file_name().unwrap())
        };

        // Check if output path is within source repository
        if !output_canonical.starts_with(source_root) {
            bail!(
                "Security error in {}:{} - dir_path '{}' attempts to write outside source repository. \
                Resolved to: {}, but must be within: {}",
                source_file.display(),
                struct_name,
                attrs.dir_path,
                output_canonical.display(),
                source_root.display()
            );
        }

        // Parse struct fields
        let mut field_types = HashMap::new();

        for field_cap in field_re.captures_iter(struct_body) {
            let field_name = &field_cap[1];
            let field_type = field_cap[2].trim().trim_end_matches(',');
            field_types.insert(field_name.to_owned(), field_type.to_owned());
        }

        // Build JSON schema
        let schema = build_json_schema(
            &attrs.schema_id,
            struct_name,
            attrs.description.as_deref(),
            attrs.properties.as_deref(),
            &attrs.base,
            &field_types,
        );

        // Create parent directories
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write schema file
        fs::write(&output_path, serde_json::to_string_pretty(&schema)?)?;

        // Add to results (schema_id, file_path)
        results.push((attrs.schema_id, output_path.display().to_string()));
    }

    Ok(results)
}

/// Build a JSON Schema object from parsed metadata
fn build_json_schema(
    schema_id: &str,
    struct_name: &str,
    description: Option<&str>,
    properties_list: Option<&str>,
    base: &BaseAttr,
    field_types: &HashMap<String, String>,
) -> serde_json::Value {
    use serde_json::json;

    let mut schema_properties = serde_json::Map::new();
    let mut required = Vec::new();

    // Determine which properties to include
    let property_names: Vec<&str> = if let Some(props) = properties_list {
        props.split(',').map(str::trim).collect()
    } else {
        // If no properties specified, include all fields
        field_types.keys().map(String::as_str).collect()
    };

    for prop in &property_names {
        if let Some(field_type) = field_types.get(*prop) {
            let (is_required, json_type_info) = rust_type_to_json_schema(field_type);

            schema_properties.insert((*prop).to_owned(), json_type_info);

            if is_required {
                required.push((*prop).to_owned());
            }
        }
    }

    // Sort required array for consistent output
    required.sort();

    // Build schema based on whether this has a parent

    match base {
        BaseAttr::IsBase => {
            // Base type - simple flat schema
            let mut s = json!({
                "$id": format!("gts://{schema_id}"),
                "$schema": "http://json-schema.org/draft-07/schema#",
                "title": struct_name,
                "type": "object",
                "additionalProperties": false,
                "properties": schema_properties
            });

            if let Some(desc) = description {
                s["description"] = json!(desc);
            }

            if !required.is_empty() {
                s["required"] = json!(required);
            }

            s
        }
        BaseAttr::Parent(parent_name) => {
            // Child type - use allOf with $ref to parent
            // The parent's schema_id is derived from this schema's ID by removing the last segment
            let parent_schema_id = derive_parent_schema_id(schema_id);

            let mut own_properties = json!({
                "properties": schema_properties
            });

            if !required.is_empty() {
                own_properties["required"] = json!(required);
            }

            let mut s = json!({
                "$id": format!("gts://{schema_id}"),
                "$schema": "http://json-schema.org/draft-07/schema#",
                "title": format!("{struct_name} (extends {parent_name})"),
                "type": "object",
                "allOf": [
                    { "$ref": format!("gts://{parent_schema_id}") },
                    own_properties
                ]
            });

            if let Some(desc) = description {
                s["description"] = json!(desc);
            }

            s
        }
    }
}

/// Derive parent schema ID from child schema ID
/// e.g., "gts.x.core.events.type.v1~x.core.audit.event.v1~" -> "gts.x.core.events.type.v1~"
fn derive_parent_schema_id(schema_id: &str) -> String {
    // Remove trailing ~ if present for processing
    let s = schema_id.trim_end_matches('~');

    // Find the last ~ and take everything before it, then add ~ back
    if let Some(pos) = s.rfind('~') {
        format!("{}~", &s[..pos])
    } else {
        // No parent segment found, this shouldn't happen for child types
        schema_id.to_owned()
    }
}

/// Convert Rust type string to JSON Schema type
/// Returns (`is_required`, `json_schema_value`)
///
/// This function inlines the actual schema definitions for GTS types (like `GtsInstanceId`)
/// to match what schemars generates, including custom extensions like `x-gts-ref`.
fn rust_type_to_json_schema(rust_type: &str) -> (bool, serde_json::Value) {
    use serde_json::json;

    let rust_type = rust_type.trim();

    // Check if it's an Option type
    let is_optional = rust_type.starts_with("Option<");
    let inner_type = if is_optional {
        rust_type
            .strip_prefix("Option<")
            .and_then(|s| s.strip_suffix('>'))
            .unwrap_or(rust_type)
            .trim()
    } else {
        rust_type
    };

    let json_schema = match inner_type {
        "String" | "str" | "&str" => json!({ "type": "string" }),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => json!({ "type": "integer" }),
        "f32" | "f64" => json!({ "type": "number" }),
        "bool" => json!({ "type": "boolean" }),
        t if t.starts_with("Vec<") => {
            let item_type = t
                .strip_prefix("Vec<")
                .and_then(|s| s.strip_suffix('>'))
                .unwrap_or("string");
            let (_, item_schema) = rust_type_to_json_schema(item_type);
            json!({
                "type": "array",
                "items": item_schema
            })
        }
        t if t.starts_with("HashMap<") || t.starts_with("BTreeMap<") => {
            json!({ "type": "object" })
        }
        t if t.contains("Uuid") || t.contains("uuid") => {
            json!({ "type": "string", "format": "uuid" })
        }
        // GtsInstanceId - use the canonical schema from the gts crate
        "GtsInstanceId" => GtsInstanceId::json_schema_value(),
        // GtsSchemaId - use the canonical schema from the gts crate
        "GtsSchemaId" => GtsSchemaId::json_schema_value(),
        // Generic type parameter (e.g., P, T, etc.) - treat as object
        t if t.len() <= 2 && t.chars().all(|c| c.is_ascii_uppercase()) => {
            json!({ "type": "object" })
        }
        // Other types - default to object (could be another struct)
        _ => json!({ "type": "object" }),
    };

    // For Option types, add null to the type array
    let final_schema = if is_optional {
        if let Some(type_val) = json_schema.get("type").and_then(|v| v.as_str()) {
            json!({ "type": [type_val, "null"] })
        } else {
            // For $ref types, wrap in oneOf with null
            json!({
                "oneOf": [
                    json_schema,
                    { "type": "null" }
                ]
            })
        }
    } else {
        json_schema
    };

    (!is_optional, final_schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_glob_pattern() {
        // Test simple patterns
        assert!(matches_glob_pattern(
            "src/tests/compile_fail/test.rs",
            "compile_fail"
        ));
        assert!(matches_glob_pattern(
            "tests/compile_fail/test.rs",
            "compile_fail"
        ));

        // Test wildcard patterns
        assert!(matches_glob_pattern("src/tests/foo.rs", "tests/*"));
        assert!(matches_glob_pattern("src/examples/bar.rs", "examples/*"));

        // Test double-star patterns
        assert!(matches_glob_pattern("a/b/c/d/test.rs", "**/test.rs"));
    }

    #[test]
    fn test_is_in_auto_ignored_dir() {
        assert!(is_in_auto_ignored_dir(Path::new(
            "tests/compile_fail/test.rs"
        )));
        assert!(is_in_auto_ignored_dir(Path::new("src/compile_fail/foo.rs")));
        assert!(!is_in_auto_ignored_dir(Path::new("src/models.rs")));
        assert!(!is_in_auto_ignored_dir(Path::new("tests/integration.rs")));
    }

    #[test]
    fn test_has_ignore_directive() {
        assert!(has_ignore_directive("// gts:ignore\nuse foo::bar;"));
        assert!(has_ignore_directive("// GTS:IGNORE\nuse foo::bar;"));
        assert!(has_ignore_directive(
            "//! Module doc\n// gts:ignore\nuse foo::bar;"
        ));
        assert!(!has_ignore_directive("use foo::bar;\n// gts:ignore"));
        assert!(!has_ignore_directive("use foo::bar;"));
    }

    #[test]
    fn test_parse_macro_attrs_base_true() {
        let attr_body = r#"
            dir_path = "schemas",
            base = true,
            schema_id = "gts.x.core.events.type.v1~",
            description = "Base event type"
        "#;

        let attrs = parse_macro_attrs(attr_body).unwrap();
        assert_eq!(attrs.dir_path, "schemas");
        assert_eq!(attrs.schema_id, "gts.x.core.events.type.v1~");
        assert_eq!(attrs.description.as_deref(), Some("Base event type"));
        assert!(matches!(attrs.base, BaseAttr::IsBase));
    }

    #[test]
    fn test_parse_macro_attrs_base_parent() {
        let attr_body = r#"
            dir_path = "schemas",
            base = BaseEventV1,
            schema_id = "gts.x.core.events.type.v1~x.core.audit.event.v1~"
        "#;

        let attrs = parse_macro_attrs(attr_body).unwrap();
        assert_eq!(attrs.dir_path, "schemas");
        assert_eq!(
            attrs.schema_id,
            "gts.x.core.events.type.v1~x.core.audit.event.v1~"
        );
        assert!(matches!(attrs.base, BaseAttr::Parent(ref p) if p == "BaseEventV1"));
    }

    #[test]
    fn test_derive_parent_schema_id() {
        assert_eq!(
            derive_parent_schema_id("gts.x.core.events.type.v1~x.core.audit.event.v1~"),
            "gts.x.core.events.type.v1~"
        );
        assert_eq!(
            derive_parent_schema_id(
                "gts.x.core.events.type.v1~x.core.audit.event.v1~x.marketplace.orders.purchase.v1~"
            ),
            "gts.x.core.events.type.v1~x.core.audit.event.v1~"
        );
    }

    #[test]
    fn test_rust_type_to_json_schema() {
        // Basic types
        let (req, schema) = rust_type_to_json_schema("String");
        assert!(req);
        assert_eq!(schema["type"], "string");

        let (req, schema) = rust_type_to_json_schema("i32");
        assert!(req);
        assert_eq!(schema["type"], "integer");

        let (req, schema) = rust_type_to_json_schema("bool");
        assert!(req);
        assert_eq!(schema["type"], "boolean");

        // Optional types
        let (req, schema) = rust_type_to_json_schema("Option<String>");
        assert!(!req);
        assert_eq!(schema["type"][0], "string");
        assert_eq!(schema["type"][1], "null");

        // Vec types
        let (req, schema) = rust_type_to_json_schema("Vec<String>");
        assert!(req);
        assert_eq!(schema["type"], "array");
        assert_eq!(schema["items"]["type"], "string");

        // GTS types - now inlined with x-gts-ref extension
        let (req, schema) = rust_type_to_json_schema("GtsInstanceId");
        assert!(req);
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["format"], "gts-instance-id");
        assert_eq!(schema["x-gts-ref"], "gts.*");

        // Generic type parameter
        let (req, schema) = rust_type_to_json_schema("P");
        assert!(req);
        assert_eq!(schema["type"], "object");
    }
}
