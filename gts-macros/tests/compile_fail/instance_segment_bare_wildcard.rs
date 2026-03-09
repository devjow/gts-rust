//! Test: id with bare wildcard '*' as instance segment in gts_well_known_instance

use gts_macros::gts_well_known_instance;

#[gts_well_known_instance(
    dir_path = "instances",
    id = "gts.x.core.events.topic.v1~*"
)]
const ORDERS_TOPIC: &str = r#"{"name": "orders"}"#;

fn main() {}
