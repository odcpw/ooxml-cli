use serde_json::Value;

#[derive(Clone)]
pub(super) struct PptxTableMutationOptions {
    pub(super) out: Option<String>,
    pub(super) backup: Option<String>,
    pub(super) dry_run: bool,
    pub(super) in_place: bool,
    pub(super) no_validate: bool,
}

#[derive(Clone)]
pub(super) struct PptxSlideRef {
    pub(super) part: String,
}

#[derive(Clone, Copy)]
pub(super) struct XmlSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

pub(super) struct DeleteRowMutation {
    pub(super) slide_part: String,
    pub(super) updated_xml: String,
    pub(super) resolved_table_id: u32,
    pub(super) cell_count: usize,
}

pub(super) struct InsertRowMutation {
    pub(super) slide_part: String,
    pub(super) updated_xml: String,
    pub(super) resolved_table_id: u32,
    pub(super) cell_count: usize,
}

pub(super) struct DeleteColMutation {
    pub(super) slide_part: String,
    pub(super) updated_xml: String,
    pub(super) resolved_table_id: u32,
    pub(super) row_count: usize,
}

pub(super) struct InsertColMutation {
    pub(super) slide_part: String,
    pub(super) updated_xml: String,
    pub(super) resolved_table_id: u32,
    pub(super) row_count: usize,
    pub(super) width_emu: i64,
}

pub(super) struct SetCellMutation {
    pub(super) slide_part: String,
    pub(super) updated_xml: String,
    pub(super) resolved_table_id: u32,
    pub(super) previous_text: String,
    pub(super) text: String,
}

pub(super) struct UpdateMatrixMutation {
    pub(super) slide_part: String,
    pub(super) updated_xml: String,
    pub(super) resolved_table_id: u32,
    pub(super) updated_cells: usize,
    pub(super) changed_cells: usize,
}

pub(super) struct SetCellRequest<'a> {
    pub(super) file: &'a str,
    pub(super) slide: u32,
    pub(super) table_id: u32,
    pub(super) target: Option<&'a str>,
    pub(super) row: usize,
    pub(super) col: usize,
    pub(super) text: String,
}

pub(super) struct UpdateFromXlsxSource {
    pub(super) source: Value,
    pub(super) data: Vec<Vec<String>>,
    pub(super) rows: usize,
    pub(super) cols: usize,
    pub(super) range: String,
}
