//! Test: Base struct missing required ID property (id, gts_id, or gtsId)

use gts_macros::struct_to_gts_schema;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[struct_to_gts_schema(
    dir_path = "schemas",
    base = true,
    schema_id = "gts.x.core.events.topic.v1~",
    description = "Base topic type definition",
    properties = "name,description"
)]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TopicV1<P> {
    pub name: String,
    pub description: Option<String>,
    pub config: P,
}

fn main() {}
