use super::*;

fn file(directory: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn two_theta(path: &Path) -> Vec<f64> {
    let Acquisition::Xrd(data) = load(path).unwrap().acquisition else {
        panic!("expected XRD");
    };
    data.two_theta_deg
}

#[test]
fn only_the_column_line_sets_the_axis() {
    let text = "# --> /data/run_2th_scan/s1.tif\n# sample seq_a1\n#  q_nm^-1  I\n10 5\n20 7\n";
    assert_eq!(
        parse(text).unwrap().header.axis,
        HeaderAxis::Ruler(Ruler::QPerNm)
    );
    let text = "# sample q_nm^-1\n#  2th_deg  I\n10 5\n20 7\n";
    assert_eq!(
        parse(text).unwrap().header.axis,
        HeaderAxis::Ruler(Ruler::TwoThetaDeg)
    );
}

#[test]
fn radians_convert_and_other_diffraction_axes_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    let path = file(directory.path(), "a.xy", "# 2th_rad I\n0.1 5\n0.2 6\n");
    assert!((two_theta(&path)[0] - 0.1_f64.to_degrees()).abs() < 1e-9);
    for axis in ["r_mm", "d_A"] {
        let name = format!("{axis}.xy");
        let path = file(directory.path(), &name, &format!("# {axis} I\n1 5\n2 6\n"));
        assert!(is_integrated_pattern(&path));
        assert!(
            load(&path)
                .unwrap_err()
                .to_string()
                .contains("export it on")
        );
    }
}

#[test]
fn files_of_other_techniques_are_not_taken_for_xrd() {
    let directory = tempfile::tempdir().unwrap();
    for (name, text) in [
        ("larch.chi", "# k chi\n0.0 0.1\n0.05 0.2\n"),
        ("xas.xy", "# Energy I0 It\n7000 1 2\n7001 1 2\n"),
        ("raman.xy", "# Raman shift, intensity\n100 1\n101 2\n"),
        ("xps.xy", "# Binding Energy CPS\n280 1\n281 2\n"),
        ("table.dat", "1.0 2\n2.0 3\n"),
        ("dated.dat", "# measured 12th of May\n1.0 2\n2.0 3\n"),
    ] {
        let path = file(directory.path(), name, text);
        assert!(!is_integrated_pattern(&path), "{name}");
    }
}

#[test]
fn a_bare_file_opens_on_2theta_with_a_warning() {
    let directory = tempfile::tempdir().unwrap();
    let path = file(directory.path(), "a.xy", "0.000 6\n0.015 8\n");
    assert!(is_integrated_pattern(&path));
    assert!(load(&path).unwrap().warnings[0].message.contains("No axis"));
}

#[test]
fn reads_fit2d_chi_headers() {
    let text = "sample.tif\n2-Theta Angle (Degrees)\nIntensity\n   3\n 1.0 5\n 2.0 6\n 3.0 7\n";
    let parsed = parse(text).unwrap();
    assert_eq!(parsed.header.axis, HeaderAxis::Ruler(Ruler::TwoThetaDeg));
    assert_eq!(parsed.rows.len(), 3);
}

#[test]
fn reads_gsas_titles_and_three_columns() {
    let directory = tempfile::tempdir().unwrap();
    let path = file(
        directory.path(),
        "a.xye",
        "LaB6 run 4\n5.0 100 10\n5.1 110 10\n",
    );
    assert!(is_integrated_pattern(&path));
    assert_eq!(two_theta(&path), vec![5.0, 5.1]);
    let comment = parse("/* comment */\n5.0 1\n5.1 2\n").unwrap();
    assert_eq!(comment.header.axis, HeaderAxis::Unstated);
}

#[test]
fn wavelength_units_are_honored() {
    for line in [
        "# Wavelength: 7.0e-11 m",
        "# Wavelength: 7.0e-11",
        "# Wavelength: 0.07 nm",
        "# Wavelength (A): 0.7",
        "# wavelength = 0.7",
    ] {
        let value = wavelength_in(line).unwrap();
        assert!((value - 0.7).abs() < 1e-9, "{line}");
    }
}

#[test]
fn q_without_a_wavelength_is_refused_with_a_reason() {
    let directory = tempfile::tempdir().unwrap();
    let path = file(directory.path(), "a.xy", "# q_A^-1 I\n1.0 5\n2.0 6\n");
    let error = load(&path).unwrap_err().to_string();
    assert!(error.contains("no wavelength"), "{error}");
}

#[test]
fn q_converts_to_two_theta_by_bragg() {
    let directory = tempfile::tempdir().unwrap();
    let text = "# wavelength = 0.7\n# q_A^-1 I\n1.0 5\n2.0 6\n";
    let path = file(directory.path(), "a.xy", text);
    let expected = 2.0 * (0.7 / (4.0 * std::f64::consts::PI)).asin().to_degrees();
    assert!((two_theta(&path)[0] - expected).abs() < 1e-9);
}

#[test]
fn masked_rows_are_skipped_and_negatives_clipped_with_reports() {
    let directory = tempfile::tempdir().unwrap();
    let text = "# 2th_deg I\n1.0 -5\n2.0 nan\n3.0 6\n4.0 -1\n";
    let path = file(directory.path(), "a.xy", text);
    let loaded = load(&path).unwrap();
    let messages: Vec<_> = loaded.warnings.iter().map(|w| w.message.as_str()).collect();
    assert!(messages[0].starts_with("1 masked"), "{messages:?}");
    assert!(messages[1].starts_with("2 negative"), "{messages:?}");
    let Acquisition::Xrd(data) = loaded.acquisition else {
        panic!("expected XRD");
    };
    assert_eq!(data.intensity, vec![0.0, 6.0, 0.0]);
}

#[test]
fn a_descending_pattern_is_reversed_and_a_shuffled_one_refused() {
    let directory = tempfile::tempdir().unwrap();
    let path = file(directory.path(), "a.xy", "3.0 1\n2.0 2\n1.0 3\n");
    assert!(load(&path).is_ok());
    let path = file(directory.path(), "b.xy", "1.0 1\n3.0 2\n2.0 3\n");
    assert!(load(&path).is_err());
}
