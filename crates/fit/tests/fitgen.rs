//! A minimal FIT encoder for test fixtures (no real data, no serial numbers), ported from garmin-mcp `test/fitgen.py`
//! (`FitWriter`, `wellness_fit`, `hrv_status_fit`, `sleep_fit`, `activity_fit`).
//!
//! Writes valid FIT files with the few message types the decoder tests need. Field definitions follow the FIT
//! profile (global message numbers and field numbers). Shared by the tests (`mod fitgen;`) and
//! `examples/make_fake_export.rs`.
#![allow(dead_code)]

/// Seconds between the Unix epoch and the FIT epoch (1989-12-31T00:00:00Z).
pub const FIT_EPOCH_S: i64 = 631_065_600;

/// FIT seconds of a Unix ms instant.
pub fn fit_ts(unix_ms: i64) -> u32 {
    (unix_ms / 1000 - FIT_EPOCH_S) as u32
}

/// Base types: (type id byte, size).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Base {
    Enum,
    U8,
    S16,
    U16,
    S32,
    U32,
    U32z,
}

impl Base {
    fn id(self) -> u8 {
        match self {
            Base::Enum => 0x00,
            Base::U8 => 0x02,
            Base::S16 => 0x83,
            Base::U16 => 0x84,
            Base::S32 => 0x85,
            Base::U32 => 0x86,
            Base::U32z => 0x8C,
        }
    }
    fn size(self) -> u8 {
        match self {
            Base::Enum | Base::U8 => 1,
            Base::S16 | Base::U16 => 2,
            Base::S32 | Base::U32 | Base::U32z => 4,
        }
    }
    fn put(self, out: &mut Vec<u8>, x: i64) {
        match self.size() {
            1 => out.push(x as u8),
            2 => out.extend_from_slice(&(x as u16).to_le_bytes()),
            _ => out.extend_from_slice(&(x as u32).to_le_bytes()),
        }
    }
}

/// A field value: one number or an array.
#[derive(Clone, Debug)]
pub enum V {
    One(i64),
    Many(Vec<i64>),
}

impl V {
    fn vals(&self) -> Vec<i64> {
        match self {
            V::One(x) => vec![*x],
            V::Many(v) => v.clone(),
        }
    }
}

pub fn one(x: i64) -> V {
    V::One(x)
}

pub fn crc16(data: &[u8]) -> u16 {
    const T: [u16; 16] = [
        0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401, 0xA001, 0x6C00, 0x7800,
        0xB401, 0x5000, 0x9C01, 0x8801, 0x4400,
    ];
    let mut crc: u16 = 0;
    for &b in data {
        let tmp = T[(crc & 0xF) as usize];
        crc = (crc >> 4) & 0x0FFF;
        crc = crc ^ tmp ^ T[(b & 0xF) as usize];
        let tmp = T[(crc & 0xF) as usize];
        crc = (crc >> 4) & 0x0FFF;
        crc = crc ^ tmp ^ T[((b >> 4) & 0xF) as usize];
    }
    crc
}

type Sig = (u16, Vec<(u8, u8, u8)>);

#[derive(Default)]
pub struct FitWriter {
    body: Vec<u8>,
    next: usize,
    /// the definition currently held by each local message number
    slots: Vec<Option<Sig>>,
}

impl FitWriter {
    pub fn new() -> Self {
        FitWriter {
            body: vec![],
            next: 0,
            slots: vec![None; 16],
        }
    }

