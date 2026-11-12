//! The Wellness Lab engine: every number the app shows. NaN is a missing value inside the engine.
#![allow(
    clippy::collapsible_match,
    clippy::field_reassign_with_default,
    clippy::needless_lifetimes,
    clippy::len_zero,
    clippy::needless_range_loop,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::manual_range_contains
)]

pub mod baseline;
pub mod beats;
pub mod bench;
pub mod circadian;
pub mod classify;
pub mod compare;
pub mod coverage;
pub mod data;
pub mod detect;
pub mod dfa;
pub mod events;
pub mod experiments;
pub mod hrv4t;
pub mod kalman;
pub mod lab;
pub mod labels;
pub mod math;
pub mod nights;
pub mod optim;
pub mod registry;
pub mod render;
pub mod schema;
pub mod stats;
pub mod synth;
pub mod teardown;
pub mod time;
