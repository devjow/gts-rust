/// x-gts-ref validation support for GTS schemas.
///
/// This module implements validation for the `x-gts-ref` extension as specified
/// in the GTS specification v0.5, section 9.5.
///
/// # Overview
///
/// The `x-gts-ref` extension allows schemas to enforce that string values must be
/// valid GTS identifiers or match specific patterns. This is useful for ensuring
/// referential integrity in GTS-based systems.
///
/// # Features
///
/// 1. **Schema Validation**: Validates that `x-gts-ref` fields in schemas contain valid patterns
/// 2. **Instance Validation**: Validates that instance values match their `x-gts-ref` constraints
/// 3. **JSON Pointer Resolution**: Supports JSON Pointer references (e.g., `/$id`, `/properties/name`)
/// 4. **GTS ID Pattern Matching**: Validates GTS IDs and prefix patterns (e.g., `gts.x.y._.z.v1~`)
///
/// # Examples
///
/// ## Schema with x-gts-ref
///
/// ```json
/// {
///   "$id": "gts.x.example._.user.v1~",
///   "type": "object",
///   "properties": {
///     "id": {
///       "type": "string",
///       "x-gts-ref": "/$id"
///     },
///     "role": {
///       "type": "string",
///       "x-gts-ref": "gts.x.example._.role.v1~"
///     }
///   }
/// }
/// ```
///
/// ## Usage
///
/// ```rust
/// use gts::XGtsRefValidator;
/// use serde_json::json;
///
/// let validator = XGtsRefValidator::new();
///
/// // Validate a schema
/// let schema = json!({
///     "$id": "gts.x.test._.schema.v1~",
///     "properties": {
///         "id": {"type": "string", "x-gts-ref": "/$id"}
///     }
/// });
/// let errors = validator.validate_schema(&schema, "", None);
/// assert!(errors.is_empty());
///
/// // Validate an instance
/// let instance = json!({"id": "gts.x.test._.schema.v1~"});
/// let errors = validator.validate_instance(&instance, &schema, "");
/// assert!(errors.is_empty());
/// ```
///
/// # x-gts-ref Patterns
///
/// The `x-gts-ref` field can contain:
///
/// - **GTS ID Pattern**: A full or prefix GTS identifier (e.g., `gts.x.y._.z.v1~`)
/// - **JSON Pointer**: A reference to another field in the schema (e.g., `/$id`, `/properties/name`)
///
/// ## JSON Pointer Resolution
///
/// When an `x-gts-ref` starts with `/`, it's treated as a JSON Pointer that resolves
/// to a value in the schema. The resolved value must be a valid GTS ID pattern.
///
/// Example:
/// ```json
/// {
///   "$id": "gts.x.example._.user.v1~",
///   "properties": {
///     "type": {"type": "string", "x-gts-ref": "/$id"}
///   }
/// }
/// ```
///
/// In this case, the `type` field must match the schema's `$id` value.

use serde_json::Value;
use std::fmt;

use crate::gts::GtsID;

/// Error type for x-gts-ref validation failures
#[derive(Debug, Clone)]
pub struct XGtsRefValidationError {
    pub field_path: String,
    pub value: String,
    pub ref_pattern: String,
    pub reason: String,
}

impl XGtsRefValidationError {
    pub fn new(field_path: String, value: String, ref_pattern: String, reason: String) -> Self {
        Self {
            field_path,
            value,
            ref_pattern,
            reason,
        }
    }
}

impl fmt::Display for XGtsRefValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "x-gts-ref validation failed for field '{}': {}",
            self.field_path, self.reason
        )
    }
}

impl std::error::Error for XGtsRefValidationError {}

/// Validator for x-gts-ref constraints in GTS schemas
pub struct XGtsRefValidator;

