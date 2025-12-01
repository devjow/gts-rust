# GTS Macros

Compile-time validation for GTS (Global Type System) schema generation from Rust structs.

## Overview

This crate provides the `#[struct_to_gts_schema]` attribute macro that validates your struct annotations at compile time. The macro ensures all required parameters are present and all specified properties exist in your struct.

**Important**: This macro performs **validation only**. It does not generate schema files. Use the `gts` CLI tool to actually generate the JSON Schema files from the source code repository.

To generate GTS schema specs from Rust code:
- Annotate your structs with the `#[struct_to_gts_schema(...)]` attribute from the `gts-macros` crate (validation-only; no files are written at compile time).
- Run the CLI over your Rust sources: `gts generate-from-rust --source path/to/your/src [--output path/to/output]`.
- The command will emit one JSON Schema file per annotated struct and print each generated **GTS ID @ file path** to stdout.

## Features

- ✅ **Compile-time validation**: Catches errors before runtime
- ✅ **Required parameter checking**: Ensures file_path, schema_id, description, and properties are all provided
- ✅ **Property existence validation**: Verifies all listed properties exist in the struct
- ✅ **Struct type validation**: Ensures only structs with named fields are annotated
- ✅ **File extension validation**: Ensures file_path ends with `.json`
- ✅ **Metadata embedding**: Stores schema information as constants for tooling access

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
gts-macros = { path = "path/to/gts-rust/gts-macros" }
serde = { version = "1.0", features = ["derive"] }
```

## Usage

### 1. Annotate Your Structs

```rust
use gts_macros::struct_to_gts_schema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[struct_to_gts_schema(
    file_path = "schemas/gts.x.myapp.entities.user.v1~.schema.json",
    schema_id = "gts.x.myapp.entities.user.v1~",
    description = "User entity with authentication information",
    properties = "id,email,name"
)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub internal_field: i32,  // Not included in schema
}
```

### 2. Generate Schemas

Use the GTS CLI tool to generate the actual JSON Schema files:

```bash
# Generate schemas using paths from macro (relative to source files)
gts generate-from-rust --source src/

# Or override output directory
gts generate-from-rust --source src/ --output schemas/

# Or with cargo
cargo run --bin gts -- generate-from-rust --source src/
```

The CLI tool will:
- Scan your source files for `#[struct_to_gts_schema]` annotations
- Extract the metadata
- Generate valid GTS JSON Schema files
- If `--output` is not specified, use the `file_path` from the macro (relative to each source file)
- If `--output` is specified, use it as the base directory and append the `file_path` from the macro

## Macro Parameters

All parameters are **required**:

| Parameter | Description | Example | Validation |
|-----------|-------------|---------|------------|
| `file_path` | Output path relative to source file | `"schemas/gts.x.core.iam.user.v1~.schema.json"` | Must end with `.json` |
| `schema_id` | GTS identifier | `"gts.x.app.entities.user.v1~"` | - |
| `description` | Human-readable description | `"User entity"` | - |
| `properties` | Comma-separated field list | `"id,email,name"` | All must exist in struct |

### File Path Rules

1. **Relative paths**: The `file_path` is relative to the source file containing the struct
2. **Must end with `.json`**: Compilation fails if it doesn't
3. **Security**: Cannot reference paths outside the source repository (e.g., `../../etc/passwd`)
4. **Examples**:
   - ✅ `"schemas/user.v1~.schema.json"` - Creates `src/schemas/user.v1~.schema.json`
   - ✅ `"../schemas/user.v1~.schema.json"` - Creates `schemas/user.v1~.schema.json` (one level up)
   - ❌ `"schemas/user.schema"` - Compilation error: must end with `.json`
   - ❌ `"../../../etc/passwd.json"` - CLI error: path escapes repository

### GTS ID Format

GTS identifiers must follow this format:

```
gts.<vendor>.<package>.<namespace>.<type>.v<MAJOR>[.<MINOR>]~
```

Examples:
- `gts.x.core.iam.user.v1~` - Base IAM user definition
- `gts.x.bss.marketplace.product.v1.0~` - Base marketplace product schema

## Compile-Time Validation

### Error: Missing Property

```rust
#[struct_to_gts_schema(
    file_path = "schemas/user.v1~.schema.json",
    schema_id = "gts.x.app.entities.user.v1~",
    description = "User",
    properties = "id,nonexistent"  // ❌ Error!
)]
pub struct User {
    pub id: String,
}
```

**Compile error:**
```
error: struct_to_gts_schema: Property 'nonexistent' not found in struct.
       Available fields: ["id"]
```

### Error: Invalid File Extension

```rust
#[struct_to_gts_schema(
    file_path = "schemas/user.schema",  // ❌ Error!
    schema_id = "gts.x.app.entities.user.v1~",
    description = "User",
    properties = "id"
)]
pub struct User {
    pub id: String,
}
```

