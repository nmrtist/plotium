use super::*;
use crate::actions::Action;
use plotium_analysis::afm_hertz::{
    HertzMethod, HertzOptions, HertzReference, HertzResult, HertzWindow, fit_hertz_with_options,
};
use std::sync::Arc;

#[path = "afm_hertz_suggestion.rs"]
mod suggestion;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AfmHertzWindow {
    #[default]
    RelativeZ,
    ForceNn,
    ForcePercent,
}

impl AfmHertzWindow {
    pub const ALL: [Self; 3] = [Self::RelativeZ, Self::ForceNn, Self::ForcePercent];

    pub fn label(self) -> &'static str {
        match self {
            Self::RelativeZ => "Relative Z (nm)",
            Self::ForceNn => "Force (nN)",
            Self::ForcePercent => "Force (%)",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AfmHertzReference {
    #[default]
    Zero,
    Automatic,
    Manual,
}

#[derive(Clone)]
pub struct AfmHertzDraft {
    pub target: AfmFitTarget,
    pub spring_confirmed: bool,
    pub radius_nm: String,
    pub contact_z_nm: String,
    pub baseline_z_nm: [String; 2],
    pub fit_z_nm: [String; 2],
    pub linearized: bool,
    pub window: AfmHertzWindow,
    pub fit_force_nn: [String; 2],
    pub fit_force_percent: [String; 2],
    pub reference: AfmHertzReference,
    pub reference_nn: String,
    pub error: Option<String>,
    pub suggestion_error: Option<String>,
    result: Option<Arc<AfmHertzRun>>,
    suggestion: Option<suggestion::AfmHertzProposal>,
}

struct AfmHertzRun {
    target: AfmFitTarget,
    pixel: [usize; 2],
    data: Arc<plotium_io::AfmData>,
    raw_indices: Vec<usize>,
    result: HertzResult,
}

impl AfmHertzDraft {
    pub fn new(target: AfmFitTarget) -> Self {
        Self {
            target,
            spring_confirmed: false,
            radius_nm: String::new(),
            contact_z_nm: String::new(),
            baseline_z_nm: Default::default(),
            fit_z_nm: Default::default(),
            linearized: false,
            window: AfmHertzWindow::default(),
            fit_force_nn: Default::default(),
            fit_force_percent: Default::default(),
            reference: AfmHertzReference::default(),
            reference_nn: String::new(),
            error: None,
            suggestion_error: None,
            result: None,
            suggestion: None,
        }
    }

    pub fn input(&self, spring_constant_n_per_m: f64) -> Result<HertzOptions, String> {
        if !self.spring_confirmed {
            return Err("Confirm the file's spring constant before calculating.".into());
        }
        self.options(spring_constant_n_per_m)
    }

    pub fn options(&self, spring_constant_n_per_m: f64) -> Result<HertzOptions, String> {
        let number = |value: &str, field| {
            value
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .ok_or_else(|| format!("Enter a finite {field}."))
        };
        let range = |values: &[String; 2], name| {
            Ok::<_, String>([number(&values[0], name)?, number(&values[1], name)?])
        };
        Ok(HertzOptions {
            spring_constant_n_per_m,
            radius_nm: number(&self.radius_nm, "tip radius (nm)")?,
            baseline_z_nm: range(&self.baseline_z_nm, "baseline bound (nm)")?,
            method: if self.linearized {
                HertzMethod::Linearized
            } else {
                HertzMethod::ManualContact(number(&self.contact_z_nm, "contact Z (nm)")?)
            },
            window: match self.window {
                AfmHertzWindow::RelativeZ => {
                    HertzWindow::RelativeZ(range(&self.fit_z_nm, "fit bound (nm)")?)
                }
                AfmHertzWindow::ForceNn => {
                    HertzWindow::ForceNn(range(&self.fit_force_nn, "fit bound (nN)")?)
                }
                AfmHertzWindow::ForcePercent => {
                    HertzWindow::ForcePercent(range(&self.fit_force_percent, "fit bound (%)")?)
                }
            },
            reference: match self.reference {
                AfmHertzReference::Zero => HertzReference::Zero,
                AfmHertzReference::Automatic => HertzReference::Automatic,
                AfmHertzReference::Manual => {
                    HertzReference::Manual(number(&self.reference_nn, "reference force (nN)")?)
                }
            },
        })
    }
}

impl PlotiumApp {
    fn afm_hertz_source(&self, target: AfmFitTarget) -> Result<(&AfmDataset, f64), &'static str> {
        if target.branch != AfmForceBranch::Approach {
            return Err("Hertz fitting uses the approach branch.");
        }
        self.afm_fit_plot(target)
            .ok_or("Select the source AFM force curve.")?;
        let afm = self
            .doc
            .dataset_by_id(target.source)
            .and_then(Dataset::as_afm)
            .ok_or("The AFM source is unavailable.")?;
        // The NanoScope loader uses this extension to identify PeakForce Capture.
        // Its first displayed half-cycle is not a verified loading branch.
        if std::path::Path::new(&afm.data.source)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pfc"))
        {
            return Err(
                "Use a calibrated non-PeakForce approach curve. Hertz fitting does not support PeakForce Capture.",
            );
        }
        let forces = afm.data.forces.as_ref().ok_or("No AFM force curve.")?;
        let context = afm.force_curve_context().ok_or("No AFM force curve.")?;
        if !forces.z_positions.as_ref().is_some_and(|axis| {
            forces.approach_samples >= 6 && axis.len() >= forces.approach_samples
        }) {
            return Err("Hertz fitting needs an approach curve with relative Z in nm.");
        }
        let spring = context
            .spring_constant_n_per_m
            .ok_or("Hertz fitting needs a valid file spring constant.")?;
        if !matches!(
            context.deflection_source,
            AfmDeflectionSource::StoredLength | AfmDeflectionSource::VoltageSensitivity
        ) {
            return Err("Hertz fitting needs calibrated deflection.");
        }
        Ok((afm, spring))
    }

    pub fn afm_hertz_calibration(&self, target: AfmFitTarget) -> Result<f64, &'static str> {
        self.afm_hertz_source(target).map(|(_, spring)| spring)
    }

