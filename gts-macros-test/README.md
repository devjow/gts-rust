# GTS Macros Test

Example crate demonstrating GTS schema generation from Rust structs using the `#[struct_to_gts_schema]` macro and the `gts` CLI tool.

## Quick Start

```bash
# Generate schemas using paths from macro (relative to source files)
gts generate-from-rust --source gts-macros-test/src

# Or override output directory
gts generate-from-rust --source gts-macros-test/src --output gts-macros-test/schemas

# Or using cargo
cargo run --bin gts -- generate-from-rust --source gts-macros-test/src

# View generated schemas
find gts-macros-test -name "*.schema.json"
```

## Overview

This crate demonstrates the idiomatic Rust way to generate GTS schemas:

1. **Annotate** your structs with `#[struct_to_gts_schema(...)]`
2. **Compile** to validate annotations (compile-time checks)
3. **Generate** schemas using the `gts` CLI tool

## Example Structs

### User Entity

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[struct_to_gts_schema(
    file_path = "schemas/gts.x.test.entities.user.v1~.schema.json",
    schema_id = "gts.x.test.entities.user.v1~",
    description = "User entity with basic information",
    properties = "id,email,name,age"
)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub age: u32,
    pub internal_data: Option<String>,  // Not in schema
}
```

### Product Entity

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[struct_to_gts_schema(
    file_path = "schemas/gts.x.test.entities.product.v1~.schema.json",
    schema_id = "gts.x.test.entities.product.v1~",
    description = "Product entity with pricing information",
    properties = "id,name,price,description,in_stock"
)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub description: Option<String>,
    pub in_stock: bool,
    pub warehouse_location: String,  // Not in schema
}
```

## Workflow

### 1. Define Your Structs

Add the `#[struct_to_gts_schema]` annotation to your structs in `src/lib.rs` or any Rust file.

### 2. Compile (Validates Annotations)

```bash
cargo build --package gts-macros-test
```

This validates that:
- All required parameters are present
- All specified properties exist in the struct
- The struct has named fields

### 3. Generate Schemas

```bash
# Option A: Use paths from macro (relative to source files)
gts generate-from-rust --source gts-macros-test/src

# Option B: Override output directory
gts generate-from-rust --source gts-macros-test/src --output gts-macros-test/schemas
```

Output:
```
Scanning Rust source files in: gts-macros-test/src
  ✓ Generated 2 schema(s) from gts-macros-test/src/lib.rs

Summary:
  Files scanned: 1
  Schemas generated: 2
```

**Path behavior:**
- Without `--output`: Uses `file_path` from macro, relative to each source file
- With `--output`: Uses `--output` as base directory, appends `file_path` from macro

### 4. Use the Schemas

```bash
# Validate with GTS CLI
gts validate-id --gts-id "gts.x.test.entities.user.v1~"

# List schemas
gts --path gts-macros-test/schemas list

# Query schemas
gts --path gts-macros-test/schemas query --expr "gts.x.test.*"
```

## Generated Schemas

### User Schema

`schemas/gts.x.test.entities.user.v1~.schema.json`:

```json
{
  "$id": "gts.x.test.entities.user.v1~",
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "User",
  "type": "object",
  "description": "User entity with basic information",
  "properties": {
    "id": { "type": "string" },
    "email": { "type": "string" },
    "name": { "type": "string" },
    "age": { "type": "integer" }
  },
  "required": ["id", "email", "name", "age"]
}
```

### Product Schema

`schemas/gts.x.test.entities.product.v1~.schema.json`:

```json
{
  "$id": "gts.x.test.entities.product.v1~",
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Product",
  "type": "object",
  "description": "Product entity with pricing information",
  "properties": {
    "id": { "type": "string" },
    "name": { "type": "string" },
    "price": { "type": "number" },
    "description": { "type": "string" },
    "in_stock": { "type": "boolean" }
  },
  "required": ["id", "name", "price", "in_stock"]
}
```

Note: `description` is not in the `required` array because it's defined as `Option<String>`.

## Testing

Run the test suite:

```bash
cargo test --package gts-macros-test
```

Tests verify:
- Struct serialization works correctly
- Metadata is embedded properly
- All fields serialize as expected

## Key Features Demonstrated

1. **Compile-Time Validation**: Invalid annotations cause compilation errors
2. **Selective Properties**: Only specified fields are included in schemas
3. **Optional Fields**: `Option<T>` fields are not marked as required
4. **Type Mapping**: Automatic Rust → JSON Schema type conversion
5. **Multiple Structs**: Multiple annotated structs in the same file

## Using as a Template

To use this as a template for your own project:

1. Copy the structure:
```bash
cp -r gts-macros-test my-project
```

2. Update `Cargo.toml` with your project name

3. Modify structs in `src/lib.rs` with your entities

4. Update GTS IDs to match your vendor/package/namespace

5. Generate schemas:
```bash
gts generate-from-rust --source my-project/src
```

6. Generate schemas to specific location:
```bash
gts generate-from-rust --source my-project/src --output my-project/schemas
```

## Common Patterns

### Excluding Internal Fields

```rust
#[struct_to_gts_schema(
    file_path = "schemas/gts.x.app.user.v1~.schema.json",
    schema_id = "gts.x.app.entities.user.v1~",
    description = "User",
    properties = "id,email"  // Don't include password_hash
)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,  // Internal only
}
```

### Optional Fields

```rust
#[struct_to_gts_schema(
    file_path = "schemas/gts.x.app.profile.v1~.schema.json",
    schema_id = "gts.x.app.entities.profile.v1~",
    description = "User profile",
    properties = "name,bio,avatar_url"
)]
pub struct Profile {
    pub name: String,
    pub bio: Option<String>,      // Optional
    pub avatar_url: Option<String>, // Optional
}
```

Generated schema will have `required: ["name"]` only.

## Documentation

- **Macro Documentation**: [../gts-macros/README.md](../gts-macros/README.md)
- **Main README**: [../README.md](../README.md)
- **Quick Start**: [QUICK_START.md](QUICK_START.md)

## License

Apache-2.0
