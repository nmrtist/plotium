//! Shared byte-level variants of the original synthetic fixture, never vendor data.
use plotium_io::{nmr_series::NmrSeriesSource, nmr_view::NmrSource};
use std::sync::Arc;

pub fn bytes() -> Vec<u8> {
    include_bytes!("../fixtures/nmr/jeol-diffusion.jdf").to_vec()
}

pub fn record_mut<'a>(bytes: &'a mut [u8], name: &str) -> &'a mut [u8] {
    let count = u32::from_le_bytes(bytes[1368..1372].try_into().unwrap()) as usize;
    bytes[1376..1376 + 64 * count]
        .as_chunks_mut::<64>()
        .0
        .iter_mut()
        .find(|record| record[36..].split(|&b| b == 0).next().unwrap() == name.as_bytes())
        .expect("synthetic parameter")
}

// (packed primary unit, numeric scaler, multiplier of fixture's millisecond value).
pub const TIME_ENCODINGS: [(u8, i16, f64); 5] = [
    (0x01, 0, 0.001),  // seconds
    (0x11, 0, 1.0),    // milliseconds (fixture)
    (0x21, 0, 1000.0), // microseconds
    (0x11, -1, 10.0),  // negative numeric scaling plus milliseconds
    (0x21, 1, 100.0),  // positive numeric scaling plus microseconds
];

pub fn encode_times(bytes: &mut [u8], (unit, scaler, factor): (u8, i16, f64)) {
    for name in [
        "delta",
        "delta_large",
        "tau_1",
        "tau_2",
        "diffusion_time",
        "grad_1",
        "tau_e1",
        "tau_e2",
        "st_coef",
    ] {
        let record = record_mut(bytes, name);
        let value = f64::from_le_bytes(record[16..24].try_into().unwrap());
        record[4..6].copy_from_slice(&scaler.to_le_bytes());
        record[6] = unit;
        record[16..24].copy_from_slice(&(value * factor).to_le_bytes());
    }
}

pub fn read(bytes: &[u8]) -> NmrSeriesSource {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.jdf");
    std::fs::write(&path, bytes).unwrap();
    let dataset = nmr::ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read(&path)
        .unwrap();
    NmrSeriesSource::new(NmrSource::new(Arc::new(dataset)).unwrap()).unwrap()
}
