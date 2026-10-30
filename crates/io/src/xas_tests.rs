use super::*;

const XDAC: &str = "XDAC V1.4 Datafile V1
\"sample.000\" created on 1/2/2011 at 1:10:04 PM on X-23A2
Diffraction element= Si (311).  Ring energy= 2.80 GeV
E0= 7112.00
NUM_REGIONS= 4


Offsets= 0.00 37.25 86.25 0.00
standard on tape


--------------------------------------------------------------------------------
        Energy            I0            It            Ir       IntTime
    6912.00137     5004.0000      726.3750     1401.8750        0.5000
    6921.99811     5006.0000      734.3750     1419.8750        0.5000
    6931.99952     5031.0000      745.3750     1446.8750        0.5000
    6942.00100     5041.0000      754.3750     1466.8750        0.5000
";

const LABVIEW: &str = "# 1-D Scan File created by LabVIEW Control Panel  1/2/2007  8:39:21 PM
#
# Beamline 20 BM
#
# User Comment:
# dilute standard in buffer
#
# Detector Names/PVs:
# I0/20bm:scaler1_netA.C  I1/20bm:scaler1_netA.D
# Here is a readable list of column headings:
#  1) Mono Energy *         6) Iref
#  2) Scaler preset time *  7) DTcorr_I0
#  3) I0                    8) DXP12:total_Sum
#  4) I1                    9) DXP12:Hg_Sum
#  5) Preslit              10) DXP12:As_Sum
#
# Column Headings:
#Mono Energy *        Scaler preset time * I0                   I1                   Preslit Iref
       12100.25  10.0  1000.5  900.5  3.0  10.5  990.5  5000.0  500.0  1000.0
       12110.25  10.0  1001.5  901.5  3.0  10.5  991.5  5001.0  501.0  1001.0
       12120.25  10.0  1002.5  902.5  3.0  10.5  992.5  5002.0  502.0  1002.0
       12130.25  10.0  1003.5  903.5  3.0  10.5  993.5  5003.0  503.0  1003.0
";

const XDI: &str = "# XDI/1.0 GSE/1.0
# Column.1: energy keV
# Column.2: i0
# Column.3: itrans
# Column.4: irefer
# Column.5: mutrans
# Element.symbol: Cu
# Element.edge: K
# Facility.name: APS
# Beamline.name: 13-ID
# Mono.d_spacing: 3.13555
# ///
# copper foil
#----
# energy i0 itrans irefer mutrans
8.97900 100.0 50.0 25.0 0.693
8.97925 101.0 50.0 25.0 0.703
8.97950 102.0 50.0 25.0 0.713
8.97975 103.0 50.0 25.0 0.723
";

#[test]
fn reads_xdac_scan() {
    let scan = parse(XDAC).unwrap();
    assert_eq!(scan.kind, XasFileKind::Xdac);
    assert_eq!(scan.energy().len(), 4);
    assert_eq!(scan.energy()[1], 6921.99811);
    let channels = &scan.channels;
    assert_eq!(channels.energy, 0);
    assert_eq!(scan.columns[channels.i0.unwrap()].name, "I0");
    assert_eq!(scan.columns[channels.transmission.unwrap()].name, "It");
    assert_eq!(scan.columns[channels.reference.unwrap()].name, "Ir");
    assert!(channels.fluorescence.is_empty());
    assert_eq!(scan.beamline.as_deref(), Some("X-23A2"));
    assert_eq!(scan.sample.as_deref(), Some("standard on tape"));
    assert_eq!(scan.nominal_e0, Some(7112.0));
    assert!(scan.warnings.is_empty());
}

#[test]
fn reads_labview_scan_with_multi_word_labels() {
    let scan = parse(LABVIEW).unwrap();
    assert_eq!(scan.kind, XasFileKind::Columns);
    assert_eq!(scan.columns[0].name, "Mono Energy");
    assert_eq!(scan.columns[1].name, "Scaler preset time");
    let channels = &scan.channels;
    assert_eq!(scan.columns[channels.energy].name, "Mono Energy");
    assert_eq!(scan.columns[channels.i0.unwrap()].name, "I0");
    assert_eq!(scan.columns[channels.transmission.unwrap()].name, "I1");
    assert_eq!(scan.columns[channels.reference.unwrap()].name, "Iref");
    assert_eq!(
        scan.columns[channels.fluorescence[0]].name, "DXP12:Hg_Sum",
        "the first element sum outranks the total"
    );
    assert_eq!(channels.fluorescence.len(), 2);
    assert_eq!(scan.beamline.as_deref(), Some("20 BM"));
    assert_eq!(scan.sample.as_deref(), Some("dilute standard in buffer"));
}

#[test]
fn reads_xdi_and_converts_kev() {
    let scan = parse(XDI).unwrap();
    assert_eq!(scan.kind, XasFileKind::Xdi);
    assert!((scan.energy()[0] - 8979.0).abs() < 1e-9);
    assert_eq!(scan.channels.transmission, Some(2));
    assert_eq!(scan.channels.reference, Some(3));
    assert_eq!(scan.channels.mu, Some(4));
    assert_eq!(scan.beamline.as_deref(), Some("APS 13-ID"));
    assert_eq!(scan.element.as_deref(), Some("Cu"));
    assert_eq!(scan.edge.as_deref(), Some("K"));
    assert_eq!(scan.sample.as_deref(), Some("copper foil"));
    assert_eq!(
        scan.metadata.get("Mono.d_spacing").map(String::as_str),
        Some("3.13555")
    );
}

