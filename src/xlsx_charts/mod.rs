use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, RangeBounds, RelationshipEntry, WorkbookSheet, XlsxRangeExportOptions,
    add_relationship_to_xml, add_selector, allocate_relationship_id, check_range_max_cells,
    col_name, command_arg, copy_zip_with_part_override, copy_zip_with_part_overrides,
    decode_xml_text, ensure_content_type_override, local_name, parse_cell_ref, parse_range,
    relationship_entries, relationship_entries_from_xml, relationship_target_from_source_to_target,
    relationships_part_for, resolve_relationship_target, resolve_sheet, select_xlsx_table,
    selector_candidates, validate, validate_xlsx_mutation_output_flags, workbook_sheets,
    xlsx_range_export_with_options, xlsx_ranges_set_temp_path, xlsx_sheet_selectors, xlsx_tables,
    xml_attr_escape, xml_escape, zip_entry_names, zip_text,
};

mod commands;
mod create;
mod model;
mod options;
mod read;
mod style;
mod support;
mod update_source;

// Preserve the old crate-facing xlsx_charts surface while the implementation lives in focused seams.
pub(crate) use commands::{
    xlsx_charts_convert_type, xlsx_charts_copy_style, xlsx_charts_create, xlsx_charts_list,
    xlsx_charts_set_axis, xlsx_charts_set_chart_area_fill, xlsx_charts_set_legend,
    xlsx_charts_set_plot_area_fill, xlsx_charts_set_series_style, xlsx_charts_set_title,
    xlsx_charts_show, xlsx_charts_update_source,
};
pub(crate) use model::TemplateChartStylePatch;
pub(crate) use options::{
    XlsxChartConvertTypeOptions, XlsxChartCopyStyleOptions, XlsxChartCreateOptions,
    XlsxChartFillTarget, XlsxChartSetAxisOptions, XlsxChartSetFillOptions,
    XlsxChartSetLegendOptions, XlsxChartSetSeriesStyleOptions, XlsxChartSetTitleOptions,
    XlsxChartUpdateSourceOptions,
};
pub(crate) use style::apply_template_chart_series_style_xml;

pub(in crate::xlsx_charts) use create::*;
pub(in crate::xlsx_charts) use model::*;
pub(in crate::xlsx_charts) use read::*;
pub(in crate::xlsx_charts) use style::*;
pub(in crate::xlsx_charts) use support::*;
pub(in crate::xlsx_charts) use update_source::*;
