# GTS Rust Implementation

A complete Rust implementation of the Global Type System (GTS)

## Overview

GTS (Global Type System)[https://github.com/globaltypesystem/gts-spec] is a simple, human-readable, globally unique identifier and referencing system for data type definitions (e.g., JSON Schemas) and data instances (e.g., JSON objects). This Rust implementation provides high-performance, type-safe operations for working with GTS identifiers.

## Roadmap

Featureset:

- [x] **OP#1 - ID Validation**: Verify identifier syntax using regex patterns
- [x] **OP#2 - ID Extraction**: Fetch identifiers from JSON objects or JSON Schema documents
- [x] **OP#3 - ID Parsing**: Decompose identifiers into constituent parts (vendor, package, namespace, type, version, etc.)
- [x] **OP#4 - ID Pattern Matching**: Match identifiers against patterns containing wildcards
- [x] **OP#5 - ID to UUID Mapping**: Generate deterministic UUIDs from GTS identifiers
- [x] **OP#6 - Schema Validation**: Validate object instances against their corresponding schemas
- [x] **OP#7 - Relationship Resolution**: Load all schemas and instances, resolve inter-dependencies, and detect broken references
- [x] **OP#8 - Compatibility Checking**: Verify that schemas with different MINOR versions are compatible
- [x] **OP#8.1 - Backward compatibility checking**
- [x] **OP#8.2 - Forward compatibility checking**
- [x] **OP#8.3 - Full compatibility checking**
- [x] **OP#9 - Version Casting**: Transform instances between compatible MINOR versions
- [x] **OP#10 - Query Execution**: Filter identifier collections using the GTS query language
- [x] **OP#11 - Attribute Access**: Retrieve property values and metadata using the attribute selector (`@`)

See details in [gts/README.md](gts/README.md)

Other GTS spec [Reference Implementation](https://github.com/globaltypesystem/gts-spec/blob/main/README.md#9-reference-implementation-recommendations) recommended features support:

- [ ] **In-memory entities registry** - simple GTS entities registry with optional GTS references validation on entity registration
- [x] **CLI** - command-line interface for all GTS operations
- [x] **Web server** - a non-production web-server with REST API for the operations processing and testing
- [ ] **x-gts-ref** - to support special GTS entity reference annotation in schemas
- [x] **YAML support** - to support YAML files (*.yml, *.yaml) as input files
- [ ] **TypeSpec support** - add [typespec.io](https://typespec.io/) files (*.tsp) support
- [ ] **UUID for instances** - to support UUID as ID in JSON instances

Rust-specific features:
- [x] Generate GTS schemas from Rust source code, see [gts-macros/README.md](gts-macros/README.md) and [gts-macros-test/README.md](gts-macros-test/README.md)
- [ ] Automatically refer to GTS schemas for referenced objects

Technical Backlog:

- [x] **Code coverage** - target is 90%
- [ ] **Documentation** - add documentation for all the features
- [ ] **Interface** - export publicly available interface and keep cli and others private
- [ ] **Server API** - finalise the server API
- [ ] **Final code cleanup** - remove unused code, denormalize, add critical comments, etc.


## Architecture

The project is organized as a Cargo workspace with two crates:

### `gts` (Library Crate)

Core library providing all GTS functionality:

- **gts.rs** - GTS ID parsing, validation, wildcard matching
- **entities.rs** - JSON entities, configuration, validation
- **path_resolver.rs** - JSON path resolution
- **schema_cast.rs** - Schema compatibility and casting
- **files_reader.rs** - File system scanning
- **store.rs** - Entity storage and querying
- **ops.rs** - High-level operations API

### `gts-cli` (Binary Crate)

Command-line tool and HTTP server:

- **cli.rs** - Full CLI with all commands
- **gen_schemas.rs** - GTS schema generation from Rust source code
- **server.rs** - Axum-based HTTP server
- **main.rs** - Entry point

## Installation

### From Source

```bash
git clone https://github.com/globaltypesystem/gts-rust
cd gts-rust
cargo build --release
```

The binary will be available at `target/release/gts`.

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
gts = { path = "path/to/gts-rust/gts" }
```

## Usage

### CLI Commands

All CLI commands support `--path` to specify data directories and `--config` for custom configuration.

#### OP#1 - ID Validation

Verify that a GTS identifier follows the correct syntax.

```bash
# Validate a schema ID
gts validate-id --gts-id "gts.x.core.events.event.v1~"

# Validate an instance ID
gts validate-id --gts-id "gts.x.core.events.event.v1.0"

# Validate a chained ID
gts validate-id --gts-id "gts.x.core.events.event.v1~vendor.app._.custom.v2~"

# Invalid ID example
gts validate-id --gts-id "invalid-id"
```

**Output:**
```json
{
  "id": "gts.x.core.events.event.v1~",
  "valid": true,
  "error": ""
}
```

#### OP#2 - ID Extraction

Extract GTS identifiers from JSON objects. This happens automatically when loading files.

```bash
# List all entities (extracts IDs from all JSON/YAML files)
gts --path ./examples list --limit 10
```

#### OP#3 - ID Parsing

Decompose a GTS identifier into its constituent parts.

```bash
# Parse a simple schema ID
gts parse-id --gts-id "gts.x.core.events.event.v1~"

# Parse an instance ID with minor version
gts parse-id --gts-id "gts.vendor.package.namespace.type.v2.5"

# Parse a chained ID
gts parse-id --gts-id "gts.x.core.events.event.v1~vendor.app._.custom.v2~"
```

**Output:**
```json
{
  "id": "gts.x.core.events.event.v1~",
  "ok": true,
  "segments": [
    {
      "vendor": "x",
      "package": "core",
      "namespace": "events",
      "type": "event",
      "ver_major": 1,
      "ver_minor": null,
      "is_type": true
    }
  ],
  "error": ""
}
```

#### OP#4 - ID Pattern Matching

Match identifiers against patterns with wildcards.

```bash
# Match with wildcard namespace
gts match-id-pattern --pattern "gts.x.core.*" --candidate "gts.x.core.events.event.v1~"

# Match specific version range
gts match-id-pattern --pattern "gts.x.*.events.*.v1~" --candidate "gts.x.core.events.event.v1~"

# No match example
gts match-id-pattern --pattern "gts.vendor.*" --candidate "gts.x.core.events.event.v1~"
```

**Output:**
```json
{
  "candidate": "gts.x.core.events.event.v1~",
  "pattern": "gts.x.core.*",
  "match": true
}
```

#### OP#5 - ID to UUID Mapping

Generate deterministic UUIDs from GTS identifiers.

```bash
# Generate UUID with major version scope (default)
gts uuid --gts-id "gts.x.core.events.event.v1~"

# Generate UUID with minor version scope
gts uuid --gts-id "gts.x.core.events.event.v1.0" --scope minor

# Same major version produces same UUID
gts uuid --gts-id "gts.x.core.events.event.v1.5" --scope major
```

**Output:**
```json
{
  "id": "gts.x.core.events.event.v1~",
  "uuid": "a3d5e8f1-2b4c-5d6e-8f9a-1b2c3d4e5f6a"
}
```

#### OP#6 - Schema Validation

Validate object instances against their corresponding schemas.

```bash
# Validate a single instance
gts --path ./examples validate-instance --gts-id "gts.x.core.events.event.v1.0"

# The system:
# 1. Loads the instance by ID
# 2. Finds its schema (via $schema or type field)
# 3. Validates using JSON Schema validation
```

**Output:**
```json
{
  "id": "gts.x.core.events.event.v1.0",
  "ok": true
}
```

#### OP#7 - Relationship Resolution

Load all schemas and instances, resolve inter-dependencies, and detect broken references.

```bash
# Resolve relationships for an entity
gts --path ./examples resolve-relationships --gts-id "gts.x.core.events.event.v1.0"

# The system:
# 1. Loads the entity
# 2. Extracts all GTS ID references ($ref, nested IDs)
# 3. Resolves each reference
# 4. Reports missing or broken references
```

**Output:**
```json
{
  "id": "gts.x.core.events.event.v1.0",
  "ok": true,
  "refs": [
    "gts.x.core.events.event.v1~",
    "gts.x.core.models.user.v1~"
  ],
  "missing_refs": [],
  "error": ""
}
```

#### OP#8 - Compatibility Checking

Verify that schemas with different MINOR versions are compatible.

```bash
# Check backward compatibility (v1 -> v2)
gts --path ./examples compatibility --old-schema-id "gts.x.core.events.event.v1.1~" \
                  --new-schema-id "gts.x.core.events.event.v1.2~"

# The system checks:
# - OP#8.1: Backward compatibility (old instances work with new schema)
# - OP#8.2: Forward compatibility (new instances work with old schema)
# - OP#8.3: Full compatibility (both directions compatible)
```

**Output:**
```json
{
  "old_schema_id": "gts.x.core.events.event.v1.1~",
  "new_schema_id": "gts.x.core.events.event.v1.2~",
  "is_backward_compatible": true,
  "is_forward_compatible": false,
  "is_fully_compatible": false,
  "backward_errors": [],
  "forward_errors": [
    "Added required properties: email"
  ]
}
```

#### OP#9 - Version Casting

Transform instances between compatible MINOR versions.

```bash
# Cast instance from v1.0 to v2 schema
gts --path ./examples cast --from-id "gts.x.core.events.event.v1.1~" \
         --to-schema-id "gts.x.core.events.event.v1.2~"

# The system:
# 1. Loads source instance and both schemas
# 2. Checks compatibility
# 3. Applies transformations (adds defaults, removes extra fields, updates const values)
# 4. Returns transformed instance
```

**Output:**
```json
{
  "from": "gts.x.core.events.event.v1.0",
  "to": "gts.x.core.events.event.v1.2~",
  "direction": "up",
  "is_backward_compatible": true,
  "is_forward_compatible": false,
  "added_properties": ["region"],
  "removed_properties": [],
  "casted_entity": {
    "gtsId": "gts.x.core.events.event.v1.0",
    "name": "example",
    "region": "us-east"
  }
}
```

#### OP#10 - Query Execution

Filter identifier collections using the GTS query language.

```bash
# Query with wildcard pattern
gts --path ./examples query --expr "gts.x.core.events.*" --limit 50

# Query with attribute filter
gts --path ./examples query --expr "gts.x.core.events.*[status=active]" --limit 50

# Query schemas only (ending with ~)
gts --path ./examples query --expr "gts.x.*.*.*.v1~" --limit 100

# Query specific namespace
gts --path ./examples query --expr "gts.vendor.package.namespace.*" --limit 20
```

**Output:**
```json
{
  "error": "",
  "count": 3,
  "limit": 50,
  "results": [
    {"gtsId": "gts.x.core.events.event.v1~", ...},
    {"gtsId": "gts.x.core.events.event.v1.0", ...},
    {"gtsId": "gts.x.core.events.topic.v1~", ...}
  ]
}
```

#### OP#11 - Attribute Access

Retrieve property values and metadata using the attribute selector (`@`).

```bash
# Access top-level property
gts --path ./examples attr --gts-with-path "gts.x.core.events.event.v1.0@name"

# Access nested property
gts --path ./examples attr --gts-with-path "gts.x.core.events.event.v1.0@metadata.timestamp"

# Access array element
gts --path ./examples attr --gts-with-path "gts.x.core.events.event.v1.0@tags[0]"

# Access schema property
gts --path ./examples attr --gts-with-path "gts.x.core.events.event.v1~@properties.name.type"
```

**Output:**
```json
{
  "id": "gts.x.core.events.event.v1.0",
  "path": "metadata.timestamp",
  "value": "2025-11-09T23:00:00Z",
  "ok": true
}
```

#### Additional Commands

**List Entities:**
```bash
gts --path ./examples list --limit 100
```

**Start HTTP Server:**
```bash
# Start server without HTTP logging (WARNING level only)
gts --path ./examples server --host 127.0.0.1 --port 8000
# CURL: curl http://127.0.0.1:8000/entities | jq .

# Start server with HTTP request logging (-v or --verbose)
gts -v --path ./examples server --host 127.0.0.1 --port 8000

# Start server with detailed logging including request/response bodies (-vv)
gts -vv --path ./examples server --host 127.0.0.1 --port 8000
```

Verbose logging format:
- **No flag**: WARNING level only (no HTTP request logs)
- **`-v`**: INFO level - Logs HTTP requests with color-coded output
- **`-vv`**: DEBUG level - Additionally logs request/response bodies with pretty-printed JSON

**Generate OpenAPI Spec:**
```bash
gts openapi-spec --out openapi.json --host 127.0.0.1 --port 8000
```

### Library Usage

All operations are available through the `GtsOps` API.

#### Setup

```rust
use gts::{GtsID, GtsOps, GtsConfig, GtsWildcard};
use serde_json::json;

// Initialize GTS operations with data paths
let mut ops = GtsOps::new(
    Some(vec!["./data".to_string(), "./schemas".to_string()]),
    None,  // Optional config file path
    0      // Verbosity level
);
```

#### OP#1 - ID Validation

```rust
// Validate a GTS ID
let result = ops.validate_id("gts.x.core.events.event.v1~");
assert!(result.valid);

// Validate invalid ID
let result = ops.validate_id("invalid-id");
assert!(!result.valid);
assert!(!result.error.is_empty());

// Direct validation without ops
let is_valid = GtsID::is_valid("gts.x.core.events.event.v1~");
assert!(is_valid);
```

#### OP#2 - ID Extraction

```rust
// ID extraction happens automatically when loading entities
// Configure which fields to check for IDs:
let config = GtsConfig {
    entity_id_fields: vec![
        "$id".to_string(),
        "gtsId".to_string(),
        "id".to_string(),
    ],
    schema_id_fields: vec![
        "$schema".to_string(),
        "type".to_string(),
    ],
};

// Load entities (IDs extracted automatically)
let results = ops.list(100);
for entity in results.results {
    if let Some(id) = entity.get("gtsId") {
        println!("Found ID: {}", id);
    }
}
```

#### OP#3 - ID Parsing

```rust
// Parse a GTS ID into components
let result = ops.parse_id("gts.x.core.events.event.v1.2~");
assert!(result.ok);
assert_eq!(result.segments.len(), 1);

let segment = &result.segments[0];
assert_eq!(segment.vendor, "x");
assert_eq!(segment.package, "core");
assert_eq!(segment.namespace, "events");
assert_eq!(segment.type_name, "event");
assert_eq!(segment.ver_major, Some(1));
assert_eq!(segment.ver_minor, Some(2));
assert!(segment.is_type);

// Parse chained ID
let result = ops.parse_id("gts.x.core.events.event.v1~vendor.app._.custom.v2~");
assert_eq!(result.segments.len(), 2);

// Direct parsing
let id = GtsID::new("gts.x.core.events.event.v1~")?;
assert_eq!(id.gts_id_segments.len(), 1);
```

#### OP#4 - ID Pattern Matching

```rust
// Match ID against wildcard pattern
let result = ops.match_id_pattern(
    "gts.x.core.*",
    "gts.x.core.events.event.v1~"
);
assert!(result.is_match);

// No match
let result = ops.match_id_pattern(
    "gts.vendor.*",
    "gts.x.core.events.event.v1~"
);
assert!(!result.is_match);

// Direct wildcard matching
let pattern = GtsWildcard::new("gts.x.*.events.*")?;
let id = GtsID::new("gts.x.core.events.event.v1~")?;
assert!(pattern.matches(&id));
```

#### OP#5 - ID to UUID Mapping

```rust
// Generate UUID from GTS ID
let result = ops.uuid("gts.x.core.events.event.v1~", "major");
assert!(!result.uuid.is_empty());

// Minor scope UUID
let result = ops.uuid("gts.x.core.events.event.v1.0", "minor");

// Direct UUID generation
let id = GtsID::new("gts.x.core.events.event.v1~")?;
let uuid = id.to_uuid();
println!("UUID: {}", uuid);

// Same major version produces same UUID
let id1 = GtsID::new("gts.x.core.events.event.v1.0")?;
let id2 = GtsID::new("gts.x.core.events.event.v1.5")?;
assert_eq!(id1.to_uuid(), id2.to_uuid());
```

#### OP#6 - Schema Validation

```rust
// Validate instance against its schema
let result = ops.validate_instance("gts.x.core.events.event.v1.0");
assert!(result.ok);

if !result.ok {
    println!("Validation error: {}", result.error);
}

// The system automatically:
// 1. Loads the instance
// 2. Finds its schema (via $schema or type field)
// 3. Validates using JSON Schema
```

#### OP#7 - Relationship Resolution

```rust
// Resolve all references for an entity
let result = ops.resolve_relationships("gts.x.core.events.event.v1.0");
assert!(result.ok);

// Check for broken references
if !result.missing_refs.is_empty() {
    println!("Missing references:");
    for ref_id in result.missing_refs {
        println!("  - {}", ref_id);
    }
}

// List all references
for ref_id in result.refs {
    println!("Reference: {}", ref_id);
}
```

#### OP#8 - Compatibility Checking

```rust
// Check schema compatibility
let result = ops.compatibility(
    "gts.x.core.events.event.v1~",
    "gts.x.core.events.event.v2~"
);

// OP#8.1 - Backward compatibility
if result.is_backward_compatible {
    println!("Old instances work with new schema");
} else {
    println!("Backward incompatible:");
    for error in result.backward_errors {
        println!("  - {}", error);
    }
}

// OP#8.2 - Forward compatibility
if result.is_forward_compatible {
    println!("New instances work with old schema");
} else {
    println!("Forward incompatible:");
    for error in result.forward_errors {
        println!("  - {}", error);
    }
}

// OP#8.3 - Full compatibility
if result.is_fully_compatible {
    println!("Fully compatible in both directions");
}
```

#### OP#9 - Version Casting

```rust
// Cast instance to new schema version
let result = ops.cast(
    "gts.x.core.events.event.v1.0",
    "gts.x.core.events.event.v2~"
);

assert!(result.ok);

// Check what changed
println!("Direction: {}", result.direction);
println!("Added properties: {:?}", result.added_properties);
println!("Removed properties: {:?}", result.removed_properties);

// Get the transformed entity
if let Some(casted) = result.casted_entity {
    println!("Casted entity: {}", serde_json::to_string_pretty(&casted)?);
}

// Check compatibility
if !result.is_backward_compatible {
    println!("Warning: Not backward compatible");
    for reason in result.incompatibility_reasons {
        println!("  - {}", reason);
    }
}
```

#### OP#10 - Query Execution

```rust
// Query with wildcard pattern
let results = ops.query("gts.x.core.events.*", 50);
println!("Found {} entities", results.count);

for entity in results.results {
    if let Some(id) = entity.get("gtsId") {
        println!("  - {}", id);
    }
}

// Query with attribute filter
let results = ops.query("gts.x.core.events.*[status=active]", 100);

// Query schemas only
let results = ops.query("gts.x.*.*.*.v1~", 100);

// List all entities
let results = ops.list(1000);
```

#### OP#11 - Attribute Access

```rust
// Access entity attribute
let result = ops.attr("gts.x.core.events.event.v1.0@name");
assert!(result.ok);
println!("Name: {}", result.value);

// Access nested property
let result = ops.attr("gts.x.core.events.event.v1.0@metadata.timestamp");

// Access array element
let result = ops.attr("gts.x.core.events.event.v1.0@tags[0]");

// Access schema property
let result = ops.attr("gts.x.core.events.event.v1~@properties.name.type");
assert_eq!(result.value.as_str(), Some("string"));

// Handle missing attributes
if !result.ok {
    println!("Attribute not found: {}", result.error);
}
```

#### Complete Example

```rust
use gts::GtsOps;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize
    let mut ops = GtsOps::new(
        Some(vec!["./data".to_string()]),
        None,
        0
    );

    // OP#1: Validate ID
    let validation = ops.validate_id("gts.x.core.events.event.v1~");
    assert!(validation.valid);

    // OP#3: Parse ID
    let parsed = ops.parse_id("gts.x.core.events.event.v1.2~");
    println!("Vendor: {}", parsed.segments[0].vendor);

    // OP#5: Generate UUID
    let uuid_result = ops.uuid("gts.x.core.events.event.v1~", "major");
    println!("UUID: {}", uuid_result.uuid);

    // OP#6: Validate instance
    let validation = ops.validate_instance("gts.x.core.events.event.v1.0");
    if validation.ok {
        println!("Instance is valid");
    }

    // OP#8: Check compatibility
    let compat = ops.compatibility(
        "gts.x.core.events.event.v1~",
        "gts.x.core.events.event.v2~"
    );
    println!("Backward compatible: {}", compat.is_backward_compatible);

    // OP#9: Cast instance
    let cast = ops.cast(
        "gts.x.core.events.event.v1.0",
        "gts.x.core.events.event.v2~"
    );
    if let Some(casted) = cast.casted_entity {
        println!("Casted: {}", serde_json::to_string_pretty(&casted)?);
    }

    // OP#10: Query entities
    let results = ops.query("gts.x.core.*", 100);
    println!("Found {} entities", results.count);

    // OP#11: Access attribute
    let attr = ops.attr("gts.x.core.events.event.v1.0@name");
    println!("Name: {}", attr.value);

    Ok(())
}
```

### HTTP API

Start the server:

```bash
gts --path ./examples server --host 127.0.0.1 --port 8000
# curl http://localhost:8000/entities | jq .
```

Example API calls:

```bash
# Validate ID
curl "http://localhost:8000/validate-id?gts_id=gts.x.core.events.event.v1~"

# Parse ID
curl "http://localhost:8000/parse-id?gts_id=gts.x.core.events.event.v1.2~"

# Query entities
curl "http://localhost:8000/query?expr=gts.x.core.*&limit=10"

# Add entity
curl -X POST http://localhost:8000/entities \
  -H "Content-Type: application/json" \
  -d '{"gtsId": "gts.x.core.events.event.v1.0", "data": "..."}'
```

## Configuration

Create a `gts.config.json` file to customize entity ID field detection:

```json
{
  "entity_id_fields": [
    "$id",
    "gtsId",
    "gtsIid",
    "gtsOid",
    "gtsI",
    "gts_id",
    "gts_oid",
    "gts_iid",
    "id"
  ],
  "schema_id_fields": [
    "$schema",
    "gtsTid",
    "gtsType",
    "gtsT",
    "gts_t",
    "gts_tid",
    "gts_type",
    "type",
    "schema"
  ]
}
```

## GTS ID Format

GTS identifiers follow this format:

```
gts.<vendor>.<package>.<namespace>.<type>.v<MAJOR>[.<MINOR>][~]
```

- **Prefix**: Always starts with `gts.`
- **Vendor**: Organization or vendor code
- **Package**: Module or application name
- **Namespace**: Category within the package
- **Type**: Specific type name
- **Version**: Semantic version (major.minor)
- **Type Marker**: Trailing `~` indicates a schema/type (vs instance)

Examples:
- `gts.x.core.events.event.v1~` - Schema
- `gts.x.core.events.event.v1.0` - Instance
- `gts.x.core.events.type.v1~vendor.app._.custom.v1~` - Chained (inheritance)

## Testing

Run the test suite:

```bash
cargo test
```

Run with verbose output:

```bash
cargo test -- --nocapture
```

## Development

### Build

```bash
cargo build
```

### Build Release

```bash
cargo build --release
```

### Run Tests

```bash
cargo test
```

### Format Code

```bash
cargo fmt
```

### Lint

```bash
cargo clippy
```

## License

Apache-2.0

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Links

- [GTS Specification](https://github.com/globaltypesystem/gts-spec)
- [Python Implementation](https://github.com/globaltypesystem/gts-python)
- [Documentation](https://docs.rs/gts)

## Acknowledgments

This Rust implementation is based on the Python reference implementation and follows the GTS specification v0.4.
