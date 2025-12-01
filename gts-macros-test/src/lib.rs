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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_metadata() {
        // Verify the schema metadata is embedded
        assert_eq!(User::GTS_SCHEMA_ID, "gts.x.test.entities.user.v1~");
        assert!(User::GTS_SCHEMA_JSON.contains("User entity with basic information"));
        assert!(User::GTS_SCHEMA_JSON.contains("\"id\""));
        assert!(User::GTS_SCHEMA_JSON.contains("\"email\""));
    }

    #[test]
    fn test_product_metadata() {
        // Verify the schema metadata is embedded
        assert_eq!(Product::GTS_SCHEMA_ID, "gts.x.test.entities.product.v1~");
        assert!(Product::GTS_SCHEMA_JSON.contains("Product entity with pricing information"));
        assert!(Product::GTS_SCHEMA_JSON.contains("\"price\""));
    }

    #[test]
    fn test_user_serialization() {
        let user = User {
            id: "user-123".to_string(),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
            age: 30,
            internal_data: Some("internal".to_string()),
        };

        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("user-123"));
        assert!(json.contains("test@example.com"));
    }

    #[test]
    fn test_product_serialization() {
        let product = Product {
            id: "prod-456".to_string(),
            name: "Test Product".to_string(),
            price: 99.99,
            description: Some("A test product".to_string()),
            in_stock: true,
            warehouse_location: "Warehouse A".to_string(),
        };

        let json = serde_json::to_string(&product).unwrap();
        assert!(json.contains("prod-456"));
        assert!(json.contains("99.99"));
    }
}
