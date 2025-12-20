# GTS Macros

Procedural macros for GTS (Global Type System) schema generation from Rust structs.

## Overview

The `#[struct_to_gts_schema]` attribute macro serves **three purposes**:

1. **Compile-Time Validation** - Catches configuration errors before runtime
2. **Schema Generation** - Enables CLI-based JSON Schema file generation
3. **Runtime API** - Provides schema access and instance ID generation at runtime

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
gts-macros = { path = "path/to/gts-rust/gts-macros" }
serde = { version = "1.0", features = ["derive"] }
```

## Quick Start

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

// Runtime usage:
fn example() {
    // Get the JSON Schema
    let schema = User::GTS_TYPE_SCHEMA;

    // Generate instance IDs
    let instance_id = User::GTS_INSTANCE_ID("123.v1");
    assert_eq!(instance_id, "gts.x.myapp.entities.user.v1~123.v1");
}
```

---

## Purpose 1: Compile-Time Validation

The macro validates your annotations at compile time, catching errors early.

### What Gets Validated

| Check | Description |
|-------|-------------|
| **Required parameters** | All of `file_path`, `schema_id`, `description`, `properties` must be present |
| **Property existence** | Every property in the list must exist as a field in the struct |
| **Struct type** | Only structs with named fields are supported (no tuple structs) |
| **File extension** | `file_path` must end with `.json` |

### Compile Error Examples

**Missing property:**
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
```
error: struct_to_gts_schema: Property 'nonexistent' not found in struct.
       Available fields: ["id"]
```

**Invalid file extension:**
```rust
#[struct_to_gts_schema(
    file_path = "schemas/user.schema",  // ❌ Must end with .json
    // ...
)]
```
```
error: struct_to_gts_schema: file_path must end with '.json'.
       Got: 'schemas/user.schema'
```

**Tuple struct:**
```rust
#[struct_to_gts_schema(/* ... */)]
pub struct Data(String);  // ❌ Tuple struct not supported
```
```
error: struct_to_gts_schema: Only structs with named fields are supported
```

---

## Purpose 2: Schema Generation

Generate JSON Schema files using the GTS CLI tool.

### Generate Schemas

```bash
# Using paths from macro (relative to source files)
gts generate-from-rust --source src/

# Override output directory
gts generate-from-rust --source src/ --output schemas/

# Exclude specific directories (can be used multiple times)
gts generate-from-rust --source . --exclude "tests/*" --exclude "examples/*"

# Using cargo
cargo run --bin gts -- generate-from-rust --source src/
```

### Excluding Files

The CLI provides multiple ways to exclude files from scanning:

**1. `--exclude` option** (supports glob patterns):
```bash
gts generate-from-rust --source . --exclude "tests/*" --exclude "benches/*"
```

**2. Auto-ignored directories**: The following directories are automatically skipped:
- `compile_fail/` - trybuild compile-fail tests

**3. `// gts:ignore` directive**: Add this comment at the top of any `.rs` file:
```rust
// gts:ignore
//! This file will be skipped by the CLI

use gts_macros::struct_to_gts_schema;
// ...
```

### What the CLI Does

1. Scans source files for `#[struct_to_gts_schema]` annotations
2. Extracts metadata (schema_id, description, properties)
3. Maps Rust types to JSON Schema types
4. Generates valid JSON Schema files at the specified `file_path`

### Generated Schema Example

For the `User` struct above, generates `schemas/gts.x.myapp.entities.user.v1~.schema.json`:

```json
{
  "$id": "gts.x.myapp.entities.user.v1~",
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "User",
  "type": "object",
  "description": "User entity with authentication information",
  "properties": {
    "id": { "type": "string" },
    "email": { "type": "string" },
    "name": { "type": "string" }
  },
  "required": ["id", "email", "name"]
}
```

### Type Mapping

| Rust Type | JSON Schema Type | Format | Required |
|-----------|------------------|--------|----------|
| `String`, `&str` | `string` | - | Yes |
| `i8`-`i128`, `u8`-`u128` | `integer` | - | Yes |
| `f32`, `f64` | `number` | - | Yes |
| `bool` | `boolean` | - | Yes |
| `Vec<T>` | `array` | - | Yes |
| `Option<T>` | Same as `T` | - | **No** |
| `Uuid` | `string` | `uuid` | Yes |
| `DateTime`, `NaiveDateTime` | `string` | `date-time` | Yes |
| `NaiveDate` | `string` | `date` | Yes |
| `HashMap<K,V>`, `BTreeMap<K,V>` | `object` | - | Yes |

