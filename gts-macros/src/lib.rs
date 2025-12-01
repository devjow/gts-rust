use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Data, DeriveInput, Fields, LitStr, Token, Type,
};

/// Arguments for the struct_to_gts_schema macro
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
                _ => {
                    return Err(syn::Error::new_spanned(
                        key,
                        "Unknown attribute. Expected: file_path, schema_id, description, or properties",
                    ))
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(GtsSchemaArgs {
            file_path: file_path.ok_or_else(|| input.error("Missing required attribute: file_path"))?,
            schema_id: schema_id.ok_or_else(|| input.error("Missing required attribute: schema_id"))?,
            description: description.ok_or_else(|| input.error("Missing required attribute: description"))?,
            properties: properties.ok_or_else(|| input.error("Missing required attribute: properties"))?,
        })
    }
}

/// Validate and mark a Rust struct for GTS schema generation.
///
/// This macro performs **compile-time validation only**. It does not generate schema files.
/// Use `cargo gts generate` to actually create the JSON Schema files.
///
/// # Arguments
///
/// * `file_path` - Path where the schema file will be generated (relative to crate root)
/// * `schema_id` - GTS identifier in format: gts.vendor.package.namespace.type.vMAJOR.MINOR~
/// * `description` - Human-readable description of the schema
/// * `properties` - Comma-separated list of struct fields to include in the schema
///
/// # Example
///
/// ```ignore
/// use gts_macros::struct_to_gts_schema;
///
/// #[struct_to_gts_schema(
///     file_path = "schemas/gts.x.myapp.entities.user.v1~.schema.json",
///     schema_id = "gts.x.myapp.entities.user.v1~",
///     description = "User entity",
///     properties = "id,email,name"
/// )]
/// struct User {
///     id: String,
///     email: String,
///     name: String,
///     internal_field: i32, // Not included in schema
/// }
/// ```
///
/// # Compile-Time Validation
///
/// The macro will cause a compile-time error if:
/// - Any property listed in `properties` doesn't exist in the struct
/// - Required attributes are missing (file_path, schema_id, description, properties)
/// - The struct is not a struct with named fields
///
/// # Generating Schemas
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
#[proc_macro_attribute]
pub fn struct_to_gts_schema(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as GtsSchemaArgs);
    let input = parse_macro_input!(item as DeriveInput);

    // Validate file_path ends with .json
    if !args.file_path.ends_with(".json") {
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
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Extract struct fields
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
            return syn::Error::new_spanned(&input.ident, "struct_to_gts_schema: Only structs are supported")
                .to_compile_error()
                .into()
        }
    };

    // Validate that all requested properties exist
    let available_fields: Vec<String> = struct_fields
        .iter()
        .map(|f| f.ident.as_ref().unwrap().to_string())
        .collect();

    for prop in &property_names {
        if !available_fields.contains(prop) {
            return syn::Error::new_spanned(
                &input.ident,
                format!(
                    "struct_to_gts_schema: Property '{}' not found in struct. Available fields: {:?}",
                    prop, available_fields
                ),
            )
            .to_compile_error()
            .into();
        }
    }

    // Build JSON schema properties
    let mut schema_properties = serde_json::Map::new();
    let mut required_fields = Vec::new();

    for field in struct_fields.iter() {
        let field_name = field.ident.as_ref().unwrap().to_string();

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
    let mut schema = serde_json::json!({
        "$id": args.schema_id,
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": input.ident.to_string(),
        "type": "object",
        "description": args.description,
        "properties": schema_properties
    });

    if !required_fields.is_empty() {
        schema["required"] = serde_json::json!(required_fields);
    }

    // Generate the schema JSON string
    let schema_json = serde_json::to_string_pretty(&schema).unwrap();
    let file_path = &args.file_path;
    let schema_id = &args.schema_id;
    let description = &args.description;

    // Generate metadata that can be accessed by code generation tools
    let struct_name = &input.ident;

    let expanded = quote! {
        #input

        // Embed schema metadata as compile-time constants
        // This allows external tools (like cargo-gts) to extract the information
        impl #struct_name {
            #[doc(hidden)]
            #[allow(dead_code)]
            pub const GTS_SCHEMA_FILE_PATH: &'static str = #file_path;

            #[doc(hidden)]
            #[allow(dead_code)]
            pub const GTS_SCHEMA_ID: &'static str = #schema_id;

            #[doc(hidden)]
            #[allow(dead_code)]
            pub const GTS_SCHEMA_DESCRIPTION: &'static str = #description;

            #[doc(hidden)]
            #[allow(dead_code)]
            pub const GTS_SCHEMA_JSON: &'static str = #schema_json;
        }
    };

    TokenStream::from(expanded)
}

/// Convert Rust types to JSON Schema types
/// Returns (is_required, json_type, format)
fn rust_type_to_json_schema(ty: &Type) -> (bool, &'static str, Option<&'static str>) {
    let type_str = quote!(#ty).to_string();
    let type_str = type_str.replace(" ", "");

    // Check if it's an Option type
    let is_optional = type_str.starts_with("Option<");
    let inner_type = if is_optional {
        type_str
            .strip_prefix("Option<")
            .and_then(|s| s.strip_suffix(">"))
            .unwrap_or(&type_str)
    } else {
        &type_str
    };

    let (json_type, format) = match inner_type {
        "String" | "str" | "&str" => ("string", None),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => ("integer", None),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => ("integer", None),
        "f32" | "f64" => ("number", None),
        "bool" => ("boolean", None),
        "Vec<String>" | "Vec<&str>" => ("array", None),
        t if t.starts_with("Vec<") => ("array", None),
        t if t.contains("Uuid") || t.contains("uuid") => ("string", Some("uuid")),
        _ => ("string", None), // Default to string for unknown types
    };

    (!is_optional, json_type, format)
}
