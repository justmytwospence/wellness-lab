//! The wellness-v1 column lists, from `schema/wellness-v1.json` (the contract shared with garmin-mcp's writer).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Deserialize, Debug)]
pub struct Schema {
    pub schema_version: u32,
    pub tables: BTreeMap<String, Vec<(String, String)>>,
    pub series: BTreeMap<String, Vec<(String, String)>>,
    pub rr: Vec<(String, String)>,
}

pub fn schema() -> &'static Schema {
    static S: OnceLock<Schema> = OnceLock::new();
    S.get_or_init(|| {
        serde_json::from_str(include_str!("../../../schema/wellness-v1.json"))
            .expect("schema/wellness-v1.json")
    })
}

pub const TABLE_NAMES: [&str; 6] = ["days", "sleeps", "stages", "activities", "tags", "devices"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_parses() {
        let s = schema();
        assert_eq!(s.schema_version, 1);
        assert_eq!(
            s.tables["days"][0],
            ("day".to_string(), "INTEGER".to_string())
        );
        assert_eq!(s.series["stress"].len(), 3);
        assert_eq!(s.rr[0].0, "rr");
        for t in TABLE_NAMES {
            assert!(s.tables.contains_key(t));
        }
    }
}
