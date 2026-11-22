use super::*;
use plotium_figure::{AxisFrame, ColormapId, Contour, ContourStyle, HeatmapGrid, Series};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfmForceBranch {
    Approach,
    Retract,
}

impl AfmForceBranch {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Approach => "Approach",
            Self::Retract => "Retract",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AfmCurveAxis<'a> {
    pub name: &'static str,
    pub unit: &'a str,
}

impl AfmCurveAxis<'_> {
    pub fn label(self) -> String {
        if self.unit.is_empty() {
            self.name.into()
        } else {
            format!("{} ({})", self.name, self.unit)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfmForceXSource {
    ReconstructedZ,
    SamplePeriod,
    SampleIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfmDeflectionSource {
    StoredLength,
    VoltageSensitivity,
    MissingVoltageSensitivity,
    UnsupportedSignalUnit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AfmForceCurveContext<'a> {
    pub x: AfmCurveAxis<'a>,
    pub y: AfmCurveAxis<'a>,
    pub x_source: AfmForceXSource,
    pub deflection_source: AfmDeflectionSource,
    pub spring_constant_n_per_m: Option<f64>,
    pub deflection_sensitivity_m_per_v: Option<f64>,
    pub signal_unit: &'a str,
    deflection_to_metres: Option<f64>,
}

impl<'a> AfmForceCurveContext<'a> {
    fn from_force_set(forces: &'a plotium_io::AfmForceSet) -> Self {
        let (x_source, x_name, x_unit) = if forces.z_positions.is_some() {
            (AfmForceXSource::ReconstructedZ, "Relative Z", "nm")
        } else if forces.sample_period_s.is_some() {
            (AfmForceXSource::SamplePeriod, "Branch time", "s")
        } else {
            (AfmForceXSource::SampleIndex, "Sample index", "")
        };
        let spring_constant_n_per_m = positive_finite(forces.spring_constant_n_per_m);
        let deflection_sensitivity_m_per_v = positive_finite(forces.deflection_sensitivity_m_per_v);
        let scale_unit = forces.signal_scale.unit.trim();
        // Applying the channel scale consumes its per-count denominator.
        let signal_unit = scale_unit
            .split_once('/')
            .filter(|(_, denominator)| denominator.trim().eq_ignore_ascii_case("LSB"))
            .map_or(scale_unit, |(unit, _)| unit.trim());
        // The stored scale may have already applied the header's sensitivity.
        let quantity_unit = scale_unit
            .split_once('/')
            .map_or(scale_unit, |(unit, _)| unit.trim());
        let (deflection_source, deflection_to_metres) = match physical_length_factor(quantity_unit)
        {
            Some(factor) => (AfmDeflectionSource::StoredLength, Some(factor)),
            None if quantity_unit.eq_ignore_ascii_case("V") => match deflection_sensitivity_m_per_v
            {
                Some(factor) => (AfmDeflectionSource::VoltageSensitivity, Some(factor)),
                None => (AfmDeflectionSource::MissingVoltageSensitivity, None),
            },
            None => (AfmDeflectionSource::UnsupportedSignalUnit, None),
        };
        let (y_name, y_unit) = match (deflection_to_metres, spring_constant_n_per_m) {
            (Some(_), Some(_)) => ("Force", "nN"),
            (Some(_), None) => ("Deflection", "nm"),
            (None, _) => ("Signal", signal_unit),
        };
        Self {
            x: AfmCurveAxis {
                name: x_name,
                unit: x_unit,
            },
            y: AfmCurveAxis {
                name: y_name,
                unit: y_unit,
            },
            x_source,
            deflection_source,
            spring_constant_n_per_m,
            deflection_sensitivity_m_per_v,
            signal_unit,
            deflection_to_metres,
        }
    }
}

#[derive(Clone)]
pub struct AfmDataset {
    pub resource_id: DatasetId,
    /// Persisted mapping from stable channel keys to dataset-local field ids.
    pub field_catalog: FieldCatalog,
    pub organization: super::DatasetOrganization,
    pub acquisition_identity: plotium_io::AcquisitionIdentity,
    pub data: Arc<AfmData>,
    /// Immutable keys calculated with the loaded raster data, never serialized.
    pub(crate) image_field_keys: Arc<[String]>,
    pub name: Option<String>,
    pub selected_pixel: [usize; 2],
    pub lineage: Option<DatasetLineage>,
}

impl AfmDataset {
    pub(super) fn force_increment_nn(&self) -> Option<f64> {
        let forces = self.data.forces.as_ref()?;
        let context = self.force_curve_context()?;
        // The scale offset cancels when taking the difference for one count.
        positive_finite(Some(force_nn(
            forces.signal_scale.multiplier.abs(),
            context.deflection_to_metres?,
            context.spring_constant_n_per_m?,
        )))
    }

    pub fn load(data: AfmData) -> Self {
        let acquisition_identity =
            plotium_io::AcquisitionIdentity::from_path(std::path::Path::new(&data.source));
        let selected_pixel = data.forces.as_ref().map_or([0, 0], |forces| {
            [forces.grid_width / 2, forces.grid_height / 2]
        });
        let image_field_keys = crate::state::afm_channel_keys(&data);
        let mut field_catalog = crate::state::afm_field_catalog_for_keys(&data, &image_field_keys);
        field_catalog.attach_provenance(&data.source, None);
        Self {
            resource_id: DatasetId::new(),
            field_catalog,
            organization: Default::default(),
            acquisition_identity,
            data: Arc::new(data),
            image_field_keys,
            name: None,
            selected_pixel,
            lineage: None,
        }
    }

    pub fn map_figure(&self, field: FieldId, colormap: ColormapId) -> Option<Figure> {
        let channel = self.image_for_field(field)?;
        let values: Vec<f32> = channel
            .raw
            .iter()
            .map(|value| channel.scale.apply(*value) as f32)
            .collect();
        let (minimum, maximum) = values
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), value| {
                (lo.min(value), hi.max(value))
            });
        let mut figure = Figure::new(
            &channel.name,
            Axis::new(&channel.lateral_unit, 0.0, channel.scan_size_x),
            Axis::new(&channel.lateral_unit, 0.0, channel.scan_size_y),
        );
        figure.heatmap = Some(HeatmapGrid {
            rows: channel.height,
            cols: channel.width,
            values,
            x_bounds: [0.0, channel.scan_size_x],
            y_bounds: [0.0, channel.scan_size_y],
            x_edges: None,
            y_edges: None,
            colormap,
            value_range: [minimum, maximum],
        });
        figure.lock_aspect = true;
        figure.axis_frame = AxisFrame::Box;
        Some(figure)
    }

    pub(crate) fn contour_base_figure(&self, field: FieldId) -> Option<Figure> {
        let channel = self.image_for_field(field)?;
        let mut figure = Figure::new(
            &channel.name,
            Axis::new(&channel.lateral_unit, 0.0, channel.scan_size_x),
            Axis::new(&channel.lateral_unit, 0.0, channel.scan_size_y),
        );
        figure.lock_aspect = true;
        figure.axis_frame = AxisFrame::Box;
        Some(figure)
    }

    pub(crate) fn contour_figure_from_geometry(
        &self,
        field: FieldId,
        geometry: &ContourGeometry,
        style: &ContourStyle,
    ) -> Option<Figure> {
        let mut figure = self.contour_base_figure(field)?;
        if !geometry.positive.is_empty() {
            figure.contours.push(Contour {
                origin: None,
                segments: geometry.positive.as_ref().to_vec(),
                color: style.positive_color.resolve(),
                width: style.width.get(),
            });
        }
        if !geometry.negative.is_empty() {
            figure.contours.push(Contour {
                origin: None,
                segments: geometry.negative.as_ref().to_vec(),
                color: style.negative_color.resolve(),
                width: style.width.get(),
            });
        }
        Some(figure)
    }

    pub fn force_figure(&self, field: FieldId) -> Option<Figure> {
        (self.field_catalog.id_for_key("afm.force_curve") == Some(field)).then_some(())?;
        let forces = self.data.forces.as_ref()?;
        let context = AfmForceCurveContext::from_force_set(forces);
        let curve = forces.curve_raw(self.selected_pixel[0], self.selected_pixel[1])?;
        let x_value = |display_index: usize| {
            forces
                .z_positions
                .as_ref()
                .and_then(|axis| axis.get(display_index).copied())
                .or_else(|| {
                    let branch_index = if display_index < forces.approach_samples {
                        display_index
                    } else {
                        display_index - forces.approach_samples
                    };
                    forces
                        .sample_period_s
                        .map(|period| branch_index as f64 * period)
                })
                .unwrap_or_else(|| {
                    if display_index < forces.approach_samples {
                        display_index as f64
                    } else {
                        (display_index - forces.approach_samples) as f64
                    }
                })
        };
        let calibrated = |raw: i32| {
            let stored = forces.signal_scale.apply(raw);
            match (
                context.deflection_to_metres,
                context.spring_constant_n_per_m,
            ) {
                (Some(to_metres), Some(spring)) => force_nn(stored, to_metres, spring),
                (Some(to_metres), None) => stored * to_metres * 1.0e9,
                (None, _) => stored,
            }
        };
        let ordered: Vec<[f64; 2]> = forces
            .display_order
            .iter()
            .enumerate()
            .filter_map(|(display_index, &raw_index)| {
                curve
                    .get(raw_index)
                    .map(|&raw| [x_value(display_index), calibrated(raw)])
            })
            .collect();
        let split = forces.approach_samples.min(ordered.len());
        let (x_min, x_max) = ordered
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), point| {
                (lo.min(point[0]), hi.max(point[0]))
            });
        let (y_min, y_max) = ordered
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), point| {
                (lo.min(point[1]), hi.max(point[1]))
            });
        let mut figure = Figure::new(
            format!("{} Curve", context.y.name),
            Axis::new(context.x.label(), x_min, x_max),
            Axis::new(context.y.label(), y_min, y_max),
        );
        figure.series.push(Series::line(
            AfmForceBranch::Approach.name(),
            ordered[..split].to_vec(),
        ));
        figure.series.push(
            Series::line(AfmForceBranch::Retract.name(), ordered[split..].to_vec())
                .colored(Color::rgb(0xd1, 0x24, 0x2a)),
        );
        Some(figure)
    }

    pub fn force_curve_context(&self) -> Option<AfmForceCurveContext<'_>> {
        self.data
            .forces
            .as_ref()
            .map(AfmForceCurveContext::from_force_set)
    }

    pub fn force_branch_sample_count(&self, branch: AfmForceBranch) -> usize {
        let Some(forces) = &self.data.forces else {
            return 0;
        };
        let approach = forces.approach_samples.min(forces.samples_per_curve);
        match branch {
            AfmForceBranch::Approach => approach,
            AfmForceBranch::Retract => forces.samples_per_curve - approach,
        }
    }

    fn image_for_field(&self, field: FieldId) -> Option<&plotium_io::AfmImageChannel> {
        self.data
            .images
            .iter()
            .zip(self.image_field_keys.iter())
            .find(|(_, key)| self.field_catalog.id_for_key(key) == Some(field))
            .map(|(channel, _)| channel)
    }
}

impl PlotiumApp {
    pub fn afm_force_plot_dataset(&self, canvas: usize, object: ObjectId) -> Option<DatasetId> {
        let plot = self.doc.canvases.get(canvas)?.object(object)?.plot()?;
        let dataset = plot.primary_dataset()?;
        (plot.chart.type_id == "afm_force_curve"
            && self.doc.dataset_by_id(dataset)?.as_afm().is_some())
        .then_some(dataset)
    }
}

fn force_nn(stored: f64, to_metres: f64, spring: f64) -> f64 {
    stored * to_metres * spring * 1.0e9
}

fn positive_finite(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

fn physical_length_factor(unit: &str) -> Option<f64> {
    let normalized = unit.to_ascii_lowercase().replace(['µ', '~'], "u");
    match normalized.as_str() {
        "m" => Some(1.0),
        "mm" => Some(1.0e-3),
        "um" => Some(1.0e-6),
        "nm" => Some(1.0e-9),
        "pm" => Some(1.0e-12),
        _ => None,
    }
}
