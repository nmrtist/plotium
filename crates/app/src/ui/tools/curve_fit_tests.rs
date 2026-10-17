use super::*;

#[test]
fn frozen_b_table_selects_diffusion_model_with_or_without_acquisition_constants() {
    let mut app = PlotiumApp::new_with_settings(plotium_core::settings::Settings::default());
    app.new_table_dataset();
    let table = app.doc.datasets[0].as_table_mut().unwrap();
    assert!(table.provenance.is_none());
    table.meta.diffusion_input =
        Some(plotium_core::DiffusionInput::direct(&[0., 1e9, 2e9], "s/m²").unwrap());
    assert_eq!(
        default_fit_model(table),
        plotium_analysis::models::DIFFUSION_B_ID
    );
    table.meta.diffusion = Some(plotium_core::state::DiffusionConstants {
        gamma: 2.675e8,
        delta: 0.002,
        big_delta: 0.1,
        tau: 0.,
        shape_factor: 1.,
    });
    assert_eq!(
        default_fit_model(table),
        plotium_analysis::models::DIFFUSION_B_ID
    );
}
