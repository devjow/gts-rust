use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

use crate::entities::GtsEntity;
use crate::gts::{GTS_URI_PREFIX, GtsID, GtsWildcard};
use crate::schema_cast::GtsEntityCastResult;

/// Custom retriever for resolving gts:// URI scheme references in JSON Schema validation
struct GtsRetriever {
    store: Arc<RwLock<HashMap<String, Value>>>,
}

impl GtsRetriever {
    fn new(store_map: &HashMap<String, GtsEntity>) -> Self {
        let mut schemas = HashMap::new();

        // Pre-populate with all schemas from the store
        for (id, entity) in store_map {
            if entity.is_schema {
                // Store with gts:// URI format
                let uri = format!("{GTS_URI_PREFIX}{id}");
                schemas.insert(uri, entity.content.clone());
            }
        }

        Self {
            store: Arc::new(RwLock::new(schemas)),
        }
    }
}

impl jsonschema::Retrieve for GtsRetriever {
    #[allow(clippy::cognitive_complexity)]
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let uri_str = uri.as_str();

        tracing::debug!("GtsRetriever: Attempting to retrieve URI: {uri_str}");

        // Only handle gts:// URIs
        if !uri_str.starts_with(GTS_URI_PREFIX) {
            tracing::warn!("GtsRetriever: Unknown scheme for URI: {uri_str}");
            return Err(format!("Unknown scheme for URI: {uri_str}").into());
        }

        let store = self.store.read().map_err(|e| format!("Lock error: {e}"))?;

        tracing::debug!("GtsRetriever: Store contains {} schemas", store.len());

        if let Some(schema) = store.get(uri_str) {
            tracing::debug!("GtsRetriever: Successfully retrieved schema for {uri_str}");
            Ok(schema.clone())
        } else {
            tracing::warn!("GtsRetriever: Schema not found: {uri_str}");
            tracing::debug!(
                "GtsRetriever: Available URIs: {:?}",
                store.keys().collect::<Vec<_>>()
            );
            Err(format!("Schema not found: {uri_str}").into())
        }
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("JSON object with GTS ID '{0}' not found in store")]
    ObjectNotFound(String),
    #[error("JSON schema with GTS ID '{0}' not found in store")]
    SchemaNotFound(String),
    #[error("JSON entity with GTS ID '{0}' not found in store")]
    EntityNotFound(String),
    #[error("Can't determine JSON schema ID for instance with GTS ID '{0}'")]
    SchemaForInstanceNotFound(String),
    #[error(
        "Cannot cast from schema ID '{0}'. The from_id must be an instance (not ending with '~')"
    )]
    CastFromSchemaNotAllowed(String),
    #[error("Entity must have a valid gts_id")]
    InvalidEntity,
    #[error("Schema type_id must end with '~'")]
    InvalidSchemaId,
    #[error("{0}")]
    ValidationError(String),
    #[error("Invalid $ref: {0}")]
    InvalidRef(String),
}

pub trait GtsReader: Send {
    fn iter(&mut self) -> Box<dyn Iterator<Item = GtsEntity> + '_>;
    fn read_by_id(&self, entity_id: &str) -> Option<GtsEntity>;
    fn reset(&mut self);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GtsStoreQueryResult {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
    pub count: usize,
    pub limit: usize,
    pub results: Vec<Value>,
}

pub struct GtsStore {
    by_id: HashMap<String, GtsEntity>,
    reader: Option<Box<dyn GtsReader>>,
}

impl GtsStore {
    pub fn new(reader: Option<Box<dyn GtsReader>>) -> Self {
        let mut store = GtsStore {
            by_id: HashMap::new(),
            reader,
        };

        if store.reader.is_some() {
            store.populate_from_reader();
        }

        tracing::info!("Populated GtsStore with {} entities", store.by_id.len());
        store
    }

    fn populate_from_reader(&mut self) {
        if let Some(ref mut reader) = self.reader {
            for entity in reader.iter() {
                // Use effective_id() which handles both GTS IDs and anonymous instance IDs
                if let Some(id) = entity.effective_id() {
                    self.by_id.insert(id, entity);
                }
            }
        }
    }

    /// Registers an entity in the store.
    ///
    /// # Errors
    /// Returns `StoreError::InvalidEntity` if the entity has no effective ID.
    pub fn register(&mut self, entity: GtsEntity) -> Result<(), StoreError> {
        let id = entity.effective_id().ok_or(StoreError::InvalidEntity)?;
        self.by_id.insert(id, entity);
        Ok(())
    }

