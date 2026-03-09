//! Test: id without ~ separator in gts_well_known_instance

use gts_macros::gts_well_known_instance;

#[gts_well_known_instance(
    dir_path = "instances",
    id = "gts.x.core.events.topic.v1.x.commerce._.orders.v1.0"
)]
const ORDERS_TOPIC: &str = r#"{"name": "orders"}"#;

fn main() {}
