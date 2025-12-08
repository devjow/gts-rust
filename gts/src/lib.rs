pub mod entities;
pub mod files_reader;
pub mod gts;
pub mod ops;
pub mod path_resolver;
pub mod schema_cast;
pub mod store;
pub mod x_gts_ref;

// Test modules - add any new *_tests.rs files here
#[cfg(test)]
mod entities_tests;

#[cfg(test)]
mod gts_tests;

#[cfg(test)]
mod ops_tests;

#[cfg(test)]
mod schema_cast_tests;

#[cfg(test)]
mod store_tests;

// Re-export commonly used types
pub use entities::{GtsConfig, GtsEntity, GtsFile, ValidationError, ValidationResult};
pub use files_reader::GtsFileReader;
pub use gts::{GtsError, GtsID, GtsIdSegment, GtsWildcard};
pub use ops::GtsOps;
pub use path_resolver::JsonPathResolver;
pub use schema_cast::{GtsEntityCastResult, SchemaCastError};
pub use store::{GtsReader, GtsStore, GtsStoreQueryResult, StoreError};
pub use x_gts_ref::{XGtsRefValidationError, XGtsRefValidator};
