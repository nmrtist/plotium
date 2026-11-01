//! Reads real patterns outside the repository. Set `PLOTIUM_XRD_EXAMPLES` to a
//! folder of `.xy` files and run with `--ignored`.
use plotium_io::{Acquisition, load_path};

#[test]
#[ignore = "needs PLOTIUM_XRD_EXAMPLES"]
fn every_pattern_keeps_each_row() {
    let folder = std::env::var_os("PLOTIUM_XRD_EXAMPLES").unwrap();
    let mut count = 0;
    for entry in std::fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "xy") {
            continue;
        }
        let rows = std::fs::read_to_string(&path).unwrap().lines().count();
        let Acquisition::Xrd(data) = load_path(&path).unwrap().acquisition else {
            panic!("{path:?} did not open as XRD");
        };
        assert_eq!(data.len(), rows, "{path:?}");
        count += 1;
    }
    println!("{count} patterns");
    assert!(count > 0);
}
