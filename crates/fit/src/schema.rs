//! The wellness-v1 column lists. `wellness-lab-core` loads them with
//! `include_str!("../../../schema/wellness-v1.json")`; the builder emits columns in exactly that order.

pub use wellness_lab_core::schema::{schema, Schema, TABLE_NAMES};
