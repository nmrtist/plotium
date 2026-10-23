use plotium_core::data::{RelPlanV1, SnapshotRead, TableSchema};
use plotium_core::state::{DatasetId, TableCellEditDraft, TableEditDelta};

pub(super) struct TableTransformRequest {
    pub input_datasets: Vec<usize>,
    pub name: String,
    pub plan: RelPlanV1,
}

pub(super) struct TableCatalogEntry {
    pub dataset: usize,
    pub name: String,
    pub read: SnapshotRead,
    pub schema: TableSchema,
}

pub(super) struct TableSheetContext<'a> {
    pub dataset: usize,
    pub commit: &'a mut Option<TableEditDelta>,
    pub draft: &'a mut Option<TableCellEditDraft>,
    pub transform: &'a mut Option<TableTransformRequest>,
    pub refresh: &'a mut Option<(usize, Vec<DatasetId>)>,
    pub catalog: &'a [TableCatalogEntry],
    pub transform_running: bool,
}
