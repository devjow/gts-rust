// Proc macros run at compile time, so panics become compile errors
#![allow(clippy::expect_used, clippy::unwrap_used)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Data, DeriveInput, Fields, LitStr, Token, Type,
};

/// Arguments for the `struct_to_gts_schema` macro
struct GtsSchemaArgs {
    file_path: String,
    schema_id: String,
    description: String,
    properties: String,
}

impl Parse for GtsSchemaArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut file_path: Option<String> = None;
        let mut schema_id: Option<String> = None;
        let mut description: Option<String> = None;
        let mut properties: Option<String> = None;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "file_path" => file_path = Some(value.value()),
                "schema_id" => schema_id = Some(value.value()),
                "description" => description = Some(value.value()),
                "properties" => properties = Some(value.value()),
                _ => return Err(syn::Error::new_spanned(
                    key,
                    "Unknown attribute. Expected: file_path, schema_id, description, or properties",
                )),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(GtsSchemaArgs {
            file_path: file_path
                .ok_or_else(|| input.error("Missing required attribute: file_path"))?,
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
/// ## 1. Compile-Time Validation
///
/// The macro will cause a compile-time error if:
/// - Any property listed in `properties` doesn't exist in the struct
/// - Required attributes are missing (`file_path`, `schema_id`, `description`, `properties`)
/// - The struct is not a struct with named fields
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
/// This will generate JSON Schema files at the specified `file_path` for each annotated struct.
///
/// ## 3. Runtime API
///
/// The macro generates these associated items:
///
/// - `GTS_SCHEMA_JSON: &'static str` - The JSON Schema with `$id` set to `schema_id`
/// - `GTS_MAKE_INSTANCE_ID(segment: &str) -> String` - Generate an instance ID by appending
///   a segment to the schema ID. The segment must be a valid GTS segment (e.g., "a.b.c.v1")
///
/// # Arguments
///
/// * `file_path` - Path where the schema file will be generated (relative to crate root)
/// * `schema_id` - GTS identifier in format: `gts.vendor.package.namespace.type.vMAJOR~`
/// * `description` - Human-readable description of the schema
/// * `properties` - Comma-separated list of struct fields to include in the schema
///
/// # Example
///
/// ```ignore
/// use gts_macros::struct_to_gts_schema;
///
/// #[struct_to_gts_schema(
///     file_path = "schemas/gts.x.core.events.topic.v1~.schema.json",
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
/// let schema = User::GTS_SCHEMA_JSON;
/// let instance_id = User::GTS_MAKE_INSTANCE_ID("vendor.marketplace.orders.order_created.v1");
/// assert_eq!(instance_id, "gts.x.core.events.topic.v1~vendor.marketplace.orders.order_created.v1");
/// ```
#[proc_macro_attribute]
#[allow(clippy::too_many_lines, clippy::missing_panics_doc)]
pub fn struct_to_gts_schema(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as GtsSchemaArgs);
    let input = parse_macro_input!(item as DeriveInput);

    // Validate file_path ends with .json
    if !std::path::Path::new(&args.file_path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        return syn::Error::new_spanned(
            &input.ident,
            format!(
                "struct_to_gts_schema: file_path must end with '.json'. Got: '{}'",
                args.file_path
            ),
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

    // Build JSON schema properties at compile time
    let mut schema_properties = serde_json::Map::new();
    let mut required_fields = Vec::new();

    for field in struct_fields {
        let Some(ident) = field.ident.as_ref() else {
            continue;
        };
        let field_name = ident.to_string();

        if !property_names.contains(&field_name) {
            continue;
        }

        let field_type = &field.ty;
        let (is_required, json_type, format) = rust_type_to_json_schema(field_type);

        let mut prop = serde_json::json!({
            "type": json_type
        });

        if let Some(fmt) = format {
            prop["format"] = serde_json::json!(fmt);
        }

        schema_properties.insert(field_name.clone(), prop);

        if is_required {
            required_fields.push(field_name);
        }
    }

    // Build the complete schema
    // The $id uses the URI format "gts://gts.x.y.z..." for JSON Schema compatibility
    let struct_name = &input.ident;
    let schema_id_uri = format!("gts://{}", args.schema_id);
    let mut schema = serde_json::json!({
        "$id": schema_id_uri,
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": struct_name.to_string(),
        "type": "object",
        "description": args.description,
        "properties": schema_properties
    });

    if !required_fields.is_empty() {
        schema["required"] = serde_json::json!(required_fields);
    }

    // Generate the schema JSON string
    let schema_json =
        serde_json::to_string_pretty(&schema).expect("schema serialization should not fail");

    let file_path = &args.file_path;
    let schema_id = &args.schema_id;
    let description = &args.description;
    let properties_str = &args.properties;

    let expanded = quote! {
        #input

        impl #struct_name {
            /// File path where the GTS schema will be generated by the CLI.
            #[doc(hidden)]
            #[allow(dead_code)]
            pub const GTS_SCHEMA_FILE_PATH: &'static str = #file_path;

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

            /// The JSON Schema as a compile-time constant string.
            ///
            /// The `$id` field is set to the `schema_id` from the macro annotation.
            #[allow(dead_code)]
            pub const GTS_SCHEMA_JSON: &'static str = #schema_json;

            /// Generate a GTS instance ID by appending a segment to the schema ID.
            ///
            /// # Arguments
            ///
            /// * `segment` - A valid GTS segment to append (e.g., "a.b.c.v1", "instance.v1.0")
            ///
            /// # Returns
            ///
            /// A string in the format: `{schema_id}{segment}`
            ///
            /// # Example
            ///
            /// ```ignore
            /// let id = User::GTS_MAKE_INSTANCE_ID("123.v1");
            /// // Returns: "gts.x.myapp.entities.user.v1~123.v1"
            /// ```
            #[allow(dead_code)]
            #[must_use]
            pub fn GTS_MAKE_INSTANCE_ID(segment: &str) -> String {
                format!("{}{}", #schema_id, segment)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Convert Rust types to JSON Schema types at compile time.
/// Returns (`is_required`, `json_type`, `format`)
fn rust_type_to_json_schema(ty: &Type) -> (bool, &'static str, Option<&'static str>) {
    let type_str = quote::quote!(#ty).to_string();
    let type_str = type_str.replace(' ', "");

    // Check if it's an Option type
    let is_optional = type_str.starts_with("Option<");
    let inner_type = if is_optional {
        type_str
            .strip_prefix("Option<")
            .and_then(|s| s.strip_suffix('>'))
            .unwrap_or(&type_str)
    } else {
        &type_str
    };

    let (json_type, format) = match inner_type {
        "String" | "str" | "&str" => ("string", None),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => ("integer", None),
        "f32" | "f64" => ("number", None),
        "bool" => ("boolean", None),
        "Vec<String>" | "Vec<&str>" => ("array", None),
        t if t.starts_with("Vec<") => ("array", None),
        t if t.contains("Uuid") || t.contains("uuid") => ("string", Some("uuid")),
        t if t.contains("DateTime") || t.contains("NaiveDateTime") => ("string", Some("date-time")),
        t if t.contains("NaiveDate") => ("string", Some("date")),
        t if t.contains("NaiveTime") => ("string", Some("time")),
        t if t.starts_with("HashMap<") || t.starts_with("BTreeMap<") => ("object", None),
        _ => ("string", None), // Default to string for unknown types
    };

    (!is_optional, json_type, format)
}