    /// Registers a schema in the store.
    ///
    /// # Errors
    /// Returns `StoreError::InvalidSchemaId` if the `type_id` doesn't end with '~'.
    pub fn register_schema(&mut self, type_id: &str, schema: &Value) -> Result<(), StoreError> {
        if !type_id.ends_with('~') {
            return Err(StoreError::InvalidSchemaId);
        }

        let gts_id = GtsID::new(type_id).map_err(|_| StoreError::InvalidSchemaId)?;
        let entity = GtsEntity::new(
            None,
            None,
            schema,
            None,
            Some(gts_id),
            true,
            String::new(),
            None,
            None,
        );
        self.by_id.insert(type_id.to_owned(), entity);
        Ok(())
    }

    pub fn get(&mut self, entity_id: &str) -> Option<&GtsEntity> {
        // Check cache first
        if self.by_id.contains_key(entity_id) {
            return self.by_id.get(entity_id);
        }

        // Try to fetch from reader
        if let Some(ref reader) = self.reader
            && let Some(entity) = reader.read_by_id(entity_id)
        {
            self.by_id.insert(entity_id.to_owned(), entity);
            return self.by_id.get(entity_id);
        }

        None
    }

    /// Gets the content of a schema by its type ID.
    ///
    /// # Errors
    /// Returns `StoreError::SchemaNotFound` if the schema is not found.
    pub fn get_schema_content(&mut self, type_id: &str) -> Result<Value, StoreError> {
        if let Some(entity) = self.get(type_id) {
            return Ok(entity.content.clone());
        }
        Err(StoreError::SchemaNotFound(type_id.to_owned()))
    }

    pub fn items(&self) -> impl Iterator<Item = (&String, &GtsEntity)> {
        self.by_id.iter()
    }

