//! The Wellness Lab engine: every number the app shows. NaN is a missing value inside the engine.
#![allow(
    clippy::needless_range_loop,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::manual_range_contains
)]

pub mod baseline;
pub mod beats;
pub mod data;
pub mod dfa;
pub mod hrv4t;
pub mod lab;
pub mod labels;
pub mod nights;
pub mod optim;
pub mod schema;
pub mod stats;
pub mod synth;
pub mod time;
