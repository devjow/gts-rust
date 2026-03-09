//! Test: gts_well_known_instance applied to a const with wrong type (not &str)

use gts_macros::gts_well_known_instance;

#[gts_well_known_instance(
    dir_path = "instances",
    id = "gts.x.core.events.topic.v1~x.commerce._.orders.v1.0"
)]
const ORDERS_TOPIC: u32 = 42;

fn main() {}