    /// `fields`: (field number, base type, value); a `V::Many` value is an array field.
    pub fn message(&mut self, global: u16, fields: &[(u8, Base, V)]) {
        let sig: Sig = (
            global,
            fields
                .iter()
                .map(|(f, t, v)| (*f, t.id(), v.vals().len() as u8))
                .collect(),
        );
        let local = match self.slots.iter().position(|s| s.as_ref() == Some(&sig)) {
            Some(l) => l,
            None => {
                let l = self.next % 16;
                self.next += 1;
                let mut d = vec![0x40 | l as u8, 0, 0];
                d.extend_from_slice(&global.to_le_bytes());
                d.push(fields.len() as u8);
                for (f, t, v) in fields {
                    d.extend_from_slice(&[*f, t.size() * v.vals().len() as u8, t.id()]);
                }
                self.body.extend(d);
                self.slots[l] = Some(sig);
                l
            }
        };
        self.body.push(local as u8);
        for (_, t, v) in fields {
            for x in v.vals() {
                t.put(&mut self.body, x);
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut h = vec![14u8, 0x20];
        h.extend_from_slice(&2195u16.to_le_bytes());
        h.extend_from_slice(&(self.body.len() as u32).to_le_bytes());
        h.extend_from_slice(b".FIT");
        let c = crc16(&h);
        h.extend_from_slice(&c.to_le_bytes());
        h.extend_from_slice(&self.body);
        let c = crc16(&h);
        h.extend_from_slice(&c.to_le_bytes());
        h
    }
}

/// file_id: type, manufacturer garmin, product 3121, serial 12345, time created.
pub fn file_id(w: &mut FitWriter, ty: i64, t: u32) {
    w.message(
        0,
        &[
            (0, Base::Enum, one(ty)),
            (1, Base::U16, one(1)),
            (2, Base::U16, one(3121)),
            (3, Base::U32z, one(12345)),
            (4, Base::U32, one(t as i64)),
        ],
    );
}

/// A monitoring_b file: `minutes` of HR (60 + i), cumulative walking steps (200 i), stress (20 + i),
/// respiration 15.00 and, every other minute, undocumented message 279 altitude of 2000 m.
pub fn wellness_fit(day_start_ms: i64, minutes: u32) -> Vec<u8> {
    let mut w = FitWriter::new();
    wellness_into(&mut w, day_start_ms, minutes);
    w.to_bytes()
}

/// The body of `wellness_fit`, for tests that append messages.
pub fn wellness_into(w: &mut FitWriter, day_start_ms: i64, minutes: u32) {
    let t0 = fit_ts(day_start_ms) as i64;
    file_id(w, 32, t0 as u32); // monitoring_b
    w.message(103, &[(253, Base::U32, one(t0)), (0, Base::U32, one(t0))]); // monitoring_info
    for i in 0..minutes as i64 {
        let t = t0 + i * 60;
        // monitoring: cycles (steps when walking), hr, walking
        w.message(
            55,
            &[
                (253, Base::U32, one(t)),
                (3, Base::U32, one(200 * i)),
                (27, Base::U8, one(60 + i)),
                (5, Base::Enum, one(6)),
            ],
        );
        w.message(227, &[(0, Base::S16, one(20 + i)), (1, Base::U32, one(t))]); // stress_level
        w.message(297, &[(253, Base::U32, one(t)), (0, Base::U16, one(1500))]); // respiration_rate (breaths/min * 100)
        if i % 2 == 0 {
            // undocumented altitude message: (m + 500) * 5
            w.message(
                279,
                &[
                    (253, Base::U32, one(t)),
                    (0, Base::U16, one((2000 + 500) * 5)),
                ],
            );
        }
    }
}

/// An HRV status file (type 68): `values` (ms) every five minutes ending at `night_end`, and the status summary.
pub fn hrv_status_fit(night_end_ms: i64, values: &[f64]) -> Vec<u8> {
    let mut w = FitWriter::new();
    let t_end = fit_ts(night_end_ms) as i64;
    file_id(&mut w, 68, t_end as u32);
    let n = values.len() as i64;
    for (i, v) in values.iter().enumerate() {
        w.message(
            371,
            &[
                (253, Base::U32, one(t_end - (n - i as i64) * 300)),
                (0, Base::U16, one((v * 128.0) as i64)),
            ],
        ); // hrv_value, scale 128
    }
    let last = *values.last().unwrap_or(&0.0);
    w.message(
        370,
        &[
            (253, Base::U32, one(t_end)),
            (0, Base::U16, one(60 * 128)),
            (1, Base::U16, one((last * 128.0) as i64)),
            (2, Base::U16, one(80 * 128)),
            (3, Base::U16, one(40 * 128)),
            (4, Base::U16, one(45 * 128)),
            (5, Base::U16, one(70 * 128)),
            (6, Base::Enum, one(2)),
        ],
    );
    w.to_bytes()
}

/// A sleep file (type 49): (minutes, stage) pairs, each stage a change at its start. Stages: awake, light, deep, rem.
pub fn sleep_fit(start_ms: i64, stages: &[(i64, &str)]) -> Vec<u8> {
    let mut w = FitWriter::new();
    let t0 = fit_ts(start_ms) as i64;
    file_id(&mut w, 49, t0 as u32);
    let mut t = t0;
    for (minutes, stage) in stages {
        let code = match *stage {
            "awake" => 1,
            "light" => 2,
            "deep" => 3,
            _ => 4,
        };
        w.message(275, &[(253, Base::U32, one(t)), (0, Base::Enum, one(code))]); // sleep_level
        t += minutes * 60;
    }
    w.to_bytes()
}

/// An activity file: `seconds` of records from `start`, the beat-to-beat intervals `rr` (ms; 0xFFFF pads) in hrv
/// messages of five, a lap, a session and the activity message.
pub fn activity_fit(start_ms: i64, seconds: i64, rr: &[i64]) -> Vec<u8> {
    let mut w = FitWriter::new();
    let t0 = fit_ts(start_ms) as i64;
    file_id(&mut w, 4, t0 as u32);
    w.message(
        21,
        &[
            (253, Base::U32, one(t0)),
            (0, Base::Enum, one(0)),
            (1, Base::Enum, one(0)),
            (3, Base::U32, one(0)),
        ],
    ); // event timer start
    for i in 0..seconds {
        w.message(
            20,
            &[
                (253, Base::U32, one(t0 + i)),
                (5, Base::U32, one(i * 300)),
                (3, Base::U8, one(120 + i % 5)),
            ],
        ); // record: distance cm, hr
    }
    for chunk in rr.chunks(5) {
        let mut v = chunk.to_vec();
        v.resize(5, 0xFFFF);
        w.message(78, &[(0, Base::U16, V::Many(v))]); // hrv
    }
    let end = t0 + seconds;
    w.message(
        19,
        &[
            (253, Base::U32, one(end)),
            (2, Base::U32, one(t0)),
            (254, Base::U16, one(0)),
            (7, Base::U32, one(seconds * 1000)),
        ],
    ); // lap
    w.message(
        18,
        &[
            (253, Base::U32, one(end)),
            (2, Base::U32, one(t0)),
            (254, Base::U16, one(0)),
            (5, Base::Enum, one(1)),
            (7, Base::U32, one(seconds * 1000)),
            (9, Base::U32, one(seconds * 300)),
        ],
    ); // session: running
    w.message(
        34,
        &[
            (253, Base::U32, one(end)),
            (0, Base::U32, one(seconds * 1000)),
            (1, Base::U16, one(1)),
            (2, Base::Enum, one(26)),
            (3, Base::Enum, one(1)),
        ],
    ); // activity
    w.to_bytes()
}