impl XGtsRefValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self
    }

    /// Create a default validator
    pub fn default() -> Self {
        Self
    }

    /// Validate an instance against x-gts-ref constraints in schema
    ///
    /// # Arguments
    /// * `instance` - The data instance to validate
    /// * `schema` - The JSON schema with x-gts-ref extensions
    /// * `instance_path` - Current path in instance (for error reporting)
    ///
    /// # Returns
    /// List of validation errors (empty if valid)
    pub fn validate_instance(
        &self,
        instance: &Value,
        schema: &Value,
        instance_path: &str,
    ) -> Vec<XGtsRefValidationError> {
        let mut errors = Vec::new();
        self.visit_instance(instance, schema, schema, instance_path, &mut errors);
        errors
    }

    fn visit_instance(
        &self,
        inst: &Value,
        sch: &Value,
        root_schema: &Value,
        path: &str,
        errors: &mut Vec<XGtsRefValidationError>,
    ) {
        if !sch.is_object() {
            return;
        }

        let sch_obj = sch.as_object().unwrap();

        // Check for x-gts-ref constraint
        if let Some(x_gts_ref) = sch_obj.get("x-gts-ref") {
            if let Some(inst_str) = inst.as_str() {
                if let Some(ref_pattern) = x_gts_ref.as_str() {
                    if let Some(error) = self.validate_ref_value(inst_str, ref_pattern, path, root_schema) {
                        errors.push(error);
                    }
                }
            }
        }

        // Recurse into object properties
        if let Some(Value::String(type_str)) = sch_obj.get("type") {
            if type_str == "object" {
                if let Some(properties) = sch_obj.get("properties") {
                    if let Some(properties_obj) = properties.as_object() {
                        if let Some(inst_obj) = inst.as_object() {
                            for (prop_name, prop_schema) in properties_obj {
                                if let Some(prop_value) = inst_obj.get(prop_name) {
                                    let prop_path = if path.is_empty() {
                                        prop_name.clone()
                                    } else {
                                        format!("{}.{}", path, prop_name)
                                    };
                                    self.visit_instance(prop_value, prop_schema, root_schema, &prop_path, errors);
                                }
                            }
                        }
                    }
                }
            } else if type_str == "array" {
                if let Some(items) = sch_obj.get("items") {
                    if let Some(inst_arr) = inst.as_array() {
                        for (idx, item) in inst_arr.iter().enumerate() {
                            let item_path = format!("{}[{}]", path, idx);
                            self.visit_instance(item, items, root_schema, &item_path, errors);
                        }
                    }
                }
            }
        }
    }

    /// Validate x-gts-ref fields in a schema definition
    ///
    /// # Arguments
    /// * `schema` - The JSON schema to validate
    /// * `schema_path` - Current path in schema (for error reporting)
    /// * `root_schema` - The root schema (for resolving relative refs)
    ///
    /// # Returns
    /// List of validation errors (empty if valid)
    pub fn validate_schema(
        &self,
        schema: &Value,
        schema_path: &str,
        root_schema: Option<&Value>,
    ) -> Vec<XGtsRefValidationError> {
        let root = root_schema.unwrap_or(schema);
        let mut errors = Vec::new();
        self.visit_schema(schema, schema_path, root, &mut errors);
        errors
    }

    fn visit_schema(
        &self,
        sch: &Value,
        path: &str,
        root_schema: &Value,
        errors: &mut Vec<XGtsRefValidationError>,
    ) {
        if !sch.is_object() {
            return;
        }

        let sch_obj = sch.as_object().unwrap();

        // Check for x-gts-ref field
        if let Some(x_gts_ref) = sch_obj.get("x-gts-ref") {
            let ref_path = if path.is_empty() {
                "x-gts-ref".to_string()
            } else {
                format!("{}/x-gts-ref", path)
            };

            if let Some(ref_value) = x_gts_ref.as_str() {
                if let Some(error) = self.validate_ref_pattern(ref_value, &ref_path, root_schema) {
                    errors.push(error);
                }
            } else {
                errors.push(XGtsRefValidationError::new(
                    ref_path,
                    format!("{:?}", x_gts_ref),
                    String::new(),
                    format!(
                        "x-gts-ref value must be a string, got {}",
                        x_gts_ref.to_string()
                    ),
                ));
            }
        }

        // Recurse into nested structures
        for (key, value) in sch_obj {
            if key == "x-gts-ref" {
                continue;
            }
            let nested_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{}/{}", path, key)
            };

            if value.is_object() {
                self.visit_schema(value, &nested_path, root_schema, errors);
            } else if let Some(arr) = value.as_array() {
                for (idx, item) in arr.iter().enumerate() {
                    if item.is_object() {
                        let item_path = format!("{}[{}]", nested_path, idx);
                        self.visit_schema(item, &item_path, root_schema, errors);
                    }
                }
            }
        }
    }

    /// Validate an instance value against its x-gts-ref constraint
    fn validate_ref_value(
        &self,
        value: &str,
        ref_pattern: &str,
        field_path: &str,
        schema: &Value,
    ) -> Option<XGtsRefValidationError> {
        // Resolve pattern if it's a relative reference
        let resolved_pattern = if ref_pattern.starts_with('/') {
            match self.resolve_pointer(schema, ref_pattern) {
                Some(resolved) => {
                    if !resolved.starts_with("gts.") {
                        return Some(XGtsRefValidationError::new(
                            field_path.to_string(),
                            value.to_string(),
                            ref_pattern.to_string(),
                            format!(
                                "Resolved reference '{}' -> '{}' is not a GTS pattern",
                                ref_pattern, resolved
                            ),
                        ));
                    }
                    resolved
                }
                None => {
                    return Some(XGtsRefValidationError::new(
                        field_path.to_string(),
                        value.to_string(),
                        ref_pattern.to_string(),
                        format!("Cannot resolve reference path '{}'", ref_pattern),
                    ));
                }
            }
        } else {
            ref_pattern.to_string()
        };

        // Validate against GTS pattern
        self.validate_gts_pattern(value, &resolved_pattern, field_path)
    }

    /// Validate an x-gts-ref pattern in a schema definition
    fn validate_ref_pattern(
        &self,
        ref_pattern: &str,
        field_path: &str,
        root_schema: &Value,
    ) -> Option<XGtsRefValidationError> {
        // Case 1: Absolute GTS pattern
        if ref_pattern.starts_with("gts.") {
            return self.validate_gts_id_or_pattern(ref_pattern, field_path);
        }

        // Case 2: Relative reference
        if ref_pattern.starts_with('/') {
            match self.resolve_pointer(root_schema, ref_pattern) {
                Some(resolved) => {
                    if !GtsID::is_valid(&resolved) {
                        return Some(XGtsRefValidationError::new(
                            field_path.to_string(),
                            ref_pattern.to_string(),
                            ref_pattern.to_string(),
                            format!(
                                "Resolved reference '{}' -> '{}' is not a valid GTS identifier",
                                ref_pattern, resolved
                            ),
                        ));
                    }
                    None
                }
                None => Some(XGtsRefValidationError::new(
                    field_path.to_string(),
                    ref_pattern.to_string(),
                    ref_pattern.to_string(),
                    format!("Cannot resolve reference path '{}'", ref_pattern),
                )),
            }
        } else {
            Some(XGtsRefValidationError::new(
                field_path.to_string(),
                ref_pattern.to_string(),
                ref_pattern.to_string(),
                format!(
                    "Invalid x-gts-ref value: '{}' must start with 'gts.' or '/'",
                    ref_pattern
                ),
            ))
        }
    }

    /// Validate a GTS ID or pattern in schema definition
    fn validate_gts_id_or_pattern(
        &self,
        pattern: &str,
        field_path: &str,
    ) -> Option<XGtsRefValidationError> {
        // Valid wildcard
        if pattern == "gts.*" {
            return None;
        }

        // Wildcard pattern - validate prefix
        if pattern.contains('*') {
            let prefix = pattern.trim_end_matches('*');
            if !prefix.starts_with("gts.") {
                return Some(XGtsRefValidationError::new(
                    field_path.to_string(),
                    pattern.to_string(),
                    pattern.to_string(),
                    format!("Invalid GTS wildcard pattern: {}", pattern),
                ));
            }
            return None;
        }

        // Specific GTS ID
        if !GtsID::is_valid(pattern) {
            return Some(XGtsRefValidationError::new(
                field_path.to_string(),
                pattern.to_string(),
                pattern.to_string(),
                format!("Invalid GTS identifier: {}", pattern),
            ));
        }

        None
    }

    /// Validate value matches a GTS pattern
    fn validate_gts_pattern(
        &self,
        value: &str,
        pattern: &str,
        field_path: &str,
    ) -> Option<XGtsRefValidationError> {
        // Validate it's a valid GTS ID
        if !GtsID::is_valid(value) {
            return Some(XGtsRefValidationError::new(
                field_path.to_string(),
                value.to_string(),
                pattern.to_string(),
                format!("Value '{}' is not a valid GTS identifier", value),
            ));
        }

        // Check pattern match
        if pattern == "gts.*" {
            // Any valid GTS ID matches
        } else if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            if !value.starts_with(prefix) {
                return Some(XGtsRefValidationError::new(
                    field_path.to_string(),
                    value.to_string(),
                    pattern.to_string(),
                    format!("Value '{}' does not match pattern '{}'", value, pattern),
                ));
            }
        } else if !value.starts_with(pattern) {
            return Some(XGtsRefValidationError::new(
                field_path.to_string(),
                value.to_string(),
                pattern.to_string(),
                format!("Value '{}' does not match pattern '{}'", value, pattern),
            ));
        }

        // Note: We don't check if the entity exists in the store here
        // to avoid borrowing issues. The store check can be done separately if needed.

        None
    }

    /// Resolve a JSON Pointer in the schema
    ///
    /// # Arguments
    /// * `schema` - The schema to search
    /// * `pointer` - JSON Pointer (e.g., "/$id", "/properties/type")
    ///
    /// # Returns
    /// The resolved value as a string or None if not found
    fn resolve_pointer(&self, schema: &Value, pointer: &str) -> Option<String> {
        let path = pointer.trim_start_matches('/');
        if path.is_empty() {
            return None;
        }

        let parts: Vec<&str> = path.split('/').collect();
        let mut current = schema;

        for part in parts {
            if !current.is_object() {
                return None;
            }
            current = current.get(part)?;
        }

        // If current is a string, return it
        if let Some(s) = current.as_str() {
            return Some(s.to_string());
        }

        // If current is an object with x-gts-ref, resolve it
        if let Some(obj) = current.as_object() {
            if let Some(ref_value) = obj.get("x-gts-ref") {
                if let Some(ref_str) = ref_value.as_str() {
                    if ref_str.starts_with('/') {
                        return self.resolve_pointer(schema, ref_str);
                    }
                    return Some(ref_str.to_string());
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_gts_pattern_exact_match() {
        let validator = XGtsRefValidator::new();
        let result = validator.validate_gts_pattern(
            "gts.x.core.events.topic.v1~",
            "gts.x.core.events.topic.v1~",
            "test_field",
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_validate_gts_pattern_wildcard() {
        let validator = XGtsRefValidator::new();
        let result = validator.validate_gts_pattern(
            "gts.x.core.events.topic.v1~",
            "gts.*",
            "test_field",
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_validate_gts_pattern_prefix_match() {
        let validator = XGtsRefValidator::new();
        let result = validator.validate_gts_pattern(
            "gts.x.core.events.topic.v1~",
            "gts.x.core.*",
            "test_field",
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_validate_gts_pattern_mismatch() {
        let validator = XGtsRefValidator::new();
        let result = validator.validate_gts_pattern(
            "gts.x.core.events.topic.v1~",
            "gts.y.core.*",
            "test_field",
        );
        assert!(result.is_some());
    }

    #[test]
    fn test_validate_schema_with_x_gts_ref() {
        let validator = XGtsRefValidator::new();
        let schema = json!({
            "type": "object",
            "properties": {
                "topic_id": {
                    "type": "string",
                    "x-gts-ref": "gts.x.core.events.topic.*"
                }
            }
        });

        let errors = validator.validate_schema(&schema, "", None);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_instance_with_x_gts_ref() {
        let validator = XGtsRefValidator::new();
        let schema = json!({
            "type": "object",
            "properties": {
                "topic_id": {
                    "type": "string",
                    "x-gts-ref": "gts.x.core.events.topic.*"
                }
            }
        });

        let instance = json!({
            "topic_id": "gts.x.core.events.topic.v1~"
        });

        let errors = validator.validate_instance(&instance, &schema, "");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_instance_with_x_gts_ref_mismatch() {
        let validator = XGtsRefValidator::new();
        let schema = json!({
            "type": "object",
            "properties": {
                "topic_id": {
                    "type": "string",
                    "x-gts-ref": "gts.x.core.events.topic.*"
                }
            }
        });

        let instance = json!({
            "topic_id": "gts.y.core.events.topic.v1~"
        });

        let errors = validator.validate_instance(&instance, &schema, "");
        assert!(!errors.is_empty());
    }
}