#[test]
fn reads_plain_columns_and_guesses_kev() {
    let text = "# energy mu\n8.9790 0.1\n8.9792 0.2\n8.9794 0.3\n8.9796 0.4\n";
    let scan = parse(text).unwrap();
    assert!((scan.energy()[1] - 8979.2).abs() < 1e-9);
    assert_eq!(scan.channels.mu, Some(1));
    assert_eq!(scan.warnings.len(), 1);
}

#[test]
fn reads_a_header_without_comment_marks() {
    let text = "Energy I0 It\n7000 1 2\n7001 1 2\n7002 1 2\n7003 1 2\n";
    let scan = parse(text).unwrap();
    assert_eq!(scan.channels.transmission, Some(2));
    assert!(scan.warnings.is_empty());
}

#[test]
fn unlabeled_columns_default_to_the_first_as_energy() {
    let text = "7000 1\n7001 2\n7002 3\n7003 4\n";
    let scan = parse(text).unwrap();
    assert_eq!(scan.columns[0].name, "col1");
    assert_eq!(scan.channels.energy, 0);
    assert!(scan.warnings[0].contains("col1"));
}

#[test]
fn drops_only_a_truncated_final_row() {
    let text = "# energy i0 it\n7000 1 2\n7001 1 2\n7002 1 2\n7003 1 2\n7004 1\n";
    let scan = parse(text).unwrap();
    assert_eq!(scan.energy().len(), 4);
    assert!(scan.warnings[0].contains("line 6"));

    let text = "# energy i0 it\n7000 1 2\n7001 1\n7002 1 2\n7003 1 2\n7004 1 2\n";
    assert!(matches!(parse(text), Err(IoError::InvalidXas(_))));
}

#[test]
fn rejects_non_scan_text() {
    for text in [
        "",
        "hello world\nnot numeric\n",
        "7000\n7001\n7002\n7003\n",
        "# energy i0\n7000 1\n7001 2\n",
        "# XDI/1.0\n# Column.1: angle degrees\n# Column.2: i0\n#----\n1 1\n2 1\n3 1\n4 1\n",
        "# energy i0\n1000000 1\n1000001 2\n1000002 3\n1000003 4\n",
        "# energy i0\n30.0 1\n29.999 2\n29.998 3\n29.997 4\n",
    ] {
        assert!(
            matches!(parse(text), Err(IoError::InvalidXas(_))),
            "{text:?}"
        );
    }
}

#[test]
fn keeps_row_order_of_a_descending_scan() {
    let text = "# energy i0\n7003 1\n7002 2\n7001 3\n7000 4\n";
    assert_eq!(parse(text).unwrap().energy()[0], 7003.0);
}

#[test]
fn prefers_a_mono_energy_over_other_energies() {
    let text = "# Ring_Energy Mono_Energy I0\n2800 7000 1\n2800 7001 1\n2800 7002 1\n2800 7003 1\n";
    assert_eq!(parse(text).unwrap().channels.energy, 1);
}

#[test]
fn does_not_mistake_detector_columns_for_i0() {
    let text = "# Energy IC0 ROI0 ROI1\n7000 1 2 3\n7001 1 2 3\n7002 1 2 3\n7003 1 2 3\n";
    assert_eq!(parse(text).unwrap().channels.i0, None);
    let text = "# Energy DTcorr_I0 It\n7000 1 2\n7001 1 2\n7002 1 2\n7003 1 2\n";
    assert_eq!(parse(text).unwrap().channels.i0, Some(1));
}

#[test]
fn skips_monitors_and_normalized_data_as_fluorescence() {
    let text = "# Energy I0 I0_Sum normfluor Hg_Sum\n\
                7000 1 1 1 1\n7001 1 1 1 1\n7002 1 1 1 1\n7003 1 1 1 1\n";
    assert_eq!(parse(text).unwrap().channels.fluorescence, vec![4]);
}

#[test]
fn warns_when_energy_is_too_small_for_ev() {
    let text = "# energy i0\n8.00 1\n8.01 1\n8.02 1\n8.03 1\n";
    assert!(parse(text).unwrap().warnings[0].contains("120 eV"));
}

#[test]
fn takes_units_from_the_chosen_column() {
    let text = "# energy_keV i0\n8.00 1\n8.01 1\n8.02 1\n8.03 1\n";
    assert_eq!(parse(text).unwrap().energy()[0], 8000.0);
    let text = "# XDI/1.0\n# Column.1: energy eV\n# Column.2: readback keV\n#----\n\
                7000 7\n7001 7\n7002 7\n7003 7\n";
    let scan = parse(text).unwrap();
    assert_eq!(scan.energy()[0], 7000.0);
    assert_eq!(scan.columns[1].values[0], 7.0);
}

#[test]
fn ignores_a_dashed_rule_below_the_labels() {
    let text = "# Energy I0 It\n# ------ -- --\n7000 1 2\n7001 1 2\n7002 1 2\n7003 1 2\n";
    assert_eq!(parse(text).unwrap().channels.transmission, Some(2));
}

#[test]
fn reads_an_edge_energy_with_a_unit_and_reports_a_bad_one() {
    let scan = parse(&XDAC.replace("E0= 7112.00", "E0= 7112 eV")).unwrap();
    assert_eq!(scan.nominal_e0, Some(7112.0));
    let scan = parse(&XDAC.replace("E0= 7112.00", "E0= unknown")).unwrap();
    assert_eq!(scan.nominal_e0, None);
    assert!(scan.warnings[0].contains("E0"));
}
