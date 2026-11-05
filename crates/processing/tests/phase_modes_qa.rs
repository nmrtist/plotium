use nmr::ExecutionContext;
use plotium_processing::nmr_volume::{NmrVolume, VolumePhase};
#[test]
fn synthetic_encoded_modes_decode_and_preview() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/phase-qa-final/modes")
        .join(std::process::id().to_string());
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../io/tests/fixtures/nmr/bruker-3d-qf");
    for mode in [4, 5, 6] {
        let dir = root.join(mode.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(fixture.join("acqus"), dir.join("acqus")).unwrap();
        for file in ["acqu2s", "acqu3s"] {
            let text = std::fs::read_to_string(fixture.join(file))
                .unwrap()
                .replace("FnMODE= 1", &format!("FnMODE= {mode}"))
                .replace("TD= 3", "TD= 6")
                .replace("TD= 5", "TD= 10");
            std::fs::write(dir.join(file), text).unwrap();
        }
        let bytes: Vec<u8> = (0_i32..5 * 2 * 3 * 2 * 4 * 2)
            .flat_map(|i| (i % 23 - 11).to_le_bytes())
            .collect();
        std::fs::write(dir.join("ser"), bytes).unwrap();
        let source = plotium_io::nmr_view::NmrSource::new(
            plotium_io::nmr_bridge::read(&dir, &mut ExecutionContext::default()).unwrap(),
        )
        .unwrap();
        let baseline = [VolumePhase::default(); 3];
        let volume = NmrVolume::process_source(&source).unwrap();
        assert!(volume.supports_local_phase_preview());
        assert!((0..3).all(|i| volume.supports_phase(i)));
        let draft = [VolumePhase {
            p0_degrees: 31.,
            p1_degrees: -71.,
            pivot_fraction: 0.23,
        }; 3];
        let applied = NmrVolume::process(
            &source,
            draft,
            Default::default(),
            &mut ExecutionContext::default(),
        )
        .unwrap();
        for fixed in 0..3 {
            for index in 0..volume.shape()[fixed] {
                let p = volume
                    .phase_preview_plane(
                        fixed,
                        index,
                        baseline,
                        draft,
                        &mut ExecutionContext::default(),
                    )
                    .unwrap();
                let e = applied.plane_source(fixed, index).unwrap();
                for (a, b) in p
                    .dataset()
                    .as_processed()
                    .unwrap()
                    .data()
                    .samples()
                    .iter()
                    .zip(e.dataset().as_processed().unwrap().data().samples())
                {
                    assert!((a - b).abs() < 1e-8);
                }
            }
        }
        eprintln!(
            "synthetic mode={mode} shape={:?} bases={:?}",
            volume.shape(),
            volume
                .axes()
                .iter()
                .map(|a| a.component_basis())
                .collect::<Vec<_>>()
        );
    }
}
