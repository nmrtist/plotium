//! Lossless source evidence; no sequence model or inferred recovery delay.
use nmr::formats::jeol::{ParameterValue, Parameters};

#[derive(Clone, Debug)]
pub struct JeolDiffusionFacts {
    pub parameters: Parameters,
    pub gradient_axis: Option<usize>,
}

impl JeolDiffusionFacts {
    pub fn seconds(&self, name: &str) -> Option<f64> {
        let record = self.parameters.get(name)?;
        let unit = record.primary_unit();
        // JEOL unit 28 is seconds. Reject compound units rather than guessing.
        if unit.base_code() != 28
            || unit.power() != 1
            || record.raw_units()[2..].iter().any(|&byte| byte != 0)
        {
            return None;
        }
        record.scaled_f64()
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        match self.parameters.get(name)?.value() {
            ParameterValue::String(text) => Some(text.trim()),
            _ => None,
        }
    }

    pub fn has_full_program_identity(&self) -> bool {
        let expected = b"bpp_dste_led_diffusion.jxp";
        self.parameters
            .raw_trailing_records()
            .windows(expected.len())
            .any(|bytes| bytes == expected)
    }

    pub fn embedded_program(&self) -> Option<&str> {
        let bytes = self.parameters.raw_trailing_records();
        let start = bytes.windows(7).position(|window| window == b"\npulse\n")? + 1;
        // Only the pulse section is used as model evidence. Header fields can
        // contain user sample identifiers and are not copied into analysis state.
        let tail = &bytes[start..];
        let end = tail
            .windows(10)
            .position(|window| window == b"end pulse;")?
            + 10;
        std::str::from_utf8(&tail[..end]).ok()
    }
}