**Note**: `Option<T>` fields are not marked as `required` in the generated schema.

---

## Purpose 3: Runtime API

The macro generates associated constants and methods for runtime use.

### `GTS_TYPE_SCHEMA`

A compile-time constant containing the JSON Schema with `$id` set to `schema_id`.

```rust
// Access the schema at runtime
let schema: &'static str = User::GTS_TYPE_SCHEMA;

// Parse it if needed
let parsed: serde_json::Value = serde_json::from_str(schema).unwrap();
assert_eq!(parsed["$id"], "gts.x.myapp.entities.user.v1~");
```

### `GTS_INSTANCE_ID(segment)`

Generate instance IDs by appending a segment to the schema ID.

```rust
// Simple segment
let id = User::GTS_INSTANCE_ID("x.core.namespace.type.v1");
assert_eq!(id, "gts.x.myapp.entities.user.v1~x.core.namespace.type.v1");

// Multi-part segment
let id = User::GTS_INSTANCE_ID("x.bss.orders.commerce.v1");
assert_eq!(id, "gts.x.myapp.entities.user.v1~x.bss.orders.commerce.v1");

// Segment with wildcard
let id = User::GTS_INSTANCE_ID("a.b._.d.v1.0");
assert_eq!(id, "gts.x.myapp.entities.user.v1~a.b._.d.v1.0");

// Versioned segment
let id = User::GTS_INSTANCE_ID("vendor.pkg.namespace.instance.v2.1");
assert_eq!(id, "gts.x.myapp.entities.user.v1~vendor.pkg.namespace.instance.v2.1");
```

### Other Generated Constants

| Constant | Description |
|----------|-------------|
| `GTS_SCHEMA_ID` | The schema ID string |
| `GTS_SCHEMA_FILE_PATH` | The file path for CLI generation |
| `GTS_SCHEMA_DESCRIPTION` | The description string |
| `GTS_SCHEMA_PROPERTIES` | Comma-separated property list |

---

## Macro Parameters

All parameters are **required**:

| Parameter | Description | Example |
|-----------|-------------|---------|
| `file_path` | Output path for generated schema | `"schemas/gts.x.app.user.v1~.schema.json"` |
| `schema_id` | GTS identifier | `"gts.x.app.entities.user.v1~"` |
| `description` | Human-readable description | `"User entity"` |
| `properties` | Comma-separated field list | `"id,email,name"` |

### GTS ID Format

```
gts.<vendor>.<package>.<namespace>.<type>.v<MAJOR>[.<MINOR>]~
```

Examples:
- `gts.x.core.iam.user.v1~` - IAM user schema
- `gts.x.commerce.orders.order.v1.0~` - Order schema with minor version

---

## Complete Example

### Define Structs

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
    properties = "id,customer_id,total,status"
)]
pub struct Order {
    pub id: String,
    pub customer_id: String,
    pub total: f64,
    pub status: Option<String>,  // Optional field
}
```

### Generate Schemas

```bash
gts generate-from-rust --source src/
# Output:
#   Generated schema: gts.x.shop.entities.product.v1~ @ src/schemas/gts.x.shop.entities.product.v1~.schema.json
#   Generated schema: gts.x.shop.entities.order.v1~ @ src/schemas/gts.x.shop.entities.order.v1~.schema.json
```

### Use at Runtime

```rust
fn main() {
    // Access schema
    println!("Product schema: {}", Product::GTS_TYPE_SCHEMA);

    // Generate instance IDs
    let product_id = Product::GTS_INSTANCE_ID("sku-12345.v1");
    let order_id = Order::GTS_INSTANCE_ID("ord-98765.v1");

    println!("Product ID: {}", product_id);
    // Output: gts.x.shop.entities.product.v1~sku-12345.v1

    println!("Order ID: {}", order_id);
    // Output: gts.x.shop.entities.order.v1~ord-98765.v1
}
```

---

## Security Features

The CLI includes security checks:

1. **Path traversal prevention** - Cannot write files outside the source repository
2. **File extension enforcement** - Both macro and CLI validate `.json` extension
3. **Canonicalization** - Resolves symbolic links to prevent escapes

---

## License

Apache-2.0

## See Also

- [GTS Specification](https://github.com/globaltypesystem/gts-spec)
- [GTS CLI Documentation](../gts-cli/README.md)