    pub fn afm_hertz_points(&self, target: AfmFitTarget) -> Result<Vec<[f64; 2]>, &'static str> {
        let (afm, _) = self.afm_hertz_source(target)?;
        afm.field_catalog
            .id_for_key("afm.force_curve")
            .and_then(|field| afm.force_figure(field))
            .and_then(|figure| {
                figure
                    .series
                    .into_iter()
                    .find(|series| series.name == AfmForceBranch::Approach.name())
            })
            .map(|series| series.points)
            .filter(|points| {
                points.len() == afm.force_branch_sample_count(AfmForceBranch::Approach)
            })
            .ok_or("The selected AFM approach curve is incomplete.")
    }

    pub fn run_afm_hertz(&mut self) -> Result<(), String> {
        let draft = self
            .session
            .ui
            .afm_hertz_task
            .as_mut()
            .ok_or("Open Hertz Fit first.")?;
        draft.result = None;
        let target = draft.target;
        let (afm, spring) = self.afm_hertz_source(target)?;
        let draft = self
            .session
            .ui
            .afm_hertz_task
            .as_ref()
            .ok_or("Open Hertz Fit first.")?;
        let input = draft.input(spring)?;
        let result = fit_hertz_with_options(&self.afm_hertz_points(target)?, input)
            .map_err(|error| error.to_string())?;
        let raw_indices = afm
            .data
            .forces
            .as_ref()
            .and_then(|forces| {
                result
                    .sample_indices
                    .iter()
                    .map(|&index| forces.display_order.get(index).copied())
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or("The selected AFM samples could not be mapped to their original indices.")?;
        let run = Arc::new(AfmHertzRun {
            target,
            pixel: afm.selected_pixel,
            data: afm.data.clone(),
            raw_indices,
            result,
        });
        self.session
            .ui
            .afm_hertz_task
            .as_mut()
            .ok_or("Open Hertz Fit first.")?
            .result = Some(run);
        Ok(())
    }

    fn current_afm_hertz_run(&self) -> Option<&AfmHertzRun> {
        let draft = self.session.ui.afm_hertz_task.as_ref()?;
        let run = draft.result.as_deref()?;
        let (afm, spring) = self.afm_hertz_source(draft.target).ok()?;
        (run.target == draft.target
            && run.pixel == afm.selected_pixel
            && Arc::ptr_eq(&run.data, &afm.data)
            && draft.input(spring).ok()? == run.result.options)
            .then_some(run)
    }

    pub fn afm_hertz_result(&self) -> Option<&HertzResult> {
        self.current_afm_hertz_run().map(|run| &run.result)
    }

    pub fn create_afm_hertz_output(&mut self) -> Result<(), String> {
        let run = self
            .current_afm_hertz_run()
            .ok_or("Calculate a Hertz fit for the current inputs and source curve first.")?;
        let source = run.target.source;
        let name = format!(
            "{} - Approach Hertz Fit",
            self.doc
                .dataset_by_id(source)
                .ok_or("The AFM source is unavailable.")?
                .display_name()
        );
        let mut table = hertz_table(run)?;
        table.name = Some(name);
        table.lineage = Some(DatasetLineage::new(
            DerivationKind::AfmHertzFitTable,
            [source],
        ));
        let size = table.board_rect_pt();
        table.board_pos = next_board_frame_pos(self, [size.width, size.height]);
        let id = table.resource_id;
        self.try_execute_action(Action::CreateTableData {
            dataset: Box::new(Dataset::Table(Box::new(table))),
        })
        .map_err(|error| error.to_string())?;
        let index = self
            .doc
            .dataset_index(id)
            .ok_or("The Hertz fit table is unavailable.")?;
        self.reveal_board_frame(FrameRef::Sheet(index));
        self.session.ui.sheet_open = Some(index);
        Ok(())
    }
}

fn hertz_table(run: &AfmHertzRun) -> Result<TableDataset, String> {
    let rows = &run.result.rows;
    let points =
        |name: &str, unit: &str, f: &dyn Fn(&plotium_analysis::afm_hertz::HertzRow) -> f64| {
            FloatSeries {
                name: name.into(),
                unit: unit.into(),
                values: rows.iter().map(|row| Some(f(row))).collect(),
                uncertainty: None,
                fit: None,
            }
        };
    let mut columns = vec![
        points("Measured force", "nN", &|row| row.corrected_force_nn),
        points("Hertz prediction", "nN", &|row| row.predicted_force_nn),
        points("Residual", "nN", &|row| row.residual_nn()),
        points("Relative Z", "nm", &|row| row.z_nm),
    ];
    let input = run.result.options;
    let result = &run.result;
    let (method, zero_label, baseline_label, zero_z) = match input.method {
        HertzMethod::ManualContact(contact) => (
            "Manual Contact Hertz",
            "Contact Z",
            "Baseline force at contact",
            contact,
        ),
        HertzMethod::Linearized => (
            "Linearized Hertz",
            "Fitted extrapolated zero",
            "Baseline force at fitted zero",
            result
                .extrapolated_zero_z_nm
                .ok_or("The fitted zero is unavailable.")?,
        ),
    };
    let (window, bound_labels, unit, bounds) = match input.window {
        HertzWindow::RelativeZ(bounds) => (
            AfmHertzWindow::RelativeZ,
            ["Fit Z min", "Fit Z max"],
            "nm",
            bounds,
        ),
        HertzWindow::ForceNn(bounds) => (
            AfmHertzWindow::ForceNn,
            ["Fit force min", "Fit force max"],
            "nN",
            bounds,
        ),
        HertzWindow::ForcePercent(bounds) => (
            AfmHertzWindow::ForcePercent,
            ["Fit force min (%)", "Fit force max (%)"],
            "%",
            bounds,
        ),
    };
    let legacy = matches!(input.method, HertzMethod::ManualContact(_))
        && window == AfmHertzWindow::RelativeZ
        && input.reference == HertzReference::Zero;
    let mut metadata = vec![
        ("Reduced modulus E*", "MPa", result.reduced_modulus_mpa),
        ("Tip radius", "nm", input.radius_nm),
        ("File spring constant", "N/m", input.spring_constant_n_per_m),
        (zero_label, "nm", zero_z),
        ("Baseline Z min", "nm", input.baseline_z_nm[0]),
        ("Baseline Z max", "nm", input.baseline_z_nm[1]),
        ("Baseline slope", "nN/nm", result.baseline_slope_nn_per_nm),
        (baseline_label, "nN", result.baseline_force_at_contact_nn),
        (bound_labels[0], unit, bounds[0]),
        (bound_labels[1], unit, bounds[1]),
        ("Pixel X (zero-based)", "", run.pixel[0] as f64),
        ("Pixel Y (zero-based)", "", run.pixel[1] as f64),
    ];
    if !legacy {
        metadata.push(("Reference force", "nN", result.reference_nn));
        if let Some([low, high]) = result.force_bounds_nn {
            metadata.extend([
                ("Force threshold min", "nN", low),
                ("Force threshold max", "nN", high),
            ]);
        }
        if let Some(slope) = result.linearized_slope {
            metadata.push(("Linearized slope", "nN^(2/3)/nm", slope));
        }
        columns.push(points("Corrected separation", "nm", &|row| {
            row.z_nm + row.corrected_force_nn / input.spring_constant_n_per_m
        }));
        columns.push(FloatSeries {
            name: "Original sample index (zero-based)".into(),
            unit: String::new(),
            values: run
                .raw_indices
                .iter()
                .map(|&index| Some(index as f64))
                .collect(),
            uncertainty: None,
            fit: None,
        });
    }
    for (name, unit, value) in metadata {
        columns.push(FloatSeries {
            name: name.into(),
            unit: unit.into(),
            values: vec![Some(value); rows.len()],
            uncertainty: None,
            fit: None,
        });
    }
    let labels = [("Fit method", method), ("Fit window", window.label())];
    super::table_native::materialized_float_series_with_labels(
        (
            "Indentation".into(),
            "nm".into(),
            rows.iter().map(|row| Some(row.indentation_nm)).collect(),
        ),
        columns,
        if legacy { &[] } else { &labels },
        if legacy {
            "plotium.afm.manual-hertz.v1"
        } else {
            "plotium.afm.hertz.v1"
        },
    )
    .map_err(|error| format!("Could not create a Hertz fit table: {error}"))
}
