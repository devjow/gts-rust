#![allow(clippy::unwrap_used, clippy::expect_used)]

use gts_macros::struct_to_gts_schema;
use serde::{Deserialize, Serialize};

/// User entity for testing GTS schema generation
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
    // This field is not included in the schema
    pub internal_data: Option<String>,
}

/// Product entity for testing GTS schema generation
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
    // This field is not included in the schema
    pub warehouse_location: String,
}

// =============================================================================
// Tests for 3.a) GTS_TYPE_SCHEMA - JSON Schema with proper $id
// =============================================================================

#[test]
fn test_schema_json_contains_id() {
    // Verify GTS_TYPE_SCHEMA contains proper $id equal to schema_id
    assert!(User::GTS_TYPE_SCHEMA.contains(r#""$id": "gts.x.test.entities.user.v1~""#));
    assert!(Product::GTS_TYPE_SCHEMA.contains(r#""$id": "gts.x.test.entities.product.v1~""#));
}

#[test]
fn test_schema_json_contains_description() {
    assert!(User::GTS_TYPE_SCHEMA.contains("User entity with basic information"));
    assert!(Product::GTS_TYPE_SCHEMA.contains("Product entity with pricing information"));
}

#[test]
fn test_schema_json_contains_only_specified_properties() {
    // User: id, email, name, age should be present
    assert!(User::GTS_TYPE_SCHEMA.contains(r#""id""#));
    assert!(User::GTS_TYPE_SCHEMA.contains(r#""email""#));
    assert!(User::GTS_TYPE_SCHEMA.contains(r#""name""#));
    assert!(User::GTS_TYPE_SCHEMA.contains(r#""age""#));
    // internal_data should NOT be present (not in properties list)
    assert!(!User::GTS_TYPE_SCHEMA.contains("internal_data"));

    // Product: id, name, price, description, in_stock should be present
    assert!(Product::GTS_TYPE_SCHEMA.contains(r#""id""#));
    assert!(Product::GTS_TYPE_SCHEMA.contains(r#""name""#));
    assert!(Product::GTS_TYPE_SCHEMA.contains(r#""price""#));
    assert!(Product::GTS_TYPE_SCHEMA.contains(r#""description""#));
    assert!(Product::GTS_TYPE_SCHEMA.contains(r#""in_stock""#));
    // warehouse_location should NOT be present (not in properties list)
    assert!(!Product::GTS_TYPE_SCHEMA.contains("warehouse_location"));
}

#[test]
fn test_schema_json_is_valid_json() {
    // Verify the schema JSON can be parsed
    let user_schema: serde_json::Value = serde_json::from_str(User::GTS_TYPE_SCHEMA).unwrap();
    let product_schema: serde_json::Value = serde_json::from_str(Product::GTS_TYPE_SCHEMA).unwrap();

    // Verify key fields
    assert_eq!(user_schema["$id"], "gts.x.test.entities.user.v1~");
    assert_eq!(user_schema["type"], "object");
    assert_eq!(
        user_schema["$schema"],
        "http://json-schema.org/draft-07/schema#"
    );

    assert_eq!(product_schema["$id"], "gts.x.test.entities.product.v1~");
    assert_eq!(product_schema["type"], "object");
}

#[test]
fn test_schema_json_required_fields() {
    let user_schema: serde_json::Value = serde_json::from_str(User::GTS_TYPE_SCHEMA).unwrap();
    let required = user_schema["required"].as_array().unwrap();

    // All non-Option fields in properties should be required
    assert!(required.contains(&serde_json::json!("id")));
    assert!(required.contains(&serde_json::json!("email")));
    assert!(required.contains(&serde_json::json!("name")));
    assert!(required.contains(&serde_json::json!("age")));

    // Product: description is Option<String>, so should NOT be required
    let product_schema: serde_json::Value = serde_json::from_str(Product::GTS_TYPE_SCHEMA).unwrap();
    let product_required = product_schema["required"].as_array().unwrap();
    assert!(!product_required.contains(&serde_json::json!("description")));
    assert!(product_required.contains(&serde_json::json!("price")));
}

// =============================================================================
// Tests for 3.b) gts_instance_id - Generate instance IDs
// =============================================================================

#[test]
fn test_gts_instance_id_simple_segment() {
    // Test with simple segment
    let id = User::GTS_INSTANCE_ID("123.v1");
    assert_eq!(id, "gts.x.test.entities.user.v1~123.v1");

    let id = Product::GTS_INSTANCE_ID("abc.v1");
    assert_eq!(id, "gts.x.test.entities.product.v1~abc.v1");
}

#[test]
fn test_gts_instance_id_multi_segment() {
    // Test with multi-part segment like "a.b.c.d.v1"
    let id = User::GTS_INSTANCE_ID("orders.commerce.v1");
    assert_eq!(id, "gts.x.test.entities.user.v1~orders.commerce.v1");
}

#[test]
fn test_gts_instance_id_with_wildcard_segment() {
    // Test with segment containing wildcard "_"
    let id = User::GTS_INSTANCE_ID("a.b._.d.v1.0");
    assert_eq!(id, "gts.x.test.entities.user.v1~a.b._.d.v1.0");
}

#[test]
fn test_gts_instance_id_versioned_segment() {
    // Test with versioned segment
    let id = User::GTS_INSTANCE_ID("instance.v1.0");
    assert_eq!(id, "gts.x.test.entities.user.v1~instance.v1.0");

    let id = Product::GTS_INSTANCE_ID("sku.v2.1");
    assert_eq!(id, "gts.x.test.entities.product.v1~sku.v2.1");
}

#[test]
fn test_gts_instance_id_empty_segment() {
    // Edge case: empty segment returns just the schema_id
    let id = User::GTS_INSTANCE_ID("");
    assert_eq!(id, "gts.x.test.entities.user.v1~");
}

// =============================================================================
// Tests for metadata constants
// =============================================================================

#[test]
fn test_schema_id_constant() {
    assert_eq!(User::GTS_SCHEMA_ID, "gts.x.test.entities.user.v1~");
    assert_eq!(Product::GTS_SCHEMA_ID, "gts.x.test.entities.product.v1~");
}

#[test]
fn test_file_path_constant() {
    assert_eq!(
        User::GTS_SCHEMA_FILE_PATH,
        "schemas/gts.x.test.entities.user.v1~.schema.json"
    );
    assert_eq!(
        Product::GTS_SCHEMA_FILE_PATH,
        "schemas/gts.x.test.entities.product.v1~.schema.json"
    );
}

#[test]
fn test_properties_constant() {
    assert_eq!(User::GTS_SCHEMA_PROPERTIES, "id,email,name,age");
    assert_eq!(
        Product::GTS_SCHEMA_PROPERTIES,
        "id,name,price,description,in_stock"
    );
}

// =============================================================================
// Tests for serialization (struct still works normally)
// =============================================================================

#[test]
fn test_user_serialization() {
    let user = User {
        id: User::GTS_INSTANCE_ID("a.b.c.d.v1"), // GTS ID
        email: "test@example.com".to_owned(),
        name: "Test User".to_owned(),
        age: 30,
        internal_data: Some("internal".to_owned()),
    };

    let json = serde_json::to_string(&user).unwrap();
    assert!(json.contains("gts.x.test.entities.user.v1~a.b.c.d.v1"));
    assert!(json.contains("test@example.com"));
}

#[test]
fn test_product_serialization() {
    let product = Product {
        id: "prod-456".to_owned(), // Non GTS ID
        name: "Test Product".to_owned(),
        price: 99.99,
        description: Some("A test product".to_owned()),
        in_stock: true,
        warehouse_location: "Warehouse A".to_owned(),
    };

    let json = serde_json::to_string(&product).unwrap();
    assert!(json.contains("prod-456"));
    assert!(json.contains("99.99"));
}
