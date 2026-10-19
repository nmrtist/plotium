use super::*;

#[path = "../../io/tests/support/jeol_diffusion.rs"]
mod fixture;

// Decimal reference, evaluated independently of Plotium's encoding functions:
// (267522200.5 * 0.0035 * (0.02 * n))² * (0.1 - 2*0.0035/3 - 0.001).
// Uses the application's existing 1H gamma, not a value read from the model.
// Rounded to f64 only after a 40-digit decimal calculation; n = 1..=14.
const REFERENCE_B: [f64; 14] = [
    33_899_436.515_824_9,
    135_597_746.063_299_6,
    305_094_928.642_424_05,
    542_390_984.253_198_4,
    847_485_912.895_622_4,
    1_220_379_714.569_696_2,
    1_661_072_389.275_42,
    2_169_563_937.012_793_5,
    2_745_854_357.781_816_5,
    3_389_943_651.582_489_5,
    4_101_831_818.414_812_6,
    4_881_518_858.278_785,
    5_729_004_771.174_407,
    6_644_289_557.101_68,
];

fn assert_jeol_times_and_evidence(input: &DiffusionInput, series: &NmrSeriesSource) {
    let DiffusionModel::JeolBppDste {
        delta,
        big_delta,
        tau_1,
        tau_2,
        ..
    } = input.model
    else {
        panic!("wrong model");
    };
    for (actual, expected) in [
        (delta, 0.0035),
        (big_delta, 0.1),
        (tau_1, 0.001),
        (tau_2, 0.001),
    ] {
        assert!((actual - expected).abs() < 1e-14, "{actual} != {expected}");
    }
    let time = jeol_bpp_dste_time(delta, big_delta, tau_1, tau_2).unwrap();
    assert!((time - 0.09666666666666667).abs() < 1e-14);
    let facts = series.jeol_diffusion.as_ref().unwrap();
    // Exercise each imported evidence time against the model, including when
    // the acquisition and evidence records use different unit encodings.
    for (name, expected) in [
        ("diffusion_time", big_delta),
        ("grad_1", delta / 2.0),
        ("tau_e1", delta / 2.0 + tau_1),
        ("tau_e2", delta / 2.0 + tau_2),
        ("st_coef", time),
    ] {
        assert!(
            (facts.seconds(name).unwrap() - expected).abs() < 1e-14,
            "{name}"
        );
    }
    assert!((input.vendor_coefficient_seconds.unwrap() - time).abs() < 1e-14);
}

#[test]
fn jeol_synthetic_time_encodings_recover_reference_b_values_and_diffusion() {
    use plotium_analysis::{diffusion::fit_diffusion_b_cancellable, series::DecaySeries};

    const D: f64 = 1.2e-9;
    // Signal generation must use the independent reference, never the imported
    // b values: otherwise a unit error could cancel between signal and fitting.
    let y: Vec<_> = REFERENCE_B.iter().map(|b| 100.0 * (-D * b).exp()).collect();
    for encoding in fixture::TIME_ENCODINGS {
        let mut bytes = fixture::bytes();
        fixture::encode_times(&mut bytes, encoding);
        let series = fixture::read(&bytes);
        assert_eq!(series.rows, 14);
        let axis = series.pseudo_axis.as_ref().unwrap();
        assert_eq!(axis.unit, "T/m");
        for (i, actual) in axis.values.iter().enumerate() {
            assert!((actual - 0.02 * (i + 1) as f64).abs() < 1e-14);
        }
        let mut input = automatic_input(&series).unwrap();
        assert_jeol_times_and_evidence(&input, &series);
        assert_eq!(input.rule_version, 1);
        assert_ne!(
            input.program_sha256.as_deref(),
            Some(KNOWN_JEOL_BPP_DSTE_PROGRAM)
        );
        assert!(!input.confirmed);
        assert!(input.b_values(14).is_err());
        // Simulate the user's explicit confirmation of this synthetic program.
        input.confirmed = true;
        let b = input.b_values(14).unwrap();
        for (i, (actual, expected)) in b.iter().zip(REFERENCE_B).enumerate() {
            assert!(
                (actual / expected - 1.0).abs() < 1e-12,
                "{encoding:?}, b[{i}]={actual}"
            );
        }
        let fit =
            fit_diffusion_b_cancellable(&DecaySeries { x: b, y: y.clone() }, &|| false).unwrap();
        assert!((fit.d / D - 1.0).abs() < 1e-6, "{encoding:?}: {fit:?}");
        assert!(fit.r2 > 0.999999, "{fit:?}");
    }
}

