use super::*;

fn scan() -> XasScan {
    let mut text = String::from("# Energy I0 It\n");
    for index in 0..8 {
        text.push_str(&format!("{} 1000 {}\n", 7000 + index, 900 - index));
    }
    plotium_io::xas::parse(&text).unwrap()
}

fn decode_bytes(bytes: &[u8]) -> Result<XasScan> {
    let mut reader = EntryReader::new(
        std::io::Cursor::new(bytes),
        "test.bin",
        "XAS",
        bytes.len() as u64,
        bytes.len() as u64,
    )?;
    let scan = decode(&mut reader)?;
    reader.finish()?;
    Ok(scan)
}

#[test]
fn scan_survives_a_write_and_read() {
    let scan = scan();
    let mut bytes = Vec::new();
    write(&mut bytes, &scan).unwrap();
    assert_eq!(decode_bytes(&bytes).unwrap(), scan);
}

#[test]
fn a_foreign_signature_is_rejected() {
    assert!(decode_bytes(&[0_u8; 32]).is_err());
}

#[test]
fn project_keeps_the_signal_and_parameters() {
    let mut xas = XasDataset::load(scan());
    let mut params = xas.params;
    params.energy_shift = 1.5;
    params.transform.kweight = 3;
    xas.apply_processing(xas.signal, params);
    let signal = xas.signal;
    let mut app = crate::state::PlotiumApp::new_with_settings(Default::default());
    app.doc.datasets.push(Dataset::Xas(Box::new(xas)));
    let path = crate::project::tests::temp_project("xas-roundtrip");
    crate::project::save_project(&app, &path, false).unwrap();
    let loaded = crate::project::load_project(&path).unwrap();
    let restored = loaded.doc.datasets[0].as_xas().unwrap();
    assert_eq!(restored.signal, signal);
    assert_eq!(restored.params, params);
    assert_eq!(
        restored.scan,
        *app.doc.datasets[0].as_xas().map(|d| &d.scan).unwrap()
    );
    std::fs::remove_file(path).unwrap();
}
