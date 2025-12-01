use anyhow::{bail, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Generate GTS schemas from Rust source code with #[struct_to_gts_schema] annotations
pub fn generate_schemas_from_rust(source: &str, output: Option<&str>) -> Result<()> {
    println!("Scanning Rust source files in: {}", source);

    let source_path = Path::new(source);
    if !source_path.exists() {
        bail!("Source path does not exist: {}", source);
    }

    // Canonicalize source path to detect path traversal attempts
    let source_canonical = source_path.canonicalize()?;

    let mut schemas_generated = 0;
    let mut files_scanned = 0;

    // Walk through all .rs files
    for entry in WalkDir::new(source_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            files_scanned += 1;
            if let Ok(content) = fs::read_to_string(path) {
                // Parse the file and extract schema information
                let results = extract_and_generate_schemas(
                    &content,
                    output,
                    &source_canonical,
                    path,
                )?;
                schemas_generated += results.len();
                for (schema_id, file_path) in results {
                    println!("  Generated schema: {} @ {}", schema_id, file_path);
                }
            }
        }
    }

    println!("\nSummary:");
    println!("  Files scanned: {}", files_scanned);
    println!("  Schemas generated: {}", schemas_generated);

    if schemas_generated == 0 {
        println!("\n- No schemas found. Make sure your structs are annotated with #[struct_to_gts_schema(...)]");
    }

    Ok(())
}

/// Extract schema metadata from Rust source and generate JSON files
/// Returns a vector of (schema_id, file_path) tuples for each generated schema
fn extract_and_generate_schemas(
    content: &str,
    output_override: Option<&str>,
    source_root: &Path,
    source_file: &Path,
) -> Result<Vec<(String, String)>> {
    // Regex to find struct_to_gts_schema annotations
    let re = Regex::new(
        r#"(?s)#\[struct_to_gts_schema\(\s*file_path\s*=\s*"([^"]+)"\s*,\s*schema_id\s*=\s*"([^"]+)"\s*,\s*description\s*=\s*"([^"]+)"\s*,\s*properties\s*=\s*"([^"]+)"\s*\)\]\s*(?:pub\s+)?struct\s+(\w+)\s*\{([^}]+)\}"#
    )?;

    let mut results = Vec::new();

    for cap in re.captures_iter(content) {
        let file_path = &cap[1];
        let schema_id = &cap[2];
        let description = &cap[3];
        let properties_str = &cap[4];
        let struct_name = &cap[5];
        let struct_body = &cap[6];

        // Validate file_path ends with .json
        if !file_path.ends_with(".json") {
            bail!(
                "Invalid file_path in {}:{} - file_path must end with '.json': {}",
                source_file.display(),
                struct_name,
                file_path
            );
        }

        // Determine output path
        let output_path = if let Some(output_dir) = output_override {
            // Use CLI-provided output directory
            Path::new(output_dir).join(file_path)
        } else {
            // Use path from macro (relative to source file's directory)
            let source_dir = source_file.parent().unwrap_or(source_root);
            source_dir.join(file_path)
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
                "Security error in {}:{} - file_path '{}' attempts to write outside source repository. \
                Resolved to: {}, but must be within: {}",
                source_file.display(),
                struct_name,
                file_path,
                output_canonical.display(),
                source_root.display()
            );
        }

        // Parse properties
        let properties: Vec<&str> = properties_str.split(',').map(|s| s.trim()).collect();

        // Parse struct fields
        let field_re = Regex::new(r"(?m)^\s*(?:pub\s+)?(\w+)\s*:\s*([^,]+?)(?:,|\s*$)")?;
        let mut field_types = HashMap::new();

        for field_cap in field_re.captures_iter(struct_body) {
            let field_name = &field_cap[1];
            let field_type = field_cap[2].trim();
            field_types.insert(field_name.to_string(), field_type.to_string());
        }

        // Build JSON schema
        let schema = build_json_schema(schema_id, struct_name, description, &properties, &field_types)?;

        // Create parent directories
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write schema file
        fs::write(&output_path, serde_json::to_string_pretty(&schema)?)?;

        // Add to results (schema_id, file_path)
        results.push((schema_id.to_string(), output_path.display().to_string()));
    }

    Ok(results)
}

/// Build a JSON Schema object from parsed metadata
fn build_json_schema(
    schema_id: &str,
    struct_name: &str,
    description: &str,
    properties: &[&str],
    field_types: &HashMap<String, String>,
) -> Result<serde_json::Value> {
    use serde_json::json;

    let mut schema_properties = serde_json::Map::new();
    let mut required = Vec::new();

    for prop in properties {
        if let Some(field_type) = field_types.get(*prop) {
            let (is_required, json_type, format) = rust_type_to_json_schema_type(field_type);

            let mut prop_schema = json!({ "type": json_type });
            if let Some(fmt) = format {
                prop_schema["format"] = json!(fmt);
            }

            schema_properties.insert(prop.to_string(), prop_schema);

            if is_required {
                required.push(prop.to_string());
            }
        }
    }

    let mut schema = json!({
        "$id": schema_id,
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": struct_name,
        "type": "object",
        "description": description,
        "properties": schema_properties
    });

    if !required.is_empty() {
        schema["required"] = json!(required);
    }

    Ok(schema)
}

/// Convert Rust type string to JSON Schema type
fn rust_type_to_json_schema_type(rust_type: &str) -> (bool, &'static str, Option<&'static str>) {
    let rust_type = rust_type.trim();

    // Check if it's an Option type
    let is_optional = rust_type.starts_with("Option<");
    let inner_type = if is_optional {
        rust_type
            .strip_prefix("Option<")
            .and_then(|s| s.strip_suffix(">"))
            .unwrap_or(rust_type)
            .trim()
    } else {
        rust_type
    };

    let (json_type, format) = match inner_type {
        "String" | "str" | "&str" => ("string", None),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => ("integer", None),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => ("integer", None),
        "f32" | "f64" => ("number", None),
        "bool" => ("boolean", None),
        t if t.starts_with("Vec<") => ("array", None),
        t if t.contains("Uuid") || t.contains("uuid") => ("string", Some("uuid")),
        _ => ("string", None), // Default to string
    };

    (!is_optional, json_type, format)
}
