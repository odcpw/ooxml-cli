mod charts;
mod comments;
mod conditional_formatting;
mod data_validations;
mod freeze;
mod links_filters;
mod names;
mod pivots_workbook;
mod ranges_cells;
mod structure;
mod tables;

use super::{CommandId, CommandSpec, ExecutionSupport, FlagSpec};

const COMMAND_GROUP_REASON: &str = "it is a command group, not a leaf mutation command";
pub(super) const GROUP_COMMAND_COUNT: usize = 21;
pub(super) const FRONT_COMMAND_COUNT: usize = GROUP_COMMAND_COUNT + 1;
pub(super) const ROOT_OWNED_COMMAND_COUNT: usize = FRONT_COMMAND_COUNT + 1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum XlsxCommandId {
    Xlsx,
    Cells,
    Charts,
    Cols,
    Colwidths,
    Comments,
    ConditionalFormats,
    DataValidations,
    FiltersSorts,
    Forms,
    Freeze,
    Hyperlinks,
    Names,
    Pivots,
    Ranges,
    Rowheights,
    Rows,
    Sheets,
    Tables,
    Workbook,
    WorkbookMetadata,
    Scaffold,
    FormsEntry,
    SheetsList,
    SheetsShow,
    SheetsAdd,
    SheetsRename,
    SheetsMove,
    SheetsDelete,
    ColwidthsShow,
    ColwidthsSet,
    RowheightsShow,
    RowheightsSet,
    RowsInsert,
    RowsDelete,
    ColsInsert,
    ColsDelete,
    ChartsList,
    ChartsShow,
    ChartsCreate,
    ChartsUpdateSource,
    ChartsSetTitle,
    ChartsSetLegend,
    ChartsSetChartAreaFill,
    ChartsSetPlotAreaFill,
    ChartsSetSeriesStyle,
    ChartsConvertType,
    ChartsCopyStyle,
    ChartsSetAxis,
}

pub(super) fn command_specs() -> Vec<CommandSpec> {
    let mut specs = front_command_specs();
    specs.extend(structure::command_specs());
    specs.extend(charts::command_specs());
    specs.extend(comments::command_specs());
    specs.extend(conditional_formatting::command_specs());
    specs.extend(data_validations::command_specs());
    specs.extend(links_filters::command_specs());
    specs.extend(forms_command_specs());
    specs.extend(names::command_specs());
    specs.extend(tables::command_specs());
    specs.extend(pivots_workbook::command_specs());
    specs.extend(ranges_cells::command_specs());
    specs.extend(freeze::command_specs());
    specs
}

pub(super) fn front_command_specs() -> Vec<CommandSpec> {
    let mut specs = group_command_specs();
    specs.push(spec(
        XlsxCommandId::Scaffold,
        &["xlsx", "scaffold"],
        "scaffold <output.xlsx> (or --out <output.xlsx>)",
        "Create a minimal XLSX workbook from scratch and validate it by default.",
        &["package", "sheet"],
        vec![
            flag(
                "--out",
                "out",
                "string",
                "output workbook path; accepted as an alternative to positional <output.xlsx>",
            ),
            flag("--sheet", "sheet", "string", "initial worksheet name"),
            flag(
                "--force",
                "force",
                "bool",
                "replace an existing output file",
            ),
            flag(
                "--no-validate",
                "noValidate",
                "bool",
                "skip post-write strict validation",
            ),
        ],
        direct("it creates a package and is not an apply/serve mutation op"),
        None,
    ));
    specs
}

