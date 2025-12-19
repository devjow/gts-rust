use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::gts::GtsID;
use crate::path_resolver::JsonPathResolver;
use crate::schema_cast::{GtsEntityCastResult, SchemaCastError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    #[serde(rename = "instancePath")]
    pub instance_path: String,
    #[serde(rename = "schemaPath")]
    pub schema_path: String,
    pub keyword: String,
    pub message: String,
    pub params: HashMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Clone)]
pub struct GtsFile {
    pub path: String,
    pub name: String,
    pub content: Value,
    pub sequences_count: usize,
    pub sequence_content: HashMap<usize, Value>,
    pub validation: ValidationResult,
}

impl GtsFile {
    pub fn new(path: String, name: String, content: Value) -> Self {
        let sequence_content: HashMap<usize, Value> = if let Some(arr) = content.as_array() {
            arr.iter()
                .enumerate()
                .map(|(i, v)| (i, v.clone()))
                .collect()
        } else {
            [(0, content.clone())].into_iter().collect()
        };
        let sequences_count = sequence_content.len();

        GtsFile {
            path,
            name,
            content,
            sequences_count,
            sequence_content,
            validation: ValidationResult::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtsConfig {
    pub entity_id_fields: Vec<String>,
    pub schema_id_fields: Vec<String>,
}

impl Default for GtsConfig {
    fn default() -> Self {
        GtsConfig {
            entity_id_fields: vec![
                "$id".to_string(),
                "gtsId".to_string(),
                "gtsIid".to_string(),
                "gtsOid".to_string(),
                "gtsI".to_string(),
                "gts_id".to_string(),
                "gts_oid".to_string(),
                "gts_iid".to_string(),
                "id".to_string(),
            ],
            schema_id_fields: vec![
                "$schema".to_string(),
                "gtsTid".to_string(),
                "gtsType".to_string(),
                "gtsT".to_string(),
                "gts_t".to_string(),
                "gts_tid".to_string(),
                "gts_type".to_string(),
                "type".to_string(),
                "schema".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct GtsRef {
    pub id: String,
    pub source_path: String,
}

#[derive(Debug, Clone)]
pub struct GtsEntity {
    pub gts_id: Option<GtsID>,
    pub is_schema: bool,
    pub file: Option<GtsFile>,
    pub list_sequence: Option<usize>,
    pub label: String,
    pub content: Value,
    pub gts_refs: Vec<GtsRef>,
    pub validation: ValidationResult,
    pub schema_id: Option<String>,
    pub selected_entity_field: Option<String>,
    pub selected_schema_id_field: Option<String>,
    pub description: String,
    pub schema_refs: Vec<GtsRef>,
}

impl GtsEntity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        file: Option<GtsFile>,
        list_sequence: Option<usize>,
        content: &Value,
        cfg: Option<&GtsConfig>,
        gts_id: Option<GtsID>,
        is_schema: bool,
        label: String,
        validation: Option<ValidationResult>,
        schema_id: Option<String>,
    ) -> Self {
        let mut entity = GtsEntity {
            file,
            list_sequence,
            content: content.clone(),
            gts_id,
            is_schema,
            label,
            validation: validation.unwrap_or_default(),
            schema_id,
            selected_entity_field: None,
            selected_schema_id_field: None,
            gts_refs: Vec::new(),
            schema_refs: Vec::new(),
            description: String::new(),
        };

        // Auto-detect if this is a schema
        if entity.is_json_schema_entity() {
            entity.is_schema = true;
        }

        // Calculate IDs if config provided
        if let Some(cfg) = cfg {
            let idv = entity.calc_json_entity_id(cfg);
            entity.schema_id = entity.calc_json_schema_id(cfg);

            // If no valid GTS ID found in entity fields, use schema ID as fallback
            let mut final_id = idv;
            let is_valid_id = final_id.as_ref().is_some_and(|id| GtsID::is_valid(id));
            if !is_valid_id {
                if let Some(ref sid) = entity.schema_id {
                    if GtsID::is_valid(sid) {
                        final_id = Some(sid.clone());
                    }
                }
            }

            entity.gts_id = final_id.and_then(|id| GtsID::new(&id).ok());
        }

        // Set label
        if let Some(ref file) = entity.file {
            if let Some(seq) = entity.list_sequence {
                entity.label = format!("{}#{seq}", file.name);
            } else {
                entity.label = file.name.clone();
            }
        } else if let Some(ref gts_id) = entity.gts_id {
            entity.label = gts_id.id.clone();
        } else if entity.label.is_empty() {
            entity.label = String::new();
        }

        // Extract description
        if let Some(obj) = content.as_object() {
            if let Some(desc) = obj.get("description") {
                if let Some(s) = desc.as_str() {
                    entity.description = s.to_string();
                }
            }
        }

        // Extract references
        entity.gts_refs = entity.extract_gts_ids_with_paths();
        if entity.is_schema {
            entity.schema_refs = entity.extract_ref_strings_with_paths();
        }

        entity
    }

    fn is_json_schema_entity(&self) -> bool {
        // Check if GTS ID ends with '~' (schema marker)
        if let Some(ref gts_id) = self.gts_id {
            if gts_id.id.ends_with('~') {
                return true;
            }
        }

        // Check for $id field ending with '~' (schema marker)
        if let Some(obj) = self.content.as_object() {
            if let Some(id_value) = obj.get("$id") {
                if let Some(id_str) = id_value.as_str() {
                    if id_str.ends_with('~') {
                        return true;
                    }
                }
            }

            // Check for $schema field
            if let Some(url) = obj.get("$schema") {
                if let Some(url_str) = url.as_str() {
                    return url_str.starts_with("http://json-schema.org/")
                        || url_str.starts_with("https://json-schema.org/")
                        || url_str.starts_with("gts://")
                        || url_str.starts_with("gts.");
                }
            }
        }
        false
    }

    pub fn resolve_path(&self, path: &str) -> JsonPathResolver {
        let gts_id = self
            .gts_id
            .as_ref()
            .map(|g| g.id.clone())
            .unwrap_or_default();
        JsonPathResolver::new(gts_id, self.content.clone()).resolve(path)
    }

    pub fn cast(
        &self,
        to_schema: &GtsEntity,
        from_schema: &GtsEntity,
        resolver: Option<&()>,
    ) -> Result<GtsEntityCastResult, SchemaCastError> {
        if self.is_schema {
            // When casting a schema, from_schema might be a standard JSON Schema (no gts_id)
            if let (Some(ref self_id), Some(ref from_id)) = (&self.gts_id, &from_schema.gts_id) {
                if self_id.id != from_id.id {
                    return Err(SchemaCastError::InternalError(format!(
                        "Internal error: {} != {}",
                        self_id.id, from_id.id
                    )));
                }
            }
        }

        if !to_schema.is_schema {
            return Err(SchemaCastError::TargetMustBeSchema);
        }

        if !from_schema.is_schema {
            return Err(SchemaCastError::SourceMustBeSchema);
        }

        let from_id = self
            .gts_id
            .as_ref()
            .map(|g| g.id.clone())
            .unwrap_or_default();
        let to_id = to_schema
            .gts_id
            .as_ref()
            .map(|g| g.id.clone())
            .unwrap_or_default();

        GtsEntityCastResult::cast(
            &from_id,
            &to_id,
            &self.content,
            &from_schema.content,
            &to_schema.content,
            resolver,
        )
    }

    fn walk_and_collect<F>(content: &Value, collector: &mut Vec<GtsRef>, matcher: F)
    where
        F: Fn(&Value, &str) -> Option<GtsRef> + Copy,
    {
        fn walk<F>(node: &Value, current_path: &str, collector: &mut Vec<GtsRef>, matcher: F)
        where
            F: Fn(&Value, &str) -> Option<GtsRef> + Copy,
        {
            // Try to match current node
            if let Some(match_result) = matcher(node, current_path) {
                collector.push(match_result);
            }

            // Recurse into structures
            match node {
                Value::Object(map) => {
                    for (k, v) in map {
                        let next_path = if current_path.is_empty() {
                            k.clone()
                        } else {
                            format!("{}.{}", current_path, k)
                        };
                        walk(v, &next_path, collector, matcher);
                    }
                }
                Value::Array(arr) => {
                    for (idx, item) in arr.iter().enumerate() {
                        let next_path = format!("{}[{}]", current_path, idx);
                        walk(item, &next_path, collector, matcher);
                    }
                }
                _ => {}
            }
        }

        walk(content, "", collector, matcher);
    }

    fn deduplicate_by_id_and_path(items: Vec<GtsRef>) -> Vec<GtsRef> {
        let mut seen = HashMap::new();
        let mut result = Vec::new();

        for item in items {
            let key = format!("{}|{}", item.id, item.source_path);
            if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(key) {
                e.insert(true);
                result.push(item);
            }
        }

        result
    }

    fn extract_gts_ids_with_paths(&self) -> Vec<GtsRef> {
        let mut found = Vec::new();

        let gts_id_matcher = |node: &Value, path: &str| -> Option<GtsRef> {
            if let Some(s) = node.as_str() {
                if GtsID::is_valid(s) {
                    return Some(GtsRef {
                        id: s.to_string(),
                        source_path: if path.is_empty() {
                            "root".to_string()
                        } else {
                            path.to_string()
                        },
                    });
                }
            }
            None
        };

        Self::walk_and_collect(&self.content, &mut found, gts_id_matcher);
        Self::deduplicate_by_id_and_path(found)
    }

    fn extract_ref_strings_with_paths(&self) -> Vec<GtsRef> {
        let mut refs = Vec::new();

        let ref_matcher = |node: &Value, path: &str| -> Option<GtsRef> {
            if let Some(obj) = node.as_object() {
                if let Some(ref_val) = obj.get("$ref") {
                    if let Some(ref_str) = ref_val.as_str() {
                        let ref_path = if path.is_empty() {
                            "$ref".to_string()
                        } else {
                            format!("{}.$ref", path)
                        };
                        return Some(GtsRef {
                            id: ref_str.to_string(),
                            source_path: ref_path,
                        });
                    }
                }
            }
            None
        };

        Self::walk_and_collect(&self.content, &mut refs, ref_matcher);
        Self::deduplicate_by_id_and_path(refs)
    }

    fn get_field_value(&self, field: &str) -> Option<String> {
        if let Some(obj) = self.content.as_object() {
            if let Some(v) = obj.get(field) {
                if let Some(s) = v.as_str() {
                    if !s.trim().is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
        None
    }

    fn first_non_empty_field(&mut self, fields: &[String]) -> Option<String> {
        // First pass: look for valid GTS IDs
        for f in fields {
            if let Some(v) = self.get_field_value(f) {
                if GtsID::is_valid(&v) {
                    self.selected_entity_field = Some(f.clone());
                    return Some(v);
                }
            }
        }

        // Second pass: any non-empty string
        for f in fields {
            if let Some(v) = self.get_field_value(f) {
                self.selected_entity_field = Some(f.clone());
                return Some(v);
            }
        }

        None
    }

    fn calc_json_entity_id(&mut self, cfg: &GtsConfig) -> Option<String> {
        if let Some(id) = self.first_non_empty_field(&cfg.entity_id_fields) {
            return Some(id);
        }

        if let Some(ref file) = self.file {
            if let Some(seq) = self.list_sequence {
                return Some(format!("{}#{}", file.path, seq));
            }
            return Some(file.path.clone());
        }

        None
    }

    fn calc_json_schema_id(&mut self, cfg: &GtsConfig) -> Option<String> {
        // First try schema-specific fields
        for f in &cfg.schema_id_fields {
            if let Some(v) = self.get_field_value(f) {
                self.selected_schema_id_field = Some(f.clone());
                return Some(v);
            }
        }

        // Fallback to entity ID logic
        let idv = self.first_non_empty_field(&cfg.entity_id_fields);
        if let Some(ref id) = idv {
            if GtsID::is_valid(id) {
                if id.ends_with('~') {
                    // Don't set selected_schema_id_field when the entity ID itself is a schema ID
                    return Some(id.clone());
                }
                if let Some(last) = id.rfind('~') {
                    // Only set selected_schema_id_field when extracting a substring
                    self.selected_schema_id_field = self.selected_entity_field.clone();
                    return Some(id[..=last].to_string());
                }
            }
        }

        if let Some(ref file) = self.file {
            if let Some(seq) = self.list_sequence {
                return Some(format!("{}#{}", file.path, seq));
            }
            return Some(file.path.clone());
        }

        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_file_with_description() {
        let content = json!({
            "id": "gts.vendor.package.namespace.type.v1.0",
            "description": "Test description"
        });

        let cfg = GtsConfig::default();
        let entity = GtsEntity::new(
            None,
            None,
            &content,
            Some(&cfg),
            None,
            false,
            String::new(),
            None,
            None,
        );

        assert_eq!(entity.description, "Test description");
    }

    #[test]
    fn test_json_entity_with_file_and_sequence() {
        let file_content = json!([
            {"id": "gts.vendor.package.namespace.type.v1.0"},
            {"id": "gts.vendor.package.namespace.type.v1.1"}
        ]);

        let file = GtsFile::new(
            "/path/to/file.json".to_owned(),
            "file.json".to_owned(),
            file_content,
        );

        let entity_content = json!({"id": "gts.vendor.package.namespace.type.v1.0"});
        let cfg = GtsConfig::default();

        let entity = GtsEntity::new(
            Some(file),
            Some(0),
            &entity_content,
            Some(&cfg),
            None,
            false,
            String::new(),
            None,
            None,
        );

        assert_eq!(entity.label, "file.json#0");
    }

    #[test]
    fn test_json_entity_with_file_no_sequence() {
        let file_content = json!({"id": "gts.vendor.package.namespace.type.v1.0"});

        let file = GtsFile::new(
            "/path/to/file.json".to_owned(),
            "file.json".to_owned(),
            file_content,
        );

        let entity_content = json!({"id": "gts.vendor.package.namespace.type.v1.0"});
        let cfg = GtsConfig::default();

        let entity = GtsEntity::new(
            Some(file),
            None,
            &entity_content,
            Some(&cfg),
            None,
            false,
            String::new(),
            None,
            None,
        );

        assert_eq!(entity.label, "file.json");
    }

    #[test]
    fn test_json_entity_extract_gts_ids() {
        let content = json!({
            "id": "gts.vendor.package.namespace.type.v1.0",
            "nested": {
                "ref": "gts.other.package.namespace.type.v2.0"
            }
        });

        let cfg = GtsConfig::default();
        let entity = GtsEntity::new(
            None,
            None,
            &content,
            Some(&cfg),
            None,
            false,
            String::new(),
            None,
            None,
        );

        // gts_refs is populated during entity construction
        assert!(!entity.gts_refs.is_empty());
    }

    #[test]
    fn test_json_entity_extract_ref_strings() {
        let content = json!({
            "$ref": "gts.vendor.package.namespace.type.v1.0~",
            "properties": {
                "user": {
                    "$ref": "gts.other.package.namespace.type.v2.0~"
                }
            }
        });

        let cfg = GtsConfig::default();
        let entity = GtsEntity::new(
            None,
            None,
            &content,
            Some(&cfg),
            None,
            true, // Mark as schema so schema_refs gets populated
            String::new(),
            None,
            None,
        );

        // schema_refs is populated during entity construction for schemas
        assert!(!entity.schema_refs.is_empty());
    }

    #[test]
    fn test_json_entity_is_json_schema_entity() {
        let schema_content = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object"
        });

        let entity = GtsEntity::new(
            None,
            None,
            &schema_content,
            None,
            None,
            false,
            String::new(),
            None,
            None,
        );

        assert!(entity.is_schema);
    }

    #[test]
    fn test_json_entity_fallback_to_schema_id() {
        let content = json!({
            "type": "gts.vendor.package.namespace.type.v1.0~",
            "name": "test"
        });

        let cfg = GtsConfig::default();
        let entity = GtsEntity::new(
            None,
            None,
            &content,
            Some(&cfg),
            None,
            false,
            String::new(),
            None,
            None,
        );

        // Should fallback to schema_id when entity_id is not found
        assert!(entity.gts_id.is_some());
    }

    #[test]
    fn test_json_entity_with_custom_label() {
        let content = json!({"name": "test"});

        let entity = GtsEntity::new(
            None,
            None,
            &content,
            None,
            None,
            false,
            "custom_label".to_owned(),
            None,
            None,
        );

        assert_eq!(entity.label, "custom_label");
    }

    #[test]
    fn test_json_entity_empty_label_fallback() {
        let content = json!({"name": "test"});

        let entity = GtsEntity::new(
            None,
            None,
            &content,
            None,
            None,
            false,
            String::new(),
            None,
            None,
        );

        assert_eq!(entity.label, "");
    }

    #[test]
    fn test_validation_result_default() {
        let result = ValidationResult::default();
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validation_error_creation() {
        let mut params = std::collections::HashMap::new();
        params.insert("key".to_owned(), json!("value"));

        let error = ValidationError {
            instance_path: "/path".to_owned(),
            schema_path: "/schema".to_owned(),
            keyword: "required".to_owned(),
            message: "test error".to_owned(),
            params,
            data: Some(json!({"test": "data"})),
        };

        assert_eq!(error.instance_path, "/path");
        assert_eq!(error.message, "test error");
        assert!(error.data.is_some());
    }

    #[test]
    fn test_gts_config_entity_id_fields() {
        let cfg = GtsConfig::default();
        assert!(cfg.entity_id_fields.contains(&"id".to_owned()));
        assert!(cfg.entity_id_fields.contains(&"$id".to_owned()));
        assert!(cfg.entity_id_fields.contains(&"gtsId".to_owned()));
    }

    #[test]
    fn test_gts_config_schema_id_fields() {
        let cfg = GtsConfig::default();
        assert!(cfg.schema_id_fields.contains(&"type".to_owned()));
        assert!(cfg.schema_id_fields.contains(&"$schema".to_owned()));
        assert!(cfg.schema_id_fields.contains(&"gtsTid".to_owned()));
    }

    #[test]
    fn test_json_entity_with_validation_result() {
        let content = json!({"id": "gts.vendor.package.namespace.type.v1.0"});

        let mut validation = ValidationResult::default();
        validation.errors.push(ValidationError {
            instance_path: "/test".to_owned(),
            schema_path: "/schema/test".to_owned(),
            keyword: "type".to_owned(),
            message: "validation error".to_owned(),
            params: std::collections::HashMap::new(),
            data: None,
        });

        let entity = GtsEntity::new(
            None,
            None,
            &content,
            None,
            None,
            false,
            String::new(),
            Some(validation.clone()),
            None,
        );

        assert_eq!(entity.validation.errors.len(), 1);
    }

    #[test]
    fn test_json_entity_schema_id_field_selection() {
        let content = json!({
            "id": "gts.vendor.package.namespace.type.v1.0~instance.v1.0",
            "type": "gts.vendor.package.namespace.type.v1.0~"
        });

        let cfg = GtsConfig::default();
        let entity = GtsEntity::new(
            None,
            None,
            &content,
            Some(&cfg),
            None,
            false,
            String::new(),
            None,
            None,
        );

        assert!(entity.selected_schema_id_field.is_some());
    }

    #[test]
    fn test_json_entity_when_id_is_schema() {
        let content = json!({
            "id": "gts.vendor.package.namespace.type.v1.0~",
            "$schema": "http://json-schema.org/draft-07/schema#"
        });

        let cfg = GtsConfig::default();
        let entity = GtsEntity::new(
            None,
            None,
            &content,
            Some(&cfg),
            None,
            false,
            String::new(),
            None,
            None,
        );

        // When entity ID itself is a schema, selected_schema_id_field should be set to $schema
        assert_eq!(entity.selected_schema_id_field, Some("$schema".to_owned()));
    }
}
