use super::*;
use crate::{DiffusionIncrement, IncrementId};

#[test]
fn report_retains_conversion_and_sorts_acquired_ids_without_double_conversion() {
    for (unit, factor) in [("s/m²", 1.), ("s/cm²", 1e4), ("s/mm²", 1e6)] {
        let mut input = DiffusionInput::direct(&[0., 1., 2.], unit).unwrap();
        input.increments.reverse();
        let report = input.inspect(3);
        assert!(report.executable);
        assert_eq!(report.conversion_factor, Some(factor));
        assert_eq!(report.b_values[1].b, factor);
        assert_eq!(input.b_values(3).unwrap(), vec![0., factor, 2. * factor]);
    }
}
#[test]
fn report_accumulates_invalid_numbers_and_increment_binding_errors() {
    let mut input = DiffusionInput::direct(&[0., 1., 2.], "s/m²").unwrap();
    input.increments = vec![
        DiffusionIncrement {
            id: IncrementId(0),
            b: f64::NAN,
        },
        DiffusionIncrement {
            id: IncrementId(0),
            b: -1.,
        },
        DiffusionIncrement {
            id: IncrementId(7),
            b: 2.,
        },
    ];
    let report = input.inspect(3);
    assert_eq!(report.status, DiffusionReadiness::InvalidInput);
    for code in [
        "invalid_b_value",
        "duplicate_increment",
        "unknown_increment",
        "missing_increment",
    ] {
        assert!(
            report.issues.iter().any(|issue| issue.code == code),
            "{code}"
        );
    }
    let value = serde_json::to_value(report).unwrap();
    assert!(value["b_values"][0]["b"].is_null());
}
#[test]
fn report_distinguishes_confirmation_from_malformed_model_parameters() {
    let mut input = DiffusionInput::direct(&[0., 1., 2.], "s/m²").unwrap();
    input.confirmed = false;
    assert_eq!(
        input.inspect(3).status,
        DiffusionReadiness::EvidenceRequired
    );
    input.model = DiffusionModel::JeolBppDste {
        gamma: 1.,
        delta: 1.,
        big_delta: -1.,
        tau_1: 0.,
        tau_2: 0.,
    };
    assert!(input.gradients.is_none());
    let report = input.inspect(3);
    assert_eq!(report.status, DiffusionReadiness::InvalidInput);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.code == "invalid_model_parameters")
    );
}
