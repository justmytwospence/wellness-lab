//! The in-memory wellness-v1 output: typed columns in the schema's order, convertible to and from `core::Data`.

use crate::schema::{schema, TABLE_NAMES};
use serde::Serialize;
use std::collections::BTreeMap;
use wellness_lab_core::data::{Data, Series, Table};

/// One column. `None` is a null.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Column {
    I32(Vec<Option<i32>>),
    I64(Vec<Option<i64>>),
    F32(Vec<Option<f32>>),
    Str(Vec<Option<String>>),
}

impl Column {
    pub fn len(&self) -> usize {
        match self {
            Column::I32(v) => v.len(),
            Column::I64(v) => v.len(),
            Column::F32(v) => v.len(),
            Column::Str(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The values as f64 with NaN for null (strings give an empty vector).
    pub fn f64s(&self) -> Vec<f64> {
        match self {
            Column::I32(v) => v.iter().map(|x| x.map_or(f64::NAN, |x| x as f64)).collect(),
            Column::I64(v) => v.iter().map(|x| x.map_or(f64::NAN, |x| x as f64)).collect(),
            Column::F32(v) => v.iter().map(|x| x.map_or(f64::NAN, |x| x as f64)).collect(),
            Column::Str(_) => vec![],
        }
    }
    /// An all-null column of a DuckDB schema type (`INTEGER`, `BIGINT`, `FLOAT`, `VARCHAR`).
    pub fn nulls(ty: &str, n: usize) -> Column {
        Column::from_f64(ty, &vec![f64::NAN; n]).unwrap_or_else(|| Column::Str(vec![None; n]))
    }
    /// A numeric column of a schema type from f64 values (NaN is null); `None` for `VARCHAR`.
    pub fn from_f64(ty: &str, v: &[f64]) -> Option<Column> {
        let ok = |x: &f64| x.is_finite();
        Some(match ty {
            "INTEGER" => Column::I32(v.iter().map(|x| ok(x).then_some(*x as i32)).collect()),
            "BIGINT" => Column::I64(v.iter().map(|x| ok(x).then_some(*x as i64)).collect()),
            "FLOAT" => Column::F32(v.iter().map(|x| ok(x).then_some(*x as f32)).collect()),
            _ => return None,
        })
    }
}

/// Named columns in schema order.
pub type Columns = Vec<(String, Column)>;

/// Look up a column by name.
pub fn col<'a>(cols: &'a Columns, name: &str) -> Option<&'a Column> {
    cols.iter().find(|(n, _)| n == name).map(|(_, c)| c)
}

/// Rows of a column list (the length of its first column).
pub fn rows(cols: &Columns) -> usize {
    cols.first().map(|(_, c)| c.len()).unwrap_or(0)
}

/// A wellness-v1 dataset in memory. `rr` is keyed by activity id (`garmin:<id>`).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Tables {
    pub days: Columns,
    pub sleeps: Columns,
    pub stages: Columns,
    pub activities: Columns,
    pub tags: Columns,
    pub devices: Columns,
    pub series: BTreeMap<String, Columns>,
    pub rr: BTreeMap<String, Vec<f32>>,
}

impl Tables {
    pub fn table(&self, name: &str) -> Option<&Columns> {
        Some(match name {
            "days" => &self.days,
            "sleeps" => &self.sleeps,
            "stages" => &self.stages,
            "activities" => &self.activities,
            "tags" => &self.tags,
            "devices" => &self.devices,
            _ => return None,
        })
    }
    fn table_mut(&mut self, name: &str) -> Option<&mut Columns> {
        Some(match name {
            "days" => &mut self.days,
            "sleeps" => &mut self.sleeps,
            "stages" => &mut self.stages,
            "activities" => &mut self.activities,
            "tags" => &mut self.tags,
            "devices" => &mut self.devices,
            _ => return None,
        })
    }

    /// The inverse of loading: typed columns from `core::Data`, so synthetic data can be written by the same writers.
    pub fn from_data(data: &Data) -> Tables {
        let sch = schema();
        let mut out = Tables::default();
        for name in TABLE_NAMES {
            let t = match name {
                "days" => &data.days,
                "sleeps" => &data.sleeps,
                "stages" => &data.stages,
                "activities" => &data.activities,
                "tags" => &data.tags,
                _ => &data.devices,
            };
            *out.table_mut(name).unwrap() = table_columns(&sch.tables[name], t);
        }
        for (name, cols) in &sch.series {
            let Some(s) = data.series.get(name).filter(|s| !s.is_empty()) else {
                continue;
            };
            let mut c: Columns = Vec::with_capacity(cols.len());
            for (cname, ty) in cols {
                let column = if cname == "t" {
                    Column::I64(s.t.iter().map(|&t| Some(t)).collect())
                } else {
                    match s.cols.get(cname) {
                        Some(v) => Column::from_f64(ty, v)
                            .unwrap_or_else(|| Column::Str(vec![None; s.len()])),
                        None => Column::nulls(ty, s.len()),
                    }
                };
                c.push((cname.clone(), column));
            }
            out.series.insert(name.clone(), c);
        }
        for (id, rr) in &data.rr {
            let key = if id.starts_with("garmin:") {
                id.clone()
            } else {
                format!("garmin:{id}")
            };
            out.rr.insert(key, rr.iter().map(|&x| x as f32).collect());
        }
        out
    }

    /// Loading: `core::Data` (NaN for numeric nulls). `rr` keys lose their `garmin:` prefix, as `io::read_dir` gives them.
    pub fn to_data(&self) -> Data {
        let mut d = Data::default();
        for name in TABLE_NAMES {
            let cols = self.table(name).unwrap();
            let slot = d.table_mut(name).unwrap();
            let mut t = Table::new(rows(cols));
            for (n, c) in cols {
                match c {
                    Column::Str(v) => {
                        t.txt.insert(n.clone(), v.clone());
                    }
                    _ => {
                        t.num.insert(n.clone(), c.f64s());
                    }
                }
            }
            *slot = t;
        }
        for (name, cols) in &self.series {
            let mut s = Series::default();
            for (n, c) in cols {
                if n == "t" {
                    if let Column::I64(v) = c {
                        s.t = v.iter().map(|x| x.unwrap_or(0)).collect();
                    }
                } else {
                    s.cols.insert(n.clone(), c.f64s());
                }
            }
            d.series.insert(name.clone(), s);
        }
        for (id, rr) in &self.rr {
            let key = id.strip_prefix("garmin:").unwrap_or(id).to_string();
            d.rr.insert(key, rr.iter().map(|&x| x as f64).collect());
        }
        d
    }
}

fn table_columns(cols: &[(String, String)], t: &Table) -> Columns {
    cols.iter()
        .map(|(name, ty)| {
            let c = if ty == "VARCHAR" {
                Column::Str(t.text(name))
            } else {
                Column::from_f64(ty, &t.col(name)).unwrap()
            };
            (name.clone(), c)
        })
        .collect()
}