    /// Resolve all `$ref` references in a JSON Schema by inlining the referenced schemas.
    ///
    /// This method recursively traverses the schema, finds all `$ref` references,
    /// and replaces them with the actual schema content from the store. The result
    /// is a fully inlined schema with no external references.
    ///
    /// # Arguments
    ///
    /// * `schema` - The JSON Schema value that may contain `$ref` references
    ///
    /// # Returns
    ///
    /// A new `serde_json::Value` with all `$ref` references resolved and inlined.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gts::GtsStore;
    /// let store = GtsStore::new();
    ///
    /// // Add schemas to store
    /// store.add_schema_json("parent.v1~", parent_schema)?;
    /// store.add_schema_json("child.v1~", child_schema_with_ref)?;
    ///
    /// // Resolve references
    /// let inlined = store.resolve_schema_refs(&child_schema_with_ref);
    /// assert!(!inlined.to_string().contains("$ref"));
    /// ```
    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    pub fn resolve_schema_refs(&self, schema: &Value) -> Value {
        // Recursively resolve $ref references in the schema
        match schema {
            Value::Object(map) => {
                if let Some(Value::String(ref_uri)) = map.get("$ref") {
                    // Handle internal JSON Schema references like #/$defs/GtsInstanceId
                    // These should be inlined to match schemars 0.8 behavior (is_referenceable=false)
                    match ref_uri.as_str() {
                        "#/$defs/GtsInstanceId" => {
                            return crate::GtsInstanceId::json_schema_value();
                        }
                        "#/$defs/GtsSchemaId" => {
                            return crate::GtsSchemaId::json_schema_value();
                        }
                        s if s.starts_with("#/") => {
                            // Other internal references - keep as-is
                            let mut new_map = serde_json::Map::new();
                            for (k, v) in map {
                                new_map.insert(k.clone(), self.resolve_schema_refs(v));
                            }
                            return Value::Object(new_map);
                        }
                        _ => {} // Fall through to external ref handling
                    }

                    // Normalize the ref: strip gts:// prefix to get canonical GTS ID
                    let canonical_ref = ref_uri.strip_prefix(GTS_URI_PREFIX).unwrap_or(ref_uri);

                    // Try to resolve the reference using canonical ID
                    if let Some(entity) = self.by_id.get(canonical_ref)
                        && entity.is_schema
                    {
                        // Recursively resolve refs in the referenced schema
                        let mut resolved = self.resolve_schema_refs(&entity.content);

                        // Remove $id and $schema from resolved content to avoid URL resolution issues
                        // Note: $defs for GtsInstanceId/GtsSchemaId are inlined during resolution (see match above)
                        if let Value::Object(ref mut resolved_map) = resolved {
                            resolved_map.remove("$id");
                            resolved_map.remove("$schema");
                        }

                        // If the original object has only $ref, return the resolved schema
                        if map.len() == 1 {
                            return resolved;
                        }

                        // Otherwise, merge the resolved schema with other properties
                        if let Value::Object(resolved_map) = resolved {
                            let mut merged = resolved_map;
                            for (k, v) in map {
                                if k != "$ref" {
                                    merged.insert(k.clone(), self.resolve_schema_refs(v));
                                }
                            }
                            return Value::Object(merged);
                        }
                    }
                    // If we can't resolve, remove the $ref to avoid "relative URL" errors
                    // and keep other properties
                    let mut new_map = serde_json::Map::new();
                    for (k, v) in map {
                        if k != "$ref" {
                            new_map.insert(k.clone(), self.resolve_schema_refs(v));
                        }
                    }
                    if !new_map.is_empty() {
                        return Value::Object(new_map);
                    }
                    return schema.clone();
                }

                // Special handling for allOf arrays - merge $ref resolved schemas
                if let Some(Value::Array(all_of_array)) = map.get("allOf") {
                    let mut resolved_all_of = Vec::new();
                    let mut merged_properties = serde_json::Map::new();
                    let mut merged_required: Vec<String> = Vec::new();

                    for item in all_of_array {
                        let resolved_item = self.resolve_schema_refs(item);

                        match resolved_item {
                            Value::Object(ref item_map) => {
                                // If this item still has a $ref, keep it in allOf
                                if item_map.contains_key("$ref") {
                                    resolved_all_of.push(resolved_item);
                                } else {
                                    // Merge properties and required fields from resolved items
                                    if let Some(Value::Object(props_map)) =
                                        item_map.get("properties")
                                    {
                                        for (k, v) in props_map {
                                            merged_properties.insert(k.clone(), v.clone());
                                        }
                                    }
                                    if let Some(Value::Array(req_array)) = item_map.get("required")
                                    {
                                        for v in req_array {
                                            if let Value::String(s) = v
                                                && !merged_required.contains(s)
                                            {
                                                merged_required.push(s.to_owned());
                                            }
                                        }
                                    }
                                }
                            }
                            _ => resolved_all_of.push(resolved_item),
                        }
                    }

                    // If we have merged properties, create a single schema instead of allOf
                    if !merged_properties.is_empty() {
                        let mut merged_schema = serde_json::Map::new();

                        // Copy all properties except allOf
                        for (k, v) in map {
                            if k != "allOf" {
                                merged_schema.insert(k.clone(), v.clone());
                            }
                        }

                        // Add merged properties and required fields
                        merged_schema
                            .insert("properties".to_owned(), Value::Object(merged_properties));
                        if !merged_required.is_empty() {
                            merged_schema.insert(
                                "required".to_owned(),
                                Value::Array(
                                    merged_required.into_iter().map(Value::String).collect(),
                                ),
                            );
                        }

                        return Value::Object(merged_schema);
                    }
                }

                // Recursively process all properties
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k.clone(), self.resolve_schema_refs(v));
                }
                Value::Object(new_map)
            }
            Value::Array(arr) => {
                Value::Array(arr.iter().map(|v| self.resolve_schema_refs(v)).collect())
            }
            _ => schema.clone(),
        }
    }

    fn remove_x_gts_ref_fields(schema: &Value) -> Value {
        // Recursively remove x-gts-ref fields from a schema
        // This is needed because the jsonschema crate doesn't understand x-gts-ref
        // and will fail on JSON Pointer references like "/$id"
        match schema {
            Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (key, value) in map {
                    if key == "x-gts-ref" {
                        continue; // Skip x-gts-ref fields
                    }
                    new_map.insert(key.clone(), Self::remove_x_gts_ref_fields(value));
                }
                Value::Object(new_map)
            }
            Value::Array(arr) => {
                Value::Array(arr.iter().map(Self::remove_x_gts_ref_fields).collect())
            }
            _ => schema.clone(),
        }
    }

    fn validate_schema_x_gts_refs(&mut self, gts_id: &str) -> Result<(), StoreError> {
        if !gts_id.ends_with('~') {
            return Err(StoreError::SchemaNotFound(format!(
                "ID '{gts_id}' is not a schema (must end with '~')"
            )));
        }

        let schema_entity = self
            .get(gts_id)
            .ok_or_else(|| StoreError::SchemaNotFound(gts_id.to_owned()))?;

        if !schema_entity.is_schema {
            return Err(StoreError::SchemaNotFound(format!(
                "Entity '{gts_id}' is not a schema"
            )));
        }

        tracing::info!("Validating schema x-gts-ref fields for {}", gts_id);

        // Validate x-gts-ref constraints in the schema
        let validator = crate::x_gts_ref::XGtsRefValidator::new();
        let x_gts_ref_errors = validator.validate_schema(&schema_entity.content, "", None);

        if !x_gts_ref_errors.is_empty() {
            let error_messages: Vec<String> = x_gts_ref_errors
                .iter()
                .map(|err| {
                    if err.field_path.is_empty() {
                        err.reason.clone()
                    } else {
                        format!("{}: {}", err.field_path, err.reason)
                    }
                })
                .collect();
            let error_message =
                format!("x-gts-ref validation failed: {}", error_messages.join("; "));
            return Err(StoreError::ValidationError(error_message));
        }

        Ok(())
    }

    /// Validates all `$ref` values in a schema.
    ///
    /// Rules:
    /// - Local refs (starting with `#`) are always valid
    /// - External refs must use `gts://` URI format
    /// - The GTS ID after `gts://` must be a valid GTS identifier
    ///
    /// # Errors
    /// Returns `StoreError::InvalidRef` if any `$ref` is invalid.
    fn validate_schema_refs(schema: &Value, path: &str) -> Result<(), StoreError> {
        match schema {
            Value::Object(map) => {
                // Check $ref if present
                if let Some(Value::String(ref_uri)) = map.get("$ref") {
                    let current_path = if path.is_empty() {
                        "$ref".to_owned()
                    } else {
                        format!("{path}.$ref")
                    };

                    // Local refs (JSON Pointer) are always valid
                    if ref_uri.starts_with('#') {
                        // Valid local ref
                    }
                    // GTS refs must use gts:// URI format
                    else if let Some(gts_id) = ref_uri.strip_prefix(GTS_URI_PREFIX) {
                        // Validate the GTS ID
                        if !GtsID::is_valid(gts_id) {
                            return Err(StoreError::InvalidRef(format!(
                                "at '{current_path}': '{ref_uri}' contains invalid GTS identifier '{gts_id}'"
                            )));
                        }
                    }
                    // Any other external ref is invalid
                    else {
                        return Err(StoreError::InvalidRef(format!(
                            "at '{current_path}': '{ref_uri}' must be a local ref (starting with '#') \
                             or a GTS URI (starting with 'gts://')"
                        )));
                    }
                }

                // Recursively validate nested objects
                for (key, value) in map {
                    if key == "$ref" {
                        continue; // Already validated above
                    }
                    let nested_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    Self::validate_schema_refs(value, &nested_path)?;
                }
            }
            Value::Array(arr) => {
                for (idx, item) in arr.iter().enumerate() {
                    let nested_path = format!("{path}[{idx}]");
                    Self::validate_schema_refs(item, &nested_path)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Validates a schema against JSON Schema meta-schema and x-gts-ref constraints.
    ///
    /// # Errors
    /// Returns `StoreError` if validation fails.
    pub fn validate_schema(&mut self, gts_id: &str) -> Result<(), StoreError> {
        if !gts_id.ends_with('~') {
            return Err(StoreError::SchemaNotFound(format!(
                "ID '{gts_id}' is not a schema (must end with '~')"
            )));
        }

        let schema_entity = self
            .get(gts_id)
            .ok_or_else(|| StoreError::SchemaNotFound(gts_id.to_owned()))?;

        if !schema_entity.is_schema {
            return Err(StoreError::SchemaNotFound(format!(
                "Entity '{gts_id}' is not a schema"
            )));
        }

        let schema_content = schema_entity.content.clone();
        if !schema_content.is_object() {
            return Err(StoreError::SchemaNotFound(format!(
                "Schema '{gts_id}' content must be a dictionary"
            )));
        }

        tracing::info!("Validating schema {}", gts_id);

        // 1. Validate $ref fields - must be local (#...) or gts:// URIs
        Self::validate_schema_refs(&schema_content, "")?;

        // 2. Validate x-gts-ref fields (before JSON Schema validation)
        // This ensures we catch invalid GTS IDs in x-gts-ref before the JSON Schema
        // compiler potentially fails on them
        self.validate_schema_x_gts_refs(gts_id)?;

        // 3. Validate against JSON Schema meta-schema
        // We need to remove x-gts-ref fields before compiling because the jsonschema
        // crate doesn't understand them and will fail on JSON Pointer references
        let mut schema_for_validation = Self::remove_x_gts_ref_fields(&schema_content);

        // Check if schema contains gts:// references
        let has_gts_refs = schema_for_validation.to_string().contains("gts://");

        if has_gts_refs {
            // Skip jsonschema compilation for schemas with gts:// references during registration
            // This allows forward references (schemas referencing other schemas that don't exist yet)
            // Full validation with reference resolution will happen during instance validation
            tracing::debug!(
                "Schema {} contains gts:// references, skipping compilation during registration",
                gts_id
            );
        } else {
            // For schemas without gts:// references, validate the structure
            // Remove $id and $schema to avoid URL resolution issues
            if let Value::Object(ref mut map) = schema_for_validation {
                map.remove("$id");
                map.remove("$schema");
            }

            jsonschema::validator_for(&schema_for_validation).map_err(|e| {
                StoreError::ValidationError(format!(
                    "JSON Schema validation failed for '{gts_id}': {e}"
                ))
            })?;
        }

        tracing::info!(
            "Schema {} passed JSON Schema meta-schema validation",
            gts_id
        );

        Ok(())
    }

    /// Validates a chained schema ID by checking each derived schema against its base.
    ///
    /// For a chained ID like `gts.A~B~C~`, validates:
    /// - B (derived from A) is compatible with A
    /// - C (derived from A~B) is compatible with A~B
    ///
    /// The heavy lifting is delegated to [`crate::schema_compat`].
    ///
    /// # Errors
    /// Returns `StoreError::ValidationError` if any derived schema loosens base constraints.
    pub(crate) fn validate_schema_chain(&mut self, gts_id: &str) -> Result<(), StoreError> {
        let gid = GtsID::new(gts_id)
            .map_err(|e| StoreError::ValidationError(format!("Invalid GTS ID: {e}")))?;

        // Single-segment schemas have no parent to validate against
        if gid.gts_id_segments.len() < 2 {
            return Ok(());
        }

        // Build pairs of (base_id, derived_id) for each adjacent level
        // Note: segment.segment already includes the trailing '~' for type segments
        let segments = &gid.gts_id_segments;
        for i in 0..segments.len() - 1 {
            let base_id = format!(
                "gts.{}",
                segments[..=i]
                    .iter()
                    .map(|s| s.segment.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            );
            let derived_id = format!(
                "gts.{}",
                segments[..=i + 1]
                    .iter()
                    .map(|s| s.segment.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            );

            tracing::info!(
                "OP#12: Validating schema chain pair: base={} derived={}",
                base_id,
                derived_id
            );

            // Get and resolve both schemas
            let base_content = self.get_schema_content(&base_id).map_err(|_| {
                StoreError::ValidationError(format!(
                    "Base schema '{base_id}' not found for chain validation"
                ))
            })?;
            let derived_content = self.get_schema_content(&derived_id).map_err(|_| {
                StoreError::ValidationError(format!(
                    "Derived schema '{derived_id}' not found for chain validation"
                ))
            })?;

            let base_resolved = self.resolve_schema_refs(&base_content);
            let derived_resolved = self.resolve_schema_refs(&derived_content);

            // Extract effective schemas and compare via schema_compat module
            let base_eff = crate::schema_compat::extract_effective_schema(&base_resolved);
            let derived_eff = crate::schema_compat::extract_effective_schema(&derived_resolved);

            let errors = crate::schema_compat::validate_schema_compatibility(
                &base_eff,
                &derived_eff,
                &base_id,
                &derived_id,
            );

            if !errors.is_empty() {
                return Err(StoreError::ValidationError(format!(
                    "Schema '{}' is not compatible with base '{}': {}",
                    derived_id,
                    base_id,
                    errors.join("; ")
                )));
            }
        }

        Ok(())
    }

    /// Validates an instance against its schema.
    ///
    /// # Errors
    /// Returns `StoreError` if validation fails.
    pub fn validate_instance(&mut self, gts_id: &str) -> Result<(), StoreError> {
        let gid = GtsID::new(gts_id).map_err(|_| StoreError::ObjectNotFound(gts_id.to_owned()))?;

        let obj = self
            .get(&gid.id)
            .ok_or_else(|| StoreError::ObjectNotFound(gts_id.to_owned()))?
            .clone();

        let schema_id = obj
            .schema_id
            .as_ref()
            .ok_or_else(|| StoreError::SchemaForInstanceNotFound(gid.id.clone()))?
            .clone();

        let schema = self.get_schema_content(&schema_id)?;

        tracing::info!(
            "Validating instance {} against schema {}",
            gts_id,
            schema_id
        );

        // Resolve internal #/ references (like #/$defs/GtsInstanceId) by inlining them
        // This handles the compile-time inlining of GtsInstanceId and GtsSchemaId
        let schema_with_internal_refs_resolved = self.resolve_schema_refs(&schema);

        tracing::debug!(
            "Schema for validation: {}",
            serde_json::to_string_pretty(&schema_with_internal_refs_resolved).unwrap_or_default()
        );

        // Create custom retriever for gts:// URI resolution
        let retriever = GtsRetriever::new(&self.by_id);

        // Build validator with custom retriever to handle gts:// references
        // Internal #/ references have already been resolved by resolve_schema_refs
        // The retriever will resolve any $ref to gts:// URIs automatically
        let validator = jsonschema::options()
            .with_retriever(retriever)
            .build(&schema_with_internal_refs_resolved)
            .map_err(|e| {
                tracing::error!("Schema compilation error: {}", e);
                StoreError::ValidationError(format!(
                    "Invalid schema: {e}\nContent: {}\nSchema: {}",
                    serde_json::to_string_pretty(&obj.content).unwrap_or_default(),
                    serde_json::to_string_pretty(&schema_with_internal_refs_resolved)
                        .unwrap_or_default()
                ))
            })?;

        validator.validate(&obj.content).map_err(|_| {
            let errors: Vec<String> = validator
                .iter_errors(&obj.content)
                .map(|err| err.to_string())
                .collect();
            StoreError::ValidationError(format!("Validation failed: {}", errors.join(", ")))
        })?;

        // Validate x-gts-ref constraints
        let validator = crate::x_gts_ref::XGtsRefValidator::new();
        let x_gts_ref_errors = validator.validate_instance(&obj.content, &schema, "");

        if !x_gts_ref_errors.is_empty() {
            let error_messages: Vec<String> = x_gts_ref_errors
                .iter()
                .map(|err| {
                    if err.field_path.is_empty() {
                        err.reason.clone()
                    } else {
                        format!("{}: {}", err.field_path, err.reason)
                    }
                })
                .collect();
            let error_message =
                format!("x-gts-ref validation failed: {}", error_messages.join("; "));
            return Err(StoreError::ValidationError(error_message));
        }

        Ok(())
    }

    /// Casts an entity from one schema to another.
    ///
    /// # Errors
    /// Returns `StoreError` if the cast fails.
    pub fn cast(
        &mut self,
        from_id: &str,
        target_schema_id: &str,
    ) -> Result<GtsEntityCastResult, StoreError> {
        let from_entity = self
            .get(from_id)
            .ok_or_else(|| StoreError::EntityNotFound(from_id.to_owned()))?
            .clone();

        if from_entity.is_schema {
            return Err(StoreError::CastFromSchemaNotAllowed(from_id.to_owned()));
        }

        let to_schema = self
            .get(target_schema_id)
            .ok_or_else(|| StoreError::ObjectNotFound(target_schema_id.to_owned()))?
            .clone();

        // Get the source schema
        let (from_schema, _from_schema_id) = if from_entity.is_schema {
            let id = from_entity
                .gts_id
                .as_ref()
                .ok_or(StoreError::InvalidEntity)?
                .id
                .clone();
            (from_entity.clone(), id)
        } else {
            let schema_id = from_entity
                .schema_id
                .as_ref()
                .ok_or_else(|| StoreError::SchemaForInstanceNotFound(from_id.to_owned()))?;
            let schema = self
                .get(schema_id)
                .ok_or_else(|| StoreError::ObjectNotFound(schema_id.clone()))?
                .clone();
            (schema, schema_id.clone())
        };

        // Create a resolver to handle $ref in schemas
        // TODO: Implement custom resolver
        let resolver = None;

        from_entity
            .cast(&to_schema, &from_schema, resolver)
            .map_err(|e| StoreError::SchemaNotFound(e.to_string()))
    }

    pub fn is_minor_compatible(
        &mut self,
        old_schema_id: &str,
        new_schema_id: &str,
    ) -> GtsEntityCastResult {
        let old_entity = self.get(old_schema_id).cloned();
        let new_entity = self.get(new_schema_id).cloned();

        let (Some(old_ent), Some(new_ent)) = (old_entity, new_entity) else {
            return GtsEntityCastResult {
                from_id: old_schema_id.to_owned(),
                to_id: new_schema_id.to_owned(),
                old: old_schema_id.to_owned(),
                new: new_schema_id.to_owned(),
                direction: "unknown".to_owned(),
                added_properties: Vec::new(),
                removed_properties: Vec::new(),
                changed_properties: Vec::new(),
                is_fully_compatible: false,
                is_backward_compatible: false,
                is_forward_compatible: false,
                incompatibility_reasons: vec!["Schema not found".to_owned()],
                backward_errors: vec!["Schema not found".to_owned()],
                forward_errors: vec!["Schema not found".to_owned()],
                casted_entity: None,
                error: None,
            };
        };

        let old_schema = &old_ent.content;
        let new_schema = &new_ent.content;

        // Use the cast method's compatibility checking logic
        let (is_backward, backward_errors) =
            GtsEntityCastResult::check_backward_compatibility(old_schema, new_schema);
        let (is_forward, forward_errors) =
            GtsEntityCastResult::check_forward_compatibility(old_schema, new_schema);

        // Determine direction
        let direction = GtsEntityCastResult::infer_direction(old_schema_id, new_schema_id);

        GtsEntityCastResult {
            from_id: old_schema_id.to_owned(),
            to_id: new_schema_id.to_owned(),
            old: old_schema_id.to_owned(),
            new: new_schema_id.to_owned(),
            direction,
            added_properties: Vec::new(),
            removed_properties: Vec::new(),
            changed_properties: Vec::new(),
            is_fully_compatible: is_backward && is_forward,
            is_backward_compatible: is_backward,
            is_forward_compatible: is_forward,
            incompatibility_reasons: Vec::new(),
            backward_errors,
            forward_errors,
            casted_entity: None,
            error: None,
        }
    }

    pub fn build_schema_graph(&mut self, gts_id: &str) -> Value {
        let mut seen_gts_ids = std::collections::HashSet::new();
        self.gts2node(gts_id, &mut seen_gts_ids)
    }

    fn gts2node(
        &mut self,
        gts_id: &str,
        seen_gts_ids: &mut std::collections::HashSet<String>,
    ) -> Value {
        let mut ret = serde_json::Map::new();
        ret.insert("id".to_owned(), Value::String(gts_id.to_owned()));

        if seen_gts_ids.contains(gts_id) {
            return Value::Object(ret);
        }

        seen_gts_ids.insert(gts_id.to_owned());

        // Clone the entity to avoid borrowing issues
        let entity_clone = self.get(gts_id).cloned();

        if let Some(entity) = entity_clone {
            let mut refs = serde_json::Map::new();

            // Collect ref IDs first to avoid borrow issues
            let ref_ids: Vec<_> = entity
                .gts_refs
                .iter()
                .filter(|r| {
                    r.id != gts_id
                        && !r.id.starts_with("http://json-schema.org")
                        && !r.id.starts_with("https://json-schema.org")
                })
                .map(|r| (r.source_path.clone(), r.id.clone()))
                .collect();

            for (source_path, ref_id) in ref_ids {
                refs.insert(source_path, self.gts2node(&ref_id, seen_gts_ids));
            }

            if !refs.is_empty() {
                ret.insert("refs".to_owned(), Value::Object(refs));
            }

            if let Some(ref schema_id) = entity.schema_id {
                if !schema_id.starts_with("http://json-schema.org")
                    && !schema_id.starts_with("https://json-schema.org")
                {
                    let schema_id_clone = schema_id.clone();
                    ret.insert(
                        "schema_id".to_owned(),
                        self.gts2node(&schema_id_clone, seen_gts_ids),
                    );
                }
            } else {
                let mut errors = ret
                    .get("errors")
                    .and_then(|e| e.as_array())
                    .cloned()
                    .unwrap_or_default();
                errors.push(Value::String("Schema not recognized".to_owned()));
                ret.insert("errors".to_owned(), Value::Array(errors));
            }
        } else {
            let mut errors = ret
                .get("errors")
                .and_then(|e| e.as_array())
                .cloned()
                .unwrap_or_default();
            errors.push(Value::String("Entity not found".to_owned()));
            ret.insert("errors".to_owned(), Value::Array(errors));
        }

        Value::Object(ret)
    }

    #[must_use]
    pub fn query(&self, expr: &str, limit: usize) -> GtsStoreQueryResult {
        let mut result = GtsStoreQueryResult {
            error: String::new(),
            count: 0,
            limit,
            results: Vec::new(),
        };

        // Parse the query expression
        let (base, _, filt) = expr.partition('[');
        let base_pattern = base.trim();
        let is_wildcard = base_pattern.contains('*');

        // Parse filters if present
        let filter_str = if filt.is_empty() {
            ""
        } else {
            filt.rsplit_once(']').map_or("", |x| x.0)
        };
        let filters = Self::parse_query_filters(filter_str);

        // Validate and create pattern
        let (wildcard_pattern, exact_gts_id, error) =
            Self::validate_query_pattern(base_pattern, is_wildcard);
        if !error.is_empty() {
            result.error = error;
            return result;
        }

        // Filter entities
        for entity in self.by_id.values() {
            if result.results.len() >= limit {
                break;
            }

            if !entity.content.is_object() {
                continue;
            }

            let Some(ref gts_id) = entity.gts_id else {
                continue;
            };

            // Check if ID matches the pattern
            if !Self::matches_id_pattern(
                gts_id,
                base_pattern,
                is_wildcard,
                wildcard_pattern.as_ref(),
                exact_gts_id.as_ref(),
            ) {
                continue;
            }

            // Check filters
            if !Self::matches_filters(&entity.content, &filters) {
                continue;
            }

            result.results.push(entity.content.clone());
        }

        result.count = result.results.len();
        result
    }

    fn parse_query_filters(filter_str: &str) -> HashMap<String, String> {
        let mut filters = HashMap::new();
        if filter_str.is_empty() {
            return filters;
        }

        let parts: Vec<&str> = filter_str.split(',').map(str::trim).collect();
        for part in parts {
            if let Some((k, v)) = part.split_once('=') {
                let v = v.trim().trim_matches('"').trim_matches('\'');
                filters.insert(k.trim().to_owned(), v.to_owned());
            }
        }

        filters
    }

    fn validate_query_pattern(
        base_pattern: &str,
        is_wildcard: bool,
    ) -> (Option<GtsWildcard>, Option<GtsID>, String) {
        if is_wildcard {
            if !base_pattern.ends_with(".*") && !base_pattern.ends_with("~*") {
                return (
                    None,
                    None,
                    "Invalid query: wildcard patterns must end with .* or ~*".to_owned(),
                );
            }
            match GtsWildcard::new(base_pattern) {
                Ok(pattern) => (Some(pattern), None, String::new()),
                Err(e) => (None, None, format!("Invalid query: {e}")),
            }
        } else {
            match GtsID::new(base_pattern) {
                Ok(gts_id) => {
                    if gts_id.gts_id_segments.is_empty() {
                        (
                            None,
                            None,
                            "Invalid query: GTS ID has no valid segments".to_owned(),
                        )
                    } else {
                        (None, Some(gts_id), String::new())
                    }
                }
                Err(e) => (None, None, format!("Invalid query: {e}")),
            }
        }
    }

    fn matches_id_pattern(
        entity_id: &GtsID,
        base_pattern: &str,
        is_wildcard: bool,
        wildcard_pattern: Option<&GtsWildcard>,
        exact_gts_id: Option<&GtsID>,
    ) -> bool {
        if is_wildcard && let Some(pattern) = wildcard_pattern {
            return entity_id.wildcard_match(pattern);
        }

        // For non-wildcard patterns, use wildcard_match to support version flexibility
        if let Some(_exact) = exact_gts_id {
            match GtsWildcard::new(base_pattern) {
                Ok(pattern_as_wildcard) => entity_id.wildcard_match(&pattern_as_wildcard),
                Err(_) => entity_id.id == base_pattern,
            }
        } else {
            entity_id.id == base_pattern
        }
    }

    fn matches_filters(entity_content: &Value, filters: &HashMap<String, String>) -> bool {
        if filters.is_empty() {
            return true;
        }

        if let Some(obj) = entity_content.as_object() {
            for (key, value) in filters {
                let entity_value = obj.get(key).map_or_else(String::new, ToString::to_string);

                // Support wildcard in filter values
                if value == "*" {
                    if entity_value.is_empty() || entity_value == "null" {
                        return false;
                    }
                } else if entity_value != format!("\"{value}\"") && entity_value != *value {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }
}

// Helper trait for string partitioning
trait StringPartition {
    fn partition(&self, delimiter: char) -> (&str, &str, &str);
}

impl StringPartition for str {
    fn partition(&self, delimiter: char) -> (&str, &str, &str) {
        if let Some(pos) = self.find(delimiter) {
            let (before, after_with_delim) = self.split_at(pos);
            let after = &after_with_delim[delimiter.len_utf8()..];
            (before, &after_with_delim[..delimiter.len_utf8()], after)
        } else {
            (self, "", "")
        }
    }
}
#[cfg(test)]
#[path = "store_test.rs"]
mod store_test;
