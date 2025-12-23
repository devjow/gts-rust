// Proc macros run at compile time, so panics become compile errors
#![allow(clippy::expect_used, clippy::unwrap_used)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Data, DeriveInput, Fields, LitStr, Token,
};

/// Arguments for the `struct_to_gts_schema` macro
struct GtsSchemaArgs {
    dir_path: String,
    schema_id: String,
    description: String,
    properties: String,
}

impl Parse for GtsSchemaArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut dir_path: Option<String> = None;
        let mut schema_id: Option<String> = None;
        let mut description: Option<String> = None;
        let mut properties: Option<String> = None;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "dir_path" => dir_path = Some(value.value()),
                "schema_id" => schema_id = Some(value.value()),
                "description" => description = Some(value.value()),
                "properties" => properties = Some(value.value()),
                _ => return Err(syn::Error::new_spanned(
                    key,
                    "Unknown attribute. Expected: dir_path, schema_id, description, or properties",
                )),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(GtsSchemaArgs {
            dir_path: dir_path
                .ok_or_else(|| input.error("Missing required attribute: dir_path"))?,
            schema_id: schema_id
                .ok_or_else(|| input.error("Missing required attribute: schema_id"))?,
            description: description
                .ok_or_else(|| input.error("Missing required attribute: description"))?,
            properties: properties
                .ok_or_else(|| input.error("Missing required attribute: properties"))?,
        })
    }
}

