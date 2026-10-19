#[path = "support/jeol_diffusion.rs"]
mod fixture;

#[test]
fn synthetic_fixture_has_readable_samples_for_every_gradient() {
    let series = fixture::read(&fixture::bytes());
    assert_eq!(series.rows, 14);
    let raw = series.source_dataset().dataset().as_raw().unwrap();
    for row in 0..14 {
        let trace = raw.read_trace(&[row]).unwrap();
        assert_eq!(trace.samples().len(), 4);
        for (column, value) in trace.samples().iter().enumerate() {
            assert!((value.re - (1.0 + row as f64 + column as f64 / 10.0)).abs() < 1e-14);
            assert_eq!(value.im, 0.0);
        }
    }
}

#[test]
fn seconds_combines_numeric_scaling_and_si_prefix_for_all_time_records() {
    for encoding in fixture::TIME_ENCODINGS {
        let mut bytes = fixture::bytes();
        fixture::encode_times(&mut bytes, encoding);
        let series = fixture::read(&bytes);
        let facts = series.jeol_diffusion.as_ref().unwrap();
        for (name, expected) in [
            ("delta", 0.0035),
            ("delta_large", 0.1),
            ("tau_1", 0.001),
            ("tau_2", 0.001),
            ("diffusion_time", 0.1),
            ("grad_1", 0.00175),
            ("tau_e1", 0.00275),
            ("tau_e2", 0.00275),
            ("st_coef", 0.09666666666666667),
        ] {
            let actual = facts.seconds(name).unwrap();
            assert!(
                (actual - expected).abs() < 1e-14,
                "{name}: {encoding:?}: {actual}"
            );
        }
    }
}

#[test]
fn integer_time_records_and_signed_durations() {
    for (value, scaler, unit, expected) in [
        (3500_i32, 0_i16, 0x21, 0.0035),
        (35, -1, 0x11, 0.0035),
        (35, 2, 0x21, 0.0035),
        (0, 0, 0x11, 0.0),
        (-35, -1, 0x11, -0.0035),
    ] {
        let mut bytes = fixture::bytes();
        let record = fixture::record_mut(&mut bytes, "delta");
        record[4..6].copy_from_slice(&scaler.to_le_bytes());
        record[6] = unit;
        record[16..32].fill(0);
        record[16..20].copy_from_slice(&value.to_le_bytes());
        record[32..36].copy_from_slice(&1_i32.to_le_bytes());
        let series = fixture::read(&bytes);
        let actual = series
            .jeol_diffusion
            .as_ref()
            .unwrap()
            .seconds("delta")
            .unwrap();
        assert!((actual - expected).abs() < 1e-14);
    }
}

#[test]
fn invalid_or_missing_time_records_are_not_seconds() {
    for case in [
        "missing",
        "text",
        "base",
        "power",
        "compound",
        "prefix_overflow",
        "scale_overflow",
    ] {
        let mut bytes = fixture::bytes();
        let record = fixture::record_mut(&mut bytes, "delta");
        match case {
            "missing" => record[36..41].copy_from_slice(b"other"),
            "text" => {
                record[32..36].fill(0);
                record[16..32].fill(0);
                record[16..20].copy_from_slice(b"text");
            }
            "base" => record[7] = 13,
            "power" => record[6] = 0x12,
            "compound" => record[8..10].copy_from_slice(&[1, 13]),
            "prefix_overflow" => {
                record[6] = 0x81; // 10^24 seconds, finite before prefix conversion
                record[16..24].copy_from_slice(&1e300_f64.to_le_bytes());
            }
            "scale_overflow" => record[4..6].copy_from_slice(&309_i16.to_le_bytes()),
            _ => unreachable!(),
        }
        let series = fixture::read(&bytes);
        assert_eq!(
            series.jeol_diffusion.as_ref().unwrap().seconds("delta"),
            None,
            "{case}"
        );
    }
}