**Compile error:**
```
error: struct_to_gts_schema: file_path must end with '.json'.
       Got: 'schemas/user.schema'
```

### Error: Missing Required Parameter

```rust
#[struct_to_gts_schema(
    file_path = "schemas/user.v1~.schema.json",
    schema_id = "gts.x.app.entities.user.v1~"
    // ❌ Missing: description, properties
)]
pub struct User {
    pub id: String,
}
```

**Compile error:**
```
error: Missing required attribute: description
```

### Error: Invalid Struct Type

```rust
#[struct_to_gts_schema(
    file_path = "schemas/data.v1~.schema.json",
    schema_id = "gts.x.app.entities.data.v1~",
    description = "Data",
    properties = "value"
)]
pub struct Data(String);  // ❌ Tuple struct not supported
```

**Compile error:**
```
error: struct_to_gts_schema: Only structs with named fields are supported
```

## Type Mapping

When you generate schemas, the CLI tool automatically maps Rust types to JSON Schema types:

| Rust Type | JSON Schema Type | Format | Required |
|-----------|------------------|--------|----------|
| `String`, `&str` | `string` | - | Yes |
| `i8`-`i128`, `u8`-`u128` | `integer` | - | Yes |
| `f32`, `f64` | `number` | - | Yes |
| `bool` | `boolean` | - | Yes |
| `Vec<T>` | `array` | - | Yes |
| `Option<T>` | Same as `T` | - | **No** |
| `Uuid` | `string` | `uuid` | Yes |

**Note**: Fields wrapped in `Option<T>` are not marked as required in the generated schema.

## Complete Example

### Define Your Structs

```rust
// src/models.rs
use gts_macros::struct_to_gts_schema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[struct_to_gts_schema(
    file_path = "schemas/gts.x.shop.entities.product.v1~.schema.json",
    schema_id = "gts.x.shop.entities.product.v1~",
    description = "Product entity with pricing",
    properties = "id,name,price,in_stock"
)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub in_stock: bool,
    pub warehouse_id: String,  // Not in schema
}

#[derive(Debug, Serialize, Deserialize)]
#[struct_to_gts_schema(
    file_path = "schemas/gts.x.shop.entities.order.v1~.schema.json",
    schema_id = "gts.x.shop.entities.order.v1~",
    description = "Order entity",
    properties = "id,customer_id,total"
)]
pub struct Order {
    pub id: String,
    pub customer_id: String,
    pub total: f64,
}
```

### Generate Schemas

```bash
# From your project root - uses macro paths
gts generate-from-rust --source src/
# Creates: src/schemas/gts.x.shop.entities.product.v1~.schema.json
#          src/schemas/gts.x.shop.entities.order.v1~.schema.json

# Or override output directory
gts generate-from-rust --source src/ --output api/schemas/
# Creates: api/schemas/schemas/gts.x.shop.entities.product.v1~.schema.json
#          api/schemas/schemas/gts.x.shop.entities.order.v1~.schema.json
```

### Generated Schema

`src/schemas/gts.x.shop.entities.product.v1~.schema.json`:

```json
{
  "$id": "gts.x.shop.entities.product.v1~",
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Product",
  "type": "object",
  "description": "Product entity with pricing",
  "properties": {
    "id": { "type": "string" },
    "name": { "type": "string" },
    "price": { "type": "number" },
    "in_stock": { "type": "boolean" }
  },
  "required": ["id", "name", "price", "in_stock"]
}
```

## Integration with GTS CLI

The generated schemas work seamlessly with all the other GTS CLI commands:

```bash
# Validate schema ID
gts validate-id --gts-id "gts.x.shop.entities.product.v1~"

# Parse schema ID
gts parse-id --gts-id "gts.x.shop.entities.product.v1~"

# List schemas
gts --path schemas list

# ...
```

## Security Features

The CLI tool includes security checks:

1. **Path traversal prevention**: Cannot write files outside the source repository
2. **File extension enforcement**: Both macro and CLI validate `.json` extension
3. **Canonicalization**: Resolves symbolic links and relative paths to prevent escapes

Example of blocked path:

```rust
#[struct_to_gts_schema(
    file_path = "../../../etc/passwd.json",  // Will be blocked by CLI
    schema_id = "gts.x.app.entities.user.v1~",
    description = "User",
    properties = "id"
)]
pub struct User { pub id: String }
```

**CLI error:**
```
Security error: file_path '../../../etc/passwd.json' attempts to write
outside source repository
```

## License

Apache-2.0

## See Also

- [GTS Specification](https://github.com/globaltypesystem/gts-spec)
- [GTS CLI Documentation](../gts-cli/README.md)
- [Example Usage](../gts-macros-test/README.md)
