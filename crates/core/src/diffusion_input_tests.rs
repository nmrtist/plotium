use super::*;

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
    assert!((b[0] / 3.38994e7 - 1.0).abs() < 1e-5);
    assert!((b[13] / 6.64429e9 - 1.0).abs() < 1e-5);
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
    assert!(
        (jeol_bpp_dste_time(delta, big_delta, tau_1, tau_2).unwrap() * 1e3 - 96.6666666667).abs()
            < 1e-8
    );
    let axis = series.pseudo_axis.as_ref().unwrap();
    assert_eq!(axis.unit, "T/m");
    assert!((axis.values[0] - 0.02).abs() < 1e-12);
    assert!((axis.values[13] - 0.28).abs() < 1e-12);
}
