use super::*;

fn parse(values: &[&str]) -> Result<ParseOutcome, ParseError> {
    parse_args(values.iter().map(OsString::from))
}

#[test]
fn inspect_parser_accepts_json_on_either_side_of_input() {
    let expected = ParseOutcome::Command(Command::Inspect {
        input: "sample.jdf".into(),
        json: true,
        sampling_declaration: None,
    });
    assert_eq!(
        parse(&["plotium-cli", "inspect", "--json", "sample.jdf"]),
        Ok(expected.clone())
    );
    assert_eq!(
        parse(&["plotium-cli", "inspect", "sample.jdf", "--json"]),
        Ok(expected)
    );
}

#[test]
fn process_parser_infers_format_and_requires_named_paths() {
    assert_eq!(
        parse(&[
            "plotium-cli",
            "process",
            "sample.jdf",
            "--scheme",
            "routine.plotiumproc",
            "--output",
            "figure.svg",
        ]),
        Ok(ParseOutcome::Command(Command::Process {
            input: "sample.jdf".into(),
            scheme: "routine.plotiumproc".into(),
            output: "figure.svg".into(),
            format: OutputFormat(ExportFormat::Svg),
            sampling_declaration: None,
        }))
    );
    assert!(parse(&["plotium-cli", "process", "sample.jdf"]).is_err());
}

#[test]
fn craft_parser_accepts_multiple_and_negative_ppm_regions() {
    assert_eq!(
        parse(&[
            "plotium-cli",
            "craft",
            "acquisitions",
            "--region",
            "-0.5:0.2",
            "--region",
            "6.3:6.5",
            "--expected-ratio",
            "0.75",
            "--output",
            "result.json",
        ]),
        Ok(ParseOutcome::Command(Command::Craft {
            input: "acquisitions".into(),
            output: "result.json".into(),
            regions: vec![
                plotium_processing::craft::CraftRegion::new(
                    plotium_processing::craft::CraftRegionId(0),
                    -0.5,
                    0.2,
                ),
                plotium_processing::craft::CraftRegion::new(
                    plotium_processing::craft::CraftRegionId(1),
                    6.3,
                    6.5,
                ),
            ],
            expected_ratios: vec![0.75],
        }))
    );
}

#[test]
fn batch_parser_requires_workflow_and_manifest_paths() {
    assert_eq!(
        parse(&[
            "plotium-cli",
            "batch",
            "--workflow",
            "workflow.json",
            "--manifest",
            "run.json",
        ]),
        Ok(ParseOutcome::Command(Command::Batch {
            workflow: "workflow.json".into(),
            manifest: "run.json".into(),
        }))
    );
    assert!(parse(&["plotium-cli", "batch", "workflow.json"]).is_err());
}

#[test]
fn text_inspection_includes_mass_spectrometry_statistics() {
    let report = InspectionReport {
        schema: plotium_core::workflow::INSPECTION_SCHEMA,
        format: "sciex-wiff".to_owned(),
        provenance: plotium_core::workflow::ProvenanceReport {
            selected_path: "sample.wiff".into(),
            data_path: "sample.wiff".into(),
            parameter_paths: Vec::new(),
            companion_paths: vec!["sample.wiff.scan".into()],
        },
        dimension: plotium_core::workflow::DimensionReport {
            count: 3,
            shape: vec![2, 42, 1],
        },
        domain: "mass_spectrometry".to_owned(),
        warnings: Vec::new(),
        electrophysiology: None,
        afm: None,
        mass_spectrometry: Some(plotium_core::workflow::MassSpecReport {
            instrument: Some("SCIEX TripleTOF 6600".to_owned()),
            stream_count: 2,
            ms_scan_count: 42,
            chromatograms: vec!["total ion current chromatogram".to_owned()],
        }),
        xrd: None,
        xps: None,
    };

    let output = text_report(&report);

    assert!(output.contains("format: sciex-wiff"));
    assert!(output.contains("mass_spec.streams: 2"));
    assert!(output.contains("mass_spec.scans: 42"));
    assert!(output.contains("mass_spec.chromatograms: total ion current chromatogram"));
}

#[test]
fn workflow_errors_map_to_stable_exit_categories() {
    let status = fail(WorkflowError::FigureUnavailable("NMR 1D"));
    assert_eq!(status, Status::Canvas);
    assert_eq!(Status::Usage as u8, 2);
    assert_eq!(Status::Export as u8, 6);
}

#[test]
fn export_parser_defaults_to_pdf_and_repeats_canvas_selectors() {
    assert_eq!(
        parse(&[
            "plotium-cli",
            "export",
            "figures.plotium",
            "--canvas",
            "Figure 1",
            "--output-dir",
            "out",
            "--canvas",
            "Figure 2",
            "--dpi",
            "300",
            "--overwrite",
        ]),
        Ok(ParseOutcome::Command(Command::Export(
            export::ExportCommand {
                input: "figures.plotium".into(),
                output_dir: "out".into(),
                format: OutputFormat(ExportFormat::Pdf),
                canvases: vec!["Figure 1".into(), "Figure 2".into()],
                dpi: Some(300),
                overwrite: true,
            }
        )))
    );
    let svg = parse(&[
        "plotium-cli",
        "export",
        "figures.plotium",
        "--output-dir",
        "out",
        "--format",
        "svg",
    ]);
    assert!(matches!(
        svg,
        Ok(ParseOutcome::Command(Command::Export(
            export::ExportCommand {
                format: OutputFormat(ExportFormat::Svg),
                ..
            }
        )))
    ));
    assert!(parse(&["plotium-cli", "export", "figures.plotium"]).is_err());
    assert!(parse(&["plotium-cli", "export", "--output-dir", "out"]).is_err());
    assert!(
        parse(&[
            "plotium-cli",
            "export",
            "a.plotium",
            "--output-dir",
            "out",
            "--dpi",
            "0"
        ])
        .is_err()
    );
    assert!(
        parse(&[
            "plotium-cli",
            "export",
            "a.plotium",
            "--output-dir",
            "out",
            "--format",
            "bmp"
        ])
        .is_err()
    );
}

#[test]
fn tool_properties_lists_the_catalog_and_parameter_targets_default() {
    assert_eq!(
        parse(&["plotium-cli", "tool", "properties"]),
        Ok(ParseOutcome::Command(Command::Tool(
            tool::ToolCommand::Properties
        )))
    );
    assert!(parse(&["plotium-cli", "tool", "properties", "extra"]).is_err());
    assert_eq!(
        tool::parameter_targets("properties.set_many"),
        Some(plotium_core::automation::TargetSelector::Explicit { ids: vec![] })
    );
    assert_eq!(tool::parameter_targets("properties.set"), None);
}

#[test]
fn tool_run_input_is_optional() {
    assert_eq!(
        parse(&["plotium-cli", "tool", "run", "--request", "op.json"]),
        Ok(ParseOutcome::Command(Command::Tool(
            tool::ToolCommand::Run {
                input: None,
                request: "op.json".into(),
                output: None,
            }
        )))
    );
    assert_eq!(
        parse(&["plotium-cli", "tool", "run", "a.dx", "--request", "op.json"]),
        Ok(ParseOutcome::Command(Command::Tool(
            tool::ToolCommand::Run {
                input: Some("a.dx".into()),
                request: "op.json".into(),
                output: None,
            }
        )))
    );
    assert!(
        parse(&[
            "plotium-cli",
            "tool",
            "run",
            "a.dx",
            "b.dx",
            "--request",
            "op.json"
        ])
        .is_err()
    );
}