fn group_command_specs() -> Vec<CommandSpec> {
    vec![
        group(
            XlsxCommandId::Xlsx,
            &["xlsx"],
            "xlsx",
            "Work with XLSX workbooks",
        ),
        group(
            XlsxCommandId::Cells,
            &["xlsx", "cells"],
            "cells",
            "Read and mutate worksheet cells",
        ),
        group(
            XlsxCommandId::Charts,
            &["xlsx", "charts"],
            "charts",
            "Inspect workbook charts",
        ),
        group(
            XlsxCommandId::Cols,
            &["xlsx", "cols"],
            "cols",
            "Insert and delete worksheet columns",
        ),
        group(
            XlsxCommandId::Colwidths,
            &["xlsx", "colwidths"],
            "colwidths",
            "Inspect and set worksheet column widths",
        ),
        group(
            XlsxCommandId::Comments,
            &["xlsx", "comments"],
            "comments",
            "Inspect and mutate XLSX cell comments (legacy notes)",
        ),
        group(
            XlsxCommandId::ConditionalFormats,
            &["xlsx", "conditional-formats"],
            "conditional-formats",
            "Inspect and mutate worksheet conditional formatting",
        ),
        group(
            XlsxCommandId::DataValidations,
            &["xlsx", "data-validations"],
            "data-validations",
            "Inspect and mutate worksheet data validations",
        ),
        group(
            XlsxCommandId::FiltersSorts,
            &["xlsx", "filters-sorts"],
            "filters-sorts",
            "Auto-filter and sort for table/range workflows",
        ),
        group(
            XlsxCommandId::Forms,
            &["xlsx", "forms"],
            "forms",
            "Create worksheet-based data entry forms with non-ActiveX controls",
        ),
        group(
            XlsxCommandId::Freeze,
            &["xlsx", "freeze"],
            "freeze",
            "Inspect and set worksheet freeze panes",
        ),
        group(
            XlsxCommandId::Hyperlinks,
            &["xlsx", "hyperlinks"],
            "hyperlinks",
            "Inspect and mutate worksheet hyperlinks",
        ),
        group(
            XlsxCommandId::Names,
            &["xlsx", "names"],
            "names",
            "Inspect and mutate workbook defined names",
        ),
        group(
            XlsxCommandId::Pivots,
            &["xlsx", "pivots"],
            "pivots",
            "Inspect workbook PivotTables",
        ),
        group(
            XlsxCommandId::Ranges,
            &["xlsx", "ranges"],
            "ranges",
            "Export and set rectangular worksheet ranges",
        ),
        group(
            XlsxCommandId::Rowheights,
            &["xlsx", "rowheights"],
            "rowheights",
            "Inspect and set worksheet row heights",
        ),
        group(
            XlsxCommandId::Rows,
            &["xlsx", "rows"],
            "rows",
            "Insert and delete worksheet rows",
        ),
        group(
            XlsxCommandId::Sheets,
            &["xlsx", "sheets"],
            "sheets",
            "Inspect and mutate workbook sheets",
        ),
        group(
            XlsxCommandId::Tables,
            &["xlsx", "tables"],
            "tables",
            "Inspect and mutate workbook tables",
        ),
        group(
            XlsxCommandId::Workbook,
            &["xlsx", "workbook"],
            "workbook",
            "Workbook-level operations",
        ),
        group(
            XlsxCommandId::WorkbookMetadata,
            &["xlsx", "workbook", "metadata"],
            "metadata",
            "Inspect and update workbook metadata and calc settings",
        ),
    ]
}

pub(super) fn forms_command_specs() -> Vec<CommandSpec> {
    vec![spec(
        XlsxCommandId::FormsEntry,
        &["xlsx", "forms", "entry"],
        "entry --out <workbook.xlsm> [--field <label>...]",
        "Create a macro-enabled workbook with a non-ActiveX Group Box, Label, worksheet text inputs, and Form Control buttons for submit, clear, and sample-fill macros.",
        &["package", "sheet", "range", "form", "module"],
        vec![
            flag(
                "--out",
                "out",
                "string",
                "macro-enabled output workbook path; must end in .xlsm",
            ),
            flag(
                "--field",
                "field",
                "stringArray",
                "repeatable field label; defaults to Name, Email, Notes",
            ),
            flag(
                "--sheet",
                "sheet",
                "string",
                "worksheet name for the input form; default Form",
            ),
            flag(
                "--data-sheet",
                "dataSheet",
                "string",
                "worksheet name for appended rows; default Entries",
            ),
            flag(
                "--button",
                "button",
                "string",
                "caption for the submit Form Control button; default Submit",
            ),
            flag(
                "--force",
                "force",
                "bool",
                "replace an existing output file",
            ),
            flag(
                "--no-validate",
                "noValidate",
                "bool",
                "skip post-write strict validation",
            ),
        ],
        direct("it creates a package and is not an apply/serve mutation op"),
        None,
    )]
}

fn group(
    id: XlsxCommandId,
    path: &'static [&'static str],
    use_text: &'static str,
    short: &'static str,
) -> CommandSpec {
    CommandSpec {
        id: CommandId::Xlsx(id),
        path,
        use_text,
        short,
        target_object_kinds: &[],
        local_flags: vec![],
        execution: ExecutionSupport::GroupOnly {
            reason: Some(COMMAND_GROUP_REASON),
        },
        flag_constraints: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn spec(
    id: XlsxCommandId,
    path: &'static [&'static str],
    use_text: &'static str,
    short: &'static str,
    target_object_kinds: &'static [&'static str],
    local_flags: Vec<FlagSpec>,
    execution: ExecutionSupport,
    flag_constraints: Option<serde_json::Value>,
) -> CommandSpec {
    CommandSpec {
        id: CommandId::Xlsx(id),
        path,
        use_text,
        short,
        target_object_kinds,
        local_flags,
        execution,
        flag_constraints,
    }
}

fn flag(
    name: &'static str,
    arg_name: &'static str,
    flag_type: &'static str,
    description: &'static str,
) -> FlagSpec {
    FlagSpec {
        name,
        arg_name,
        flag_type,
        description,
    }
}

fn direct(reason: &'static str) -> ExecutionSupport {
    ExecutionSupport::DirectOnly {
        reason: Some(reason),
    }
}