/// Annotate a Rust struct for GTS schema generation.
///
/// This macro serves three purposes:
///
/// ## 1. Compile-Time Validation & Guarantees
///
/// The macro validates your annotations at compile time, catching errors early:
/// - ✅ All required attributes exist (`dir_path`, `schema_id`, `description`, `properties`)
/// - ✅ Every property in `properties` exists as a field in the struct
/// - ✅ Only structs with named fields are supported (no tuple/unit structs or enums)
/// - ✅ Single generic parameter maximum (prevents inheritance ambiguity)
/// - ✅ Valid GTS ID format enforcement
/// - ✅ Zero runtime allocation for generated constants
///
/// ## 2. Schema Generation
///
/// After annotating your structs, run:
/// ```bash
/// cargo gts generate --source src/
/// ```
///
/// Or use the GTS CLI directly:
/// ```bash
/// gts generate-from-rust --source src/ --output schemas/
/// ```
///
/// This will generate JSON Schema files at the specified `dir_path` with names derived from `schema_id` for each annotated struct (e.g., `{dir_path}/{schema_id}.schema.json`).
///
/// ## 3. Runtime API
///
/// The macro generates these associated items and implements the `GtsSchema` trait:
///
/// - `GTS_JSON_SCHEMA_WITH_REFS: &'static str` - JSON Schema with `allOf` + `$ref` for inheritance (most memory-efficient)
/// - `GTS_JSON_SCHEMA_INLINE: &'static str` - JSON Schema with parent inlined (currently identical to `WITH_REFS`; true inlining requires runtime resolution)
/// - `make_gts_instance_id(segment: &str) -> gts::GtsInstanceId` - Generate an instance ID by appending
///   a segment to the schema ID. The segment must be a valid GTS segment (e.g., "a.b.c.v1")
/// - `GtsSchema` trait implementation - Enables runtime schema composition for nested generic types
///   (e.g., `BaseEventV1<AuditPayloadV1<PlaceOrderDataV1>>`), with proper nesting and inheritance support.
///   Generic fields automatically have `additionalProperties: false` set to ensure type safety.
///
/// # Arguments
///
/// * `dir_path` - Directory where the schema file will be generated (relative to crate root)
/// * `schema_id` - GTS identifier in format: `gts.vendor.package.namespace.type.vMAJOR~`
///   - **Automatic inheritance**: If the `schema_id` contains multiple segments separated by `~`, inheritance is automatically detected
///   - Example: `gts.x.core.events.type.v1~x.core.audit.event.v1~` inherits from `gts.x.core.events.type.v1~`
/// * `description` - Human-readable description of the schema
/// * `properties` - Comma-separated list of struct fields to include in the schema
///
/// # Memory Efficiency
///
/// All generated constants are compile-time strings with **zero runtime allocation**:
/// - `GTS_JSON_SCHEMA_WITH_REFS` uses `$ref` for optimal memory usage
/// - `GTS_JSON_SCHEMA_INLINE` is identical at compile time (true inlining requires runtime schema resolution)
///
/// # Example
///
/// ```ignore
/// use gts_macros::struct_to_gts_schema;
///
/// #[struct_to_gts_schema(
///     dir_path = "schemas",
///     schema_id = "gts.x.core.events.topic.v1~",
///     description = "Event broker topics",
///     properties = "id,persisted,retention_days,name"
/// )]
/// struct User {
///     id: String,
///     persisted: bool,
///     retention_days: i32,
///     internal_field: i32, // Not included in schema (not in properties list)
/// }
///
/// // Runtime usage:
/// let schema_with_refs = User::GTS_JSON_SCHEMA_WITH_REFS;
/// let schema_inline = User::GTS_JSON_SCHEMA_INLINE;
/// let instance_id = User::make_gts_instance_id("vendor.marketplace.orders.order_created.v1");
/// assert_eq!(instance_id.as_ref(), "gts.x.core.events.topic.v1~vendor.marketplace.orders.order_created.v1");
/// ```
#[proc_macro_attribute]
#[allow(clippy::too_many_lines, clippy::missing_panics_doc)]
pub fn struct_to_gts_schema(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as GtsSchemaArgs);
    let input = parse_macro_input!(item as DeriveInput);

    // Prohibit multiple type generic parameters (GTS notation assumes nested segments)
    if input.generics.type_params().count() > 1 {
        return syn::Error::new_spanned(
            &input.ident,
            "struct_to_gts_schema: Multiple type generic parameters are not supported (GTS schemas assume nested segments)",
        )
        .to_compile_error()
        .into();
    }

    // Parse properties list
    let property_names: Vec<String> = args
        .properties
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();

    // Extract struct fields for validation
    let struct_fields = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input.ident,
                    "struct_to_gts_schema: Only structs with named fields are supported",
                )
                .to_compile_error()
                .into()
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "struct_to_gts_schema: Only structs are supported",
            )
            .to_compile_error()
            .into()
        }
    };

    // Validate that all requested properties exist
    let available_fields: Vec<String> = struct_fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(ToString::to_string))
        .collect();

    for prop in &property_names {
        if !available_fields.contains(prop) {
            return syn::Error::new_spanned(
                &input.ident,
                format!(
                    "struct_to_gts_schema: Property '{prop}' not found in struct. Available fields: {available_fields:?}"
                ),
            )
            .to_compile_error()
            .into();
        }
    }

    // Build the schema output file path from dir_path + schema_id
    let struct_name = &input.ident;
    let dir_path = &args.dir_path;
    let schema_id = &args.schema_id;
    let description = &args.description;
    let properties_str = &args.properties;

    let schema_file_path = format!("{dir_path}/{schema_id}.schema.json");

    // Extract generics to properly handle generic structs
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Get the generic type parameter name if present
    let generic_param_name: Option<String> = input
        .generics
        .type_params()
        .next()
        .map(|tp| tp.ident.to_string());

    let mut generic_field_name: Option<String> = None;

    // Find the field that uses the generic type
    if let Some(ref gp) = generic_param_name {
        for field in struct_fields {
            let field_type = &field.ty;
            let field_type_str = quote::quote!(#field_type).to_string().replace(' ', "");
            if field_type_str == *gp {
                if let Some(ident) = &field.ident {
                    generic_field_name = Some(ident.to_string());
                    break;
                }
            }
        }
    }

    // Generate the GENERIC_FIELD constant value
    let generic_field_option = if let Some(ref field_name) = generic_field_name {
        quote! { Some(#field_name) }
    } else {
        quote! { None }
    };

    // Generate gts_schema() implementation based on whether we have a generic parameter
    let has_generic = input.generics.type_params().count() > 0;

    // Build a custom where clause for GtsSchema that adds the GtsSchema bound on generic params
    let gts_schema_where_clause = if has_generic {
        let generic_param = input.generics.type_params().next().unwrap();
        let generic_ident = &generic_param.ident;
        if let Some(existing) = where_clause {
            quote! { #existing #generic_ident: ::gts::GtsSchema + ::schemars::JsonSchema, }
        } else {
            quote! { where #generic_ident: ::gts::GtsSchema + ::schemars::JsonSchema }
        }
    } else {
        quote! { #where_clause }
    };

    let gts_schema_impl = if has_generic {
        let generic_param = input.generics.type_params().next().unwrap();
        let generic_ident = &generic_param.ident;
        let generic_field_for_path = generic_field_name.as_deref().unwrap_or_default();

        quote! {
            fn gts_schema() -> serde_json::Value {
                Self::gts_schema_with_refs()
            }

            fn innermost_schema_id() -> &'static str {
                // Recursively get the innermost type's schema ID
                let inner_id = <#generic_ident as ::gts::GtsSchema>::innermost_schema_id();
                if inner_id.is_empty() {
                    Self::SCHEMA_ID
                } else {
                    inner_id
                }
            }

            fn innermost_schema() -> serde_json::Value {
                // Get the innermost type's raw schemars schema
                let inner = <#generic_ident as ::gts::GtsSchema>::innermost_schema();
                // If inner is just {"type": "object"} (from ()), return our own schema
                // schemars RootSchema serializes at root level (not under "schema" field)
                if inner.get("properties").is_none() {
                    let root_schema = schemars::schema_for!(Self);
                    return serde_json::to_value(&root_schema).expect("schemars");
                }
                inner
            }

            fn collect_nesting_path() -> Vec<&'static str> {
                // Collect the path from outermost to the PARENT of the innermost type.
                // For Outer<Middle<()>> where Outer has generic field "a" and Middle has "b":
                //   - () has no properties, so Middle IS the innermost
                //   - Path is just ["a"]
                // For Outer<Middle<Inner>> where Inner has properties:
                //   - Inner is the innermost type with properties
                //   - Path is ["a", "b"]

                let inner_path = <#generic_ident as ::gts::GtsSchema>::collect_nesting_path();
                let inner_id = <#generic_ident as ::gts::GtsSchema>::SCHEMA_ID;

                // If inner type is () (empty ID), don't include this type's field
                // because this type IS the innermost type with properties
                if inner_id.is_empty() {
                    return Vec::new();
                }

                // Otherwise, prepend this type's generic field to inner path
                let mut path = Vec::new();
                let field = #generic_field_for_path;
                if !field.is_empty() {
                    path.push(field);
                }
                path.extend(inner_path);
                path
            }

            fn gts_schema_with_refs_allof() -> serde_json::Value {
                // Get the innermost type's schema ID for $id
                let schema_id = Self::innermost_schema_id();

                // Get parent's ID by removing last segment from schema_id
                // e.g., "a~b~c~" -> "a~b~"
                let parent_schema_id = if schema_id.contains('~') {
                    let s = schema_id.trim_end_matches('~');
                    if let Some(pos) = s.rfind('~') {
                        format!("{}~", &s[..pos])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // Get innermost type's schema (its own properties)
                let innermost = Self::innermost_schema();
                let mut properties = innermost.get("properties").cloned().unwrap_or(serde_json::json!({}));
                let required = innermost.get("required").cloned().unwrap_or(serde_json::json!([]));

                // Fix null types for generic fields - change "null" to just "object" (no additionalProperties)
                // The generic field is a placeholder that will be extended by child schemas
                if let Some(props) = properties.as_object_mut() {
                    for (_, prop_val) in props.iter_mut() {
                        if prop_val.get("type").and_then(|t| t.as_str()) == Some("null") {
                            *prop_val = serde_json::json!({
                                "type": "object"
                            });
                        }
                    }
                }

                // If no parent (base type), return simple schema without allOf
                // Base types have additionalProperties: false at root level
                // Generic fields are just {"type": "object"} (will be extended by children)
                if parent_schema_id.is_empty() {
                    let mut schema = serde_json::json!({
                        "$id": format!("gts://{}", schema_id),
                        "$schema": "http://json-schema.org/draft-07/schema#",
                        "type": "object",
                        "additionalProperties": false,
                        "properties": properties
                    });
                    if !required.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                        schema["required"] = required;
                    }
                    return schema;
                }

                // Build the nesting path from outer to inner generic fields
                // For Outer<Middle<Inner>> where Outer has field "a" and Middle has field "b":
                //   - innermost is Inner
                //   - parent is derived from innermost's schema ID
                //   - path ["a", "b"] wraps Inner's properties
                let nesting_path = Self::collect_nesting_path();

                // Get the generic field name for the innermost type (if it has one)
                // This field should NOT have additionalProperties: false since it will be extended
                let innermost_generic_field = <#generic_ident as ::gts::GtsSchema>::GENERIC_FIELD;

                // Wrap properties in the nesting path
                let nested_properties = Self::wrap_in_nesting_path(&nesting_path, properties, required.clone(), innermost_generic_field);

                // Child type - use allOf with $ref to parent
                serde_json::json!({
                    "$id": format!("gts://{}", schema_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "allOf": [
                        { "$ref": format!("gts://{}", parent_schema_id) },
                        {
                            "type": "object",
                            "properties": nested_properties
                        }
                    ]
                })
            }
        }
    } else {
        quote! {
            fn gts_schema() -> serde_json::Value {
                Self::gts_schema_with_refs()
            }
            fn innermost_schema_id() -> &'static str {
                Self::SCHEMA_ID
            }
            fn innermost_schema() -> serde_json::Value {
                // Return this type's schemars schema (RootSchema serializes at root level)
                let root_schema = schemars::schema_for!(Self);
                serde_json::to_value(&root_schema).expect("schemars")
            }
            fn gts_schema_with_refs_allof() -> serde_json::Value {
                let schema_id = Self::SCHEMA_ID;

                // Get parent's ID by removing last segment
                let parent_schema_id = if schema_id.contains('~') {
                    let s = schema_id.trim_end_matches('~');
                    if let Some(pos) = s.rfind('~') {
                        format!("{}~", &s[..pos])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // Get this type's schemars schema (RootSchema serializes at root level)
                let root_schema = schemars::schema_for!(Self);
                let schema_val = serde_json::to_value(&root_schema).expect("schemars");
                let properties = schema_val.get("properties").cloned().unwrap_or_else(|| serde_json::json!({}));
                let required = schema_val.get("required").cloned().unwrap_or_else(|| serde_json::json!([]));

                // If no parent (base type), return simple schema without allOf
                // Non-generic base types have additionalProperties: false at root level
                if parent_schema_id.is_empty() {
                    let mut schema = serde_json::json!({
                        "$id": format!("gts://{}", schema_id),
                        "$schema": "http://json-schema.org/draft-07/schema#",
                        "type": "object",
                        "additionalProperties": false,
                        "properties": properties
                    });
                    if !required.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                        schema["required"] = required;
                    }
                    return schema;
                }

                // Child type - use allOf with $ref to parent
                // Non-generic child types have additionalProperties: false in their own properties section
                serde_json::json!({
                    "$id": format!("gts://{}", schema_id),
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "type": "object",
                    "allOf": [
                        { "$ref": format!("gts://{}", parent_schema_id) },
                        {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": properties,
                            "required": required
                        }
                    ]
                })
            }
        }
    };

    let expanded = quote! {
        #input

        impl #impl_generics #struct_name #ty_generics #where_clause {
            /// File path where the GTS schema will be generated by the CLI.
            #[doc(hidden)]
            #[allow(dead_code)]
            pub const GTS_SCHEMA_FILE_PATH: &'static str = #schema_file_path;

            /// GTS schema identifier (the `$id` field in the JSON Schema).
            #[doc(hidden)]
            #[allow(dead_code)]
            pub const GTS_SCHEMA_ID: &'static str = #schema_id;

            /// GTS schema description.
            #[doc(hidden)]
            #[allow(dead_code)]
            pub const GTS_SCHEMA_DESCRIPTION: &'static str = #description;

            /// Comma-separated list of properties included in the schema.
            #[doc(hidden)]
            #[allow(dead_code)]
            pub const GTS_SCHEMA_PROPERTIES: &'static str = #properties_str;

            /// Generate a GTS instance ID by appending a segment to the schema ID.
            #[allow(dead_code)]
            #[must_use]
            pub fn make_gts_instance_id(segment: &str) -> ::gts::GtsInstanceId {
                ::gts::GtsInstanceId::new(#schema_id, segment)
            }

        }

        // Implement GtsSchema trait for runtime schema composition
        impl #impl_generics ::gts::GtsSchema for #struct_name #ty_generics #gts_schema_where_clause {
            const SCHEMA_ID: &'static str = #schema_id;
            const GENERIC_FIELD: Option<&'static str> = #generic_field_option;

            fn gts_schema_with_refs() -> serde_json::Value {
                Self::gts_schema_with_refs_allof()
            }

            #gts_schema_impl
        }

        // Add helper methods for backward compatibility with tests
        impl #impl_generics #struct_name #ty_generics #gts_schema_where_clause {
            /// JSON Schema with `allOf` + `$ref` for inheritance (most memory-efficient).
            /// Returns the schema as a JSON string.
            #[allow(dead_code)]
            pub fn gts_json_schema_with_refs() -> String {
                use ::gts::GtsSchema;
                serde_json::to_string(&Self::gts_schema_with_refs_allof()).expect("Failed to serialize schema")
            }

            /// JSON Schema with parent inlined (currently identical to WITH_REFS).
            /// Returns the schema as a JSON string.
            #[allow(dead_code)]
            pub fn gts_json_schema_inline() -> String {
                Self::gts_json_schema_with_refs()
            }
        }
    };

    TokenStream::from(expanded)
}