#[test]
fn jeol_synthetic_mixed_evidence_units_still_require_program_confirmation() {
    let original = fixture::bytes();
    let mut bytes = original.clone();
    fixture::encode_times(&mut bytes, fixture::TIME_ENCODINGS[0]);
    for name in ["diffusion_time", "grad_1", "tau_e1", "tau_e2", "st_coef"] {
        let mut milliseconds = original.clone();
        fixture::record_mut(&mut bytes, name)
            .copy_from_slice(fixture::record_mut(&mut milliseconds, name));
    }
    // Even a matching filename and timing evidence cannot authorize an unknown
    // pulse section. Keep the production hash whitelist as the deciding gate.
    bytes.extend_from_slice(b"\nbpp_dste_led_diffusion.jxp\n");
    let series = fixture::read(&bytes);
    let facts = series.jeol_diffusion.as_ref().unwrap();
    assert!(facts.has_full_program_identity());
    assert_eq!(facts.gradient_axis, Some(0));
    let input = automatic_input(&series).unwrap();
    assert_jeol_times_and_evidence(&input, &series);
    assert!(!input.confirmed);
    assert!(input.b_values(14).is_err());
}

#[test]
fn direct_b_conversion_and_identity_validation() {
    let mut input = DiffusionInput::direct(&[3., 0., 1.], "s/mm²").unwrap();
    assert_eq!(input.b_values(3).unwrap(), vec![3e6, 0., 1e6]);
    input.increments.swap(0, 2);
    assert_eq!(input.b_values(3).unwrap(), vec![3e6, 0., 1e6]);
    input.increments[0].id = input.increments[1].id;
    assert!(input.b_values(3).is_err());
    assert!(DiffusionInput::direct(&[0., 1., f64::INFINITY], "s/m²").is_err());
}

/// Opt-in external reference, never copied into the repository or build output.
#[test]
#[ignore = "set PLOTIUM_DOSY_REFERENCE_FILE to a JEOL BPP-DSTE reference JDF"]
fn jeol_bpp_dste_external_reference() {
    let path = std::env::var_os("PLOTIUM_DOSY_REFERENCE_FILE").expect("reference path");
    let dataset = nmr::ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read(path)
        .unwrap();
    let source = plotium_io::nmr_view::NmrSource::new(std::sync::Arc::new(dataset)).unwrap();
    let series = NmrSeriesSource::new(source).unwrap();
    let facts = series.jeol_diffusion.as_ref().unwrap();
    let input = automatic_input(&series).unwrap();
    assert!(
        input.confirmed,
        "source evidence did not confirm: {:?}; times {:?}; hash {:?}",
        input.model,
        [
            facts.seconds("delta"),
            facts.seconds("delta_large"),
            facts.seconds("st_coef")
        ],
        input.program_sha256
    );
    let b = input.b_values(14).unwrap();
    for (actual, expected) in b.iter().zip(REFERENCE_B) {
        assert!((actual / expected - 1.0).abs() < 1e-5);
    }
    assert_eq!(series.rows, 14);
    let DiffusionModel::JeolBppDste {
        delta,
        big_delta,
        tau_1,
        tau_2,
        ..
    } = input.model
    else {
        panic!("wrong model");
    };
    for (actual, expected) in [
        (delta, 0.0035),
        (big_delta, 0.1),
        (tau_1, 0.001),
        (tau_2, 0.001),
    ] {
        assert!((actual - expected).abs() < 1e-14);
    }
    assert!(
        (jeol_bpp_dste_time(delta, big_delta, tau_1, tau_2).unwrap() * 1e3 - 96.6666666667).abs()
            < 1e-8
    );
    let axis = series.pseudo_axis.as_ref().unwrap();
    assert_eq!(axis.unit, "T/m");
    assert!((axis.values[0] - 0.02).abs() < 1e-12);
    assert!((axis.values[13] - 0.28).abs() < 1e-12);
    println!(
        "JEOL reference: delta={delta} s, Delta={big_delta} s, tau_1={tau_1} s, tau_2={tau_2} s; b={}..={} s/m²; 14 increments confirmed",
        b[0], b[13]
    );
}
