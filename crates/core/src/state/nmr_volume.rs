//! A three-dimensional acquisition owns a reusable two-dimensional display plane.
use super::*;
use plotium_io::nmr_view::NmrSource;
use plotium_processing::nmr_volume::{NmrVolume, VolumePhase};

#[derive(Clone)]
pub struct NmrVolumeState {
    pub source: NmrSource,
    pub peaks: Peak3DSet,
    pub processed: NmrVolume,
    pub fixed_axis: usize,
    pub index: usize,
    pub anchor: [usize; 3],
    pub phases: [VolumePhase; 3],
}

impl Nmr2DDataset {
    /// Build disposable display geometry without installing scientific state.
    pub fn phase_preview_contours(
        &self,
        plane: NmrSource,
        key: ContourGeometryCacheKey,
        style: plotium_figure::ContourStyle,
    ) -> Result<plotium_figure::Figure, String> {
        let mut display = self.clone();
        display.replace_volume_plane(plane)?;
        let Processed2D::Ft(spectrum) = &display.processed else {
            return Err("Phase preview requires a frequency plane".into());
        };
        let grid = std::sync::Arc::new(nmr_scalar_grid(spectrum, spectrum.real()));
        let geometry = super::compute::compute_field::run_build_contour(key, grid)?;
        Ok(super::datasets_2d_figure::apply_contour_geometry(
            (*display.processed_figure).clone(),
            &geometry,
            &style,
        ))
    }
    pub fn load_volume(source: NmrSource) -> Result<Self, String> {
        let phases = [VolumePhase::default(); 3];
        Self::load_volume_state(source, phases, 0, None)
    }

    pub fn load_volume_state(
        source: NmrSource,
        phases: [VolumePhase; 3],
        fixed_axis: usize,
        index: Option<usize>,
    ) -> Result<Self, String> {
        Self::load_volume_state_with_context(
            source,
            phases,
            fixed_axis,
            index,
            &mut nmr::ExecutionContext::default(),
        )
    }

    pub fn load_volume_state_with_context(
        source: NmrSource,
        phases: [VolumePhase; 3],
        fixed_axis: usize,
        index: Option<usize>,
        context: &mut nmr::ExecutionContext<'_>,
    ) -> Result<Self, String> {
        if fixed_axis >= 3 {
            return Err("Select an acquisition axis between 0 and 2".into());
        }
        let processed = NmrVolume::process(&source, phases, Default::default(), context)?;
        let index = index.unwrap_or(processed.shape()[fixed_axis] / 2);
        let plane = processed.plane_source(fixed_axis, index)?;
        let mut dataset = Self::load_with_pipeline(
            plane,
            Some(Params2D {
                layout: plotium_processing::Layout2D::Ft,
                f1: AxisPipeline { steps: Vec::new() },
                f2: AxisPipeline { steps: Vec::new() },
            }),
            Some(false),
            None,
            true,
        )?;
        dataset.acquisition_identity = source.identity();
        let mut anchor = processed.shape().map(|n| n / 2);
        anchor[fixed_axis] = index;
        dataset.volume = Some(NmrVolumeState {
            source,
            peaks: Peak3DSet::default(),
            processed,
            fixed_axis,
            index,
            phases,
            anchor,
        });
        Ok(dataset)
    }

    pub fn select_volume_plane(&mut self, fixed_axis: usize, index: usize) -> Result<(), String> {
        let volume = self
            .volume
            .as_ref()
            .ok_or("Open a three-dimensional spectrum first")?;
        let plane = volume.processed.plane_source(fixed_axis, index)?;
        self.replace_volume_plane(plane)?;
        let volume = self
            .volume
            .as_mut()
            .ok_or("Three-dimensional spectrum disappeared")?;
        volume.fixed_axis = fixed_axis;
        volume.index = index;
        volume.anchor[fixed_axis] = index;
        Ok(())
    }

    pub fn set_volume_phases(&mut self, phases: [VolumePhase; 3]) -> Result<(), String> {
        self.set_volume_phases_with_context(phases, &mut nmr::ExecutionContext::default())
    }

    pub fn set_volume_phases_with_context(
        &mut self,
        phases: [VolumePhase; 3],
        context: &mut nmr::ExecutionContext<'_>,
    ) -> Result<(), String> {
        let volume = self
            .volume
            .as_ref()
            .ok_or("Open a three-dimensional spectrum first")?;
        let processed = NmrVolume::process(&volume.source, phases, Default::default(), context)?;
        let plane = processed.plane_source(volume.fixed_axis, volume.index)?;
        self.replace_volume_plane(plane)?;
        let volume = self
            .volume
            .as_mut()
            .ok_or("Three-dimensional spectrum disappeared")?;
        volume.processed = processed;
        volume.phases = phases;
        volume.refresh_peaks()?;
        Ok(())
    }

    fn replace_volume_plane(&mut self, plane: NmrSource) -> Result<(), String> {
        let result = Self::load_with_pipeline(
            plane,
            Some(Params2D {
                layout: plotium_processing::Layout2D::Ft,
                f1: AxisPipeline { steps: Vec::new() },
                f2: AxisPipeline { steps: Vec::new() },
            }),
            Some(false),
            None,
            true,
        )?;
        self.data = result.data;
        self.native_base = result.native_base;
        self.native_processed = result.native_processed;
        self.base = result.base;
        self.processed = result.processed;
        self.processed_figure = result.processed_figure;
        Ok(())
    }
}
