use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::{CliError, CliResult, command_arg, reject_unknown_flags};

pub(crate) const MUTATION_ENVELOPE_SCHEMA_ID: &str =
    "https://ooxml-cli.dev/schemas/mutation-envelope.schema.json";

const MUTATION_ENVELOPE_SCHEMA_JSON: &str = r##"
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://ooxml-cli.dev/schemas/mutation-envelope.schema.json",
  "title": "OOXML Mutation Envelope",
  "description": "Stable proof and destination metadata added to every successful package mutation.",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "file",
    "family",
    "command",
    "destination",
    "changed",
    "readbackCommand",
    "validateCommand",
    "conformanceCommand",
    "checkCommand",
    "warnings",
    "aliasesApplied",
    "validated"
  ],
  "properties": {
    "file": { "type": "string", "minLength": 1 },
    "family": { "enum": ["docx", "xlsx", "pptx", "vba", "package"] },
    "command": { "type": "string", "pattern": "^ooxml " },
    "destination": { "$ref": "#/$defs/destination" },
    "changed": {
      "type": "array",
      "items": { "$ref": "#/$defs/change" }
    },
    "readbackCommand": { "type": "string", "pattern": "^ooxml " },
    "validateCommand": { "type": "string", "pattern": "^ooxml " },
    "conformanceCommand": { "type": "string", "pattern": "^ooxml " },
    "checkCommand": { "type": "string", "pattern": "^ooxml --json check " },
    "renderCommand": { "type": "string", "pattern": "^ooxml " },
    "layoutCheckCommand": { "type": "string", "pattern": "^ooxml " },
    "warnings": { "type": "array", "items": {} },
    "aliasesApplied": { "type": "array", "items": { "type": "object" } },
    "validated": { "type": "boolean" }
  },
  "allOf": [
    {
      "if": { "properties": { "family": { "enum": ["docx", "xlsx", "pptx"] } } },
      "then": { "required": ["renderCommand"] }
    },
    {
      "if": { "properties": { "family": { "const": "pptx" } } },
      "then": { "required": ["layoutCheckCommand"] }
    }
  ],
  "$defs": {
    "destination": {
      "type": "object",
      "additionalProperties": false,
      "required": ["partUri", "primarySelector", "selectors", "handle", "kind", "summary"],
      "properties": {
        "partUri": { "type": "string", "minLength": 1 },
        "primarySelector": { "type": "string", "minLength": 1 },
        "selectors": {
          "type": "array",
          "minItems": 1,
          "uniqueItems": true,
          "items": { "type": "string", "minLength": 1 }
        },
        "handle": { "type": "string", "minLength": 1 },
        "kind": { "type": "string", "minLength": 1 },
        "summary": { "type": "object", "additionalProperties": true }
      }
    },
    "change": {
      "type": "object",
      "additionalProperties": false,
      "required": ["kind", "selector", "handle"],
      "properties": {
        "kind": { "type": "string", "minLength": 1 },
        "selector": { "type": "string", "minLength": 1 },
        "handle": { "type": "string", "minLength": 1 },
        "beforeHash": { "type": "string", "pattern": "^sha256:[0-9a-f]{64}$" },
        "afterHash": { "type": "string", "pattern": "^sha256:[0-9a-f]{64}$" }
      }
    }
  }
}
"##;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutationEnvelope {
    pub(crate) file: String,
    pub(crate) family: String,
    pub(crate) command: String,
    pub(crate) destination: MutationDestination,
    pub(crate) changed: Vec<MutationChange>,
    pub(crate) readback_command: String,
    pub(crate) validate_command: String,
    pub(crate) conformance_command: String,
    pub(crate) check_command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) render_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) layout_check_command: Option<String>,
    pub(crate) warnings: Vec<Value>,
    pub(crate) aliases_applied: Vec<Value>,
    pub(crate) validated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutationDestination {
    pub(crate) part_uri: String,
    pub(crate) primary_selector: String,
    pub(crate) selectors: Vec<String>,
    pub(crate) handle: String,
    pub(crate) kind: String,
    pub(crate) summary: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutationChange {
    pub(crate) kind: String,
    pub(crate) selector: String,
    pub(crate) handle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) before_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) after_hash: Option<String>,
}

pub(crate) struct MutationEnvelopeInput {
    pub(crate) file: String,
    pub(crate) family: String,
    pub(crate) command: String,
    pub(crate) destination: MutationDestination,
    pub(crate) changed: Vec<MutationChange>,
    pub(crate) readback_command: String,
    pub(crate) warnings: Vec<Value>,
    pub(crate) aliases_applied: Vec<Value>,
    pub(crate) validated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MutationCommandSpec {
    family: &'static str,
    path: &'static [&'static str],
    destination_kind: &'static str,
    default_part_uri: &'static str,
    readback: ReadbackKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadbackKind {
    Blocks,
    Comments,
    Fields,
    Footers,
    Headers,
    Images,
    Tables,
    Outline,
}

macro_rules! docx_spec {
    ([$($segment:literal),+], $kind:literal, $part:literal, $readback:ident) => {
        MutationCommandSpec {
            family: "docx",
            path: &[$($segment),+],
            destination_kind: $kind,
            default_part_uri: $part,
            readback: ReadbackKind::$readback,
        }
    };
}

macro_rules! xlsx_spec {
    ([$($segment:literal),+], $kind:literal, $part:literal) => {
        MutationCommandSpec {
            family: "xlsx",
            path: &[$($segment),+],
            destination_kind: $kind,
            default_part_uri: $part,
            readback: ReadbackKind::Outline,
        }
    };
}

macro_rules! pptx_spec {
    ([$($segment:literal),+], $kind:literal, $part:literal) => {
        MutationCommandSpec {
            family: "pptx",
            path: &[$($segment),+],
            destination_kind: $kind,
            default_part_uri: $part,
            readback: ReadbackKind::Outline,
        }
    };
}

macro_rules! package_spec {
    ($family:literal, [$($segment:literal),+], $kind:literal, $part:literal) => {
        MutationCommandSpec {
            family: $family,
            path: &[$($segment),+],
            destination_kind: $kind,
            default_part_uri: $part,
            readback: ReadbackKind::Outline,
        }
    };
}

// This table is deliberately data rather than command-name inference. It is the
// review surface for the DOCX adoption stage and is mirrored into the public
// CommandSpec destinationKind rows.
const DOCX_MUTATION_COMMANDS: &[MutationCommandSpec] = &[
    docx_spec!(["docx", "scaffold"], "package", "/", Blocks),
    docx_spec!(
        ["docx", "blocks", "replace"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "blocks", "delete"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "blocks", "insert-after"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "breaks", "insert"],
        "section",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "sections", "set"],
        "section",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "paragraphs", "append"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "paragraphs", "insert"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "paragraphs", "set"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "paragraphs", "clear"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "styles", "apply"],
        "styled-object",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "comments", "add"],
        "comment",
        "/word/comments.xml",
        Comments
    ),
    docx_spec!(
        ["docx", "comments", "edit"],
        "comment",
        "/word/comments.xml",
        Comments
    ),
    docx_spec!(
        ["docx", "comments", "remove"],
        "comment",
        "/word/comments.xml",
        Comments
    ),
    docx_spec!(
        ["docx", "fields", "insert"],
        "field",
        "/word/document.xml",
        Fields
    ),
    docx_spec!(
        ["docx", "fields", "set-result"],
        "field",
        "/word/document.xml",
        Fields
    ),
    docx_spec!(
        ["docx", "headers", "set-text"],
        "header",
        "/word/header1.xml",
        Headers
    ),
    docx_spec!(
        ["docx", "footers", "set-text"],
        "footer",
        "/word/footer1.xml",
        Footers
    ),
    docx_spec!(
        ["docx", "images", "replace"],
        "image",
        "/word/document.xml",
        Images
    ),
    docx_spec!(
        ["docx", "images", "insert"],
        "image",
        "/word/document.xml",
        Images
    ),
    docx_spec!(
        ["docx", "replace"],
        "text-match",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "tables", "create"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "set-style"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "set-cell"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "clear-cell"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "insert-row"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "delete-row"],
        "table",
        "/word/document.xml",
        Tables
    ),
];

// XLSX destination kinds are semantic targets, while the part URI is the
// deterministic fallback used when a writer does not already return its part.
const XLSX_MUTATION_COMMANDS: &[MutationCommandSpec] = &[
    xlsx_spec!(["xlsx", "scaffold"], "package", "/"),
    xlsx_spec!(["xlsx", "sheets", "add"], "sheet", "/xl/workbook.xml"),
    xlsx_spec!(["xlsx", "sheets", "rename"], "sheet", "/xl/workbook.xml"),
    xlsx_spec!(["xlsx", "sheets", "move"], "sheet", "/xl/workbook.xml"),
    xlsx_spec!(["xlsx", "sheets", "delete"], "sheet", "/xl/workbook.xml"),
    xlsx_spec!(
        ["xlsx", "sheets", "set-tab-color"],
        "sheet",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "sheets", "set-print"],
        "sheet",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "colwidths", "set"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "colwidths", "autofit"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "rowheights", "set"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "rows", "insert"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "rows", "delete"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "cols", "insert"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "cols", "delete"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "create"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "update-source"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "set-title"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "set-legend"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "set-chart-area-fill"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "set-plot-area-fill"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "set-series-style"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "convert-type"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "copy-style"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "charts", "set-axis"],
        "chart",
        "/xl/charts/chart1.xml"
    ),
    xlsx_spec!(["xlsx", "comments", "add"], "comment", "/xl/comments1.xml"),
    xlsx_spec!(
        ["xlsx", "comments", "update"],
        "comment",
        "/xl/comments1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "comments", "remove"],
        "comment",
        "/xl/comments1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "conditional-formats", "add"],
        "conditional-format",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "conditional-formats", "delete"],
        "conditional-format",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "conditional-formats", "reorder"],
        "conditional-format",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "data-validations", "create"],
        "data-validation",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "data-validations", "update"],
        "data-validation",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "data-validations", "delete"],
        "data-validation",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "hyperlinks", "add"],
        "hyperlink",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "hyperlinks", "update"],
        "hyperlink",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "hyperlinks", "delete"],
        "hyperlink",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "filters-sorts", "set-autofilter"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "filters-sorts", "clear-autofilter"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "filters-sorts", "add-column-filter"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "filters-sorts", "clear-column-filter"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "filters-sorts", "set-sort"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "filters-sorts", "clear-sort"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(["xlsx", "names", "add"], "name", "/xl/workbook.xml"),
    xlsx_spec!(["xlsx", "names", "update"], "name", "/xl/workbook.xml"),
    xlsx_spec!(["xlsx", "names", "rename"], "name", "/xl/workbook.xml"),
    xlsx_spec!(["xlsx", "names", "delete"], "name", "/xl/workbook.xml"),
    xlsx_spec!(
        ["xlsx", "tables", "create"],
        "table",
        "/xl/tables/table1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "tables", "append-rows"],
        "table",
        "/xl/tables/table1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "tables", "append-records"],
        "table",
        "/xl/tables/table1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "tables", "set-column-format"],
        "table",
        "/xl/tables/table1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "pivots", "create"],
        "pivot",
        "/xl/pivotTables/pivotTable1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "workbook", "metadata", "update"],
        "package",
        "/xl/workbook.xml"
    ),
    xlsx_spec!(
        ["xlsx", "ranges", "set"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "ranges", "set-format"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "ranges", "set-style"],
        "range",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "cells", "set"],
        "cell",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "cells", "clear"],
        "cell",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "cells", "set-batch"],
        "cell",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "freeze", "set"],
        "sheet",
        "/xl/worksheets/sheet1.xml"
    ),
    xlsx_spec!(
        ["xlsx", "freeze", "clear"],
        "sheet",
        "/xl/worksheets/sheet1.xml"
    ),
];

const PPTX_MUTATION_COMMANDS: &[MutationCommandSpec] = &[
    pptx_spec!(
        ["pptx", "slides", "compose"],
        "slide",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "slides", "delete"],
        "slide",
        "/ppt/presentation.xml"
    ),
    pptx_spec!(["pptx", "slides", "move"], "slide", "/ppt/presentation.xml"),
    pptx_spec!(
        ["pptx", "slides", "reorder"],
        "slide",
        "/ppt/presentation.xml"
    ),
    pptx_spec!(
        ["pptx", "slides", "import-slide"],
        "slide",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "slides", "merge"],
        "slide",
        "/ppt/presentation.xml"
    ),
    pptx_spec!(["pptx", "clone-slide"], "slide", "/ppt/slides/slide1.xml"),
    pptx_spec!(
        ["pptx", "new-slide-from-layout"],
        "slide",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(["pptx", "template", "compile"], "template", "/"),
    pptx_spec!(
        ["pptx", "xlsx-bindings", "apply"],
        "slide",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(["pptx", "scaffold"], "package", "/"),
    pptx_spec!(["pptx", "add-textbox"], "shape", "/ppt/slides/slide1.xml"),
    pptx_spec!(["pptx", "text", "set"], "shape", "/ppt/slides/slide1.xml"),
    pptx_spec!(["pptx", "fields", "set"], "field", "/ppt/slides/slide1.xml"),
    pptx_spec!(
        ["pptx", "theme", "update"],
        "style",
        "/ppt/theme/theme1.xml"
    ),
    pptx_spec!(
        ["pptx", "place", "image"],
        "image",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "place", "table"],
        "table",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "place", "table-from-xlsx"],
        "table",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "shapes", "set-bounds"],
        "shape",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "shapes", "delete"],
        "shape",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "animations", "add"],
        "animation",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "animations", "remove"],
        "animation",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "animations", "reorder"],
        "animation",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "animations", "prune-stale"],
        "animation",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "masters", "add-placeholder"],
        "master",
        "/ppt/slideMasters/slideMaster1.xml"
    ),
    pptx_spec!(
        ["pptx", "masters", "import"],
        "master",
        "/ppt/slideMasters/slideMaster1.xml"
    ),
    pptx_spec!(
        ["pptx", "layouts", "clone"],
        "layout",
        "/ppt/slideLayouts/slideLayout1.xml"
    ),
    pptx_spec!(
        ["pptx", "layouts", "import"],
        "layout",
        "/ppt/slideLayouts/slideLayout1.xml"
    ),
    pptx_spec!(
        ["pptx", "layouts", "rename"],
        "layout",
        "/ppt/slideLayouts/slideLayout1.xml"
    ),
    pptx_spec!(
        ["pptx", "layouts", "set-bounds"],
        "layout",
        "/ppt/slideLayouts/slideLayout1.xml"
    ),
    pptx_spec!(
        ["pptx", "layouts", "delete-shape"],
        "layout",
        "/ppt/slideLayouts/slideLayout1.xml"
    ),
    pptx_spec!(
        ["pptx", "layouts", "add-placeholder"],
        "layout",
        "/ppt/slideLayouts/slideLayout1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "create"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "update-data"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "set-title"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "set-legend"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "set-chart-area-fill"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "set-plot-area-fill"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "set-series-style"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "set-axis"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "convert-type"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "charts", "copy-style"],
        "chart",
        "/ppt/charts/chart1.xml"
    ),
    pptx_spec!(
        ["pptx", "tables", "set-cell"],
        "table",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "tables", "delete-row"],
        "table",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "tables", "insert-row"],
        "table",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "tables", "delete-col"],
        "table",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "tables", "insert-col"],
        "table",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "tables", "update-from-xlsx"],
        "table",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(["pptx", "media", "add"], "media", "/ppt/media"),
    pptx_spec!(["pptx", "media", "replace"], "media", "/ppt/media"),
    pptx_spec!(
        ["pptx", "notes", "set"],
        "slide",
        "/ppt/notesSlides/notesSlide1.xml"
    ),
    pptx_spec!(
        ["pptx", "notes", "clear"],
        "slide",
        "/ppt/notesSlides/notesSlide1.xml"
    ),
    pptx_spec!(
        ["pptx", "comments", "add"],
        "comment",
        "/ppt/comments/comment1.xml"
    ),
    pptx_spec!(
        ["pptx", "comments", "edit"],
        "comment",
        "/ppt/comments/comment1.xml"
    ),
    pptx_spec!(
        ["pptx", "comments", "remove"],
        "comment",
        "/ppt/comments/comment1.xml"
    ),
    pptx_spec!(
        ["pptx", "replace", "text"],
        "shape",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "replace", "text-occurrences"],
        "shape",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "replace", "text-from-xlsx"],
        "shape",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "replace", "text-map-from-xlsx"],
        "shape",
        "/ppt/slides/slide1.xml"
    ),
    pptx_spec!(
        ["pptx", "replace", "images"],
        "image",
        "/ppt/slides/slide1.xml"
    ),
];

const PACKAGE_MUTATION_COMMANDS: &[MutationCommandSpec] = &[
    package_spec!("package", ["apply"], "batch", "/"),
    package_spec!("xlsx", ["convert", "xlsm-to-xlsx"], "package", "/"),
    package_spec!("package", ["find"], "text-match", "/"),
    package_spec!("package", ["repair", "normalize"], "package", "/"),
    package_spec!("package", ["template", "apply"], "package", "/"),
];

pub(crate) fn is_mutation_command_path(path: &[&str]) -> bool {
    DOCX_MUTATION_COMMANDS
        .iter()
        .chain(XLSX_MUTATION_COMMANDS)
        .chain(PPTX_MUTATION_COMMANDS)
        .chain(PACKAGE_MUTATION_COMMANDS)
        .any(|spec| spec.path == path)
}

pub(crate) fn attach_cli_mutation_envelope(
    args: &[String],
    aliases_applied: Vec<Value>,
    response: &mut Value,
) -> CliResult<()> {
    let Some(spec) = mutation_spec_for_args(args) else {
        return Ok(());
    };
    let file = mutation_destination_file(args, spec, response)?;
    let destination = mutation_destination(spec, response, args);
    let selector = destination.primary_selector.clone();
    let handle = destination.handle.clone();
    let before_hash = direct_response_hash(response, &["beforeHash", "previousHash"]);
    let after_hash = if is_removal(spec) {
        direct_response_hash(response, &["afterHash"])
    } else {
        response_hash(response, &["afterHash", "contentHash"])
    };
    let warnings = response_warnings(response);
    let validated = response
        .get("validated")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| !args.iter().any(|arg| arg == "--no-validate"));
    MutationEnvelope::from_input(MutationEnvelopeInput {
        file: file.clone(),
        family: family_for_file(spec.family, &file),
        command: command_for_args(args),
        destination,
        changed: vec![MutationChange {
            kind: spec.destination_kind.to_string(),
            selector,
            handle,
            before_hash,
            after_hash,
        }],
        readback_command: response_readback_command(spec, response)
            .or_else(|| inferred_readback_command(spec, args, &file))
            .unwrap_or_else(|| readback_command(spec.readback, &file, response)),
        warnings,
        aliases_applied,
        validated,
    })
    .attach_to(response)
}

fn mutation_spec_for_args(args: &[String]) -> Option<&'static MutationCommandSpec> {
    let spec = DOCX_MUTATION_COMMANDS
        .iter()
        .chain(XLSX_MUTATION_COMMANDS)
        .chain(PPTX_MUTATION_COMMANDS)
        .chain(PACKAGE_MUTATION_COMMANDS)
        .filter(|spec| {
            let conditional_mutation =
                spec.path != ["find"] || args.iter().any(|arg| arg == "--apply");
            conditional_mutation
                && args.len() >= spec.path.len()
                && args
                    .iter()
                    .zip(spec.path)
                    .all(|(actual, expected)| actual == expected)
        })
        .max_by_key(|spec| spec.path.len());
    debug_assert!(spec.is_none_or(|spec| is_mutation_command_path(spec.path)));
    spec
}

fn family_for_file(default_family: &str, file: &str) -> String {
    let extension = file
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("docx" | "docm") => "docx".to_string(),
        Some("xlsx" | "xlsm") => "xlsx".to_string(),
        Some("pptx" | "pptm") => "pptx".to_string(),
        _ => default_family.to_string(),
    }
}

fn mutation_destination_file(
    args: &[String],
    spec: &MutationCommandSpec,
    response: &Value,
) -> CliResult<String> {
    let file = response
        .get("output")
        .and_then(nonempty_string)
        .or_else(|| flag_value(args, "--out"))
        .or_else(|| {
            if spec.path.last().copied() == Some("scaffold") {
                args.get(spec.path.len())
                    .and_then(|value| (!value.starts_with('-')).then(|| value.to_string()))
            } else {
                None
            }
        })
        .or_else(|| response.get("file").and_then(nonempty_string))
        .or_else(|| args.get(spec.path.len()).cloned())
        .filter(|value| !value.trim().is_empty());
    file.ok_or_else(|| {
        CliError::unexpected(format!(
            "{} succeeded without an addressable destination file",
            spec.path.join(" ")
        ))
    })
}

fn nonempty_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter().enumerate().find_map(|(index, arg)| {
        if arg == name {
            args.get(index + 1).cloned()
        } else {
            arg.strip_prefix(&format!("{name}=")).map(str::to_string)
        }
    })
}

fn mutation_destination(
    spec: &MutationCommandSpec,
    response: &Value,
    args: &[String],
) -> MutationDestination {
    let addressable = first_addressable_response(response);
    let part_uri = response
        .get("destination")
        .and_then(|value| value.get("partUri"))
        .and_then(nonempty_string)
        .or_else(|| response.get("partUri").and_then(nonempty_string))
        .or_else(|| addressable.and_then(|value| value.get("partUri").and_then(nonempty_string)))
        .or_else(|| destination_part_selector(response))
        .unwrap_or_else(|| spec.default_part_uri.to_string());
    let primary_selector = response
        .get("destination")
        .and_then(|value| value.get("primarySelector"))
        .and_then(nonempty_string)
        .or_else(|| response.get("selector").and_then(nonempty_string))
        .or_else(|| {
            addressable.and_then(|value| value.get("primarySelector").and_then(nonempty_string))
        })
        .unwrap_or_else(|| selector_from_response(spec, response, args));
    let handle = response
        .get("destination")
        .and_then(|value| value.get("handle"))
        .and_then(nonempty_string)
        .or_else(|| response.get("handle").and_then(nonempty_string))
        .or_else(|| addressable.and_then(|value| value.get("handle").and_then(nonempty_string)))
        .unwrap_or_else(|| {
            format!(
                "H:{}/{}:{}",
                spec.family,
                spec.destination_kind,
                primary_selector.replace(':', "/")
            )
        });
    let mut selectors = vec![primary_selector.clone()];
    let direct_candidates = [
        response.get("selector").and_then(nonempty_string),
        response.get("blockId").and_then(nonempty_string),
        Some(handle.clone()),
    ];
    let nested_candidates = response
        .get("destination")
        .and_then(Value::as_object)
        .map(|destination| {
            ["selectors", "sheetSelectors"]
                .into_iter()
                .filter_map(|key| destination.get(key).and_then(Value::as_array))
                .flatten()
                .filter_map(nonempty_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let addressable_candidates = addressable
        .and_then(|value| value.get("selectors"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(nonempty_string)
        .collect::<Vec<_>>();
    for candidate in direct_candidates
        .into_iter()
        .flatten()
        .chain(nested_candidates)
        .chain(addressable_candidates)
    {
        if !selectors.contains(&candidate) {
            selectors.push(candidate);
        }
    }
    MutationDestination {
        part_uri,
        primary_selector,
        selectors,
        handle,
        kind: spec.destination_kind.to_string(),
        summary: response_summary(response),
    }
}

fn selector_from_response(spec: &MutationCommandSpec, response: &Value, args: &[String]) -> String {
    if matches!(spec.destination_kind, "package" | "template" | "style") {
        return "package".to_string();
    }
    if spec.path == ["pptx", "slides", "merge"]
        && let (Some(total), Some(merged)) = (
            response.get("totalSlideCount").and_then(Value::as_u64),
            response.get("mergedSlideCount").and_then(Value::as_u64),
        )
    {
        return format!("slide:{}", total.saturating_sub(merged) + 1);
    }
    if spec.path == ["pptx", "slides", "move"]
        && let Some(position) = response.get("toPosition").and_then(selector_value)
    {
        return format!("slide:{position}");
    }
    if spec.path == ["pptx", "slides", "reorder"]
        && let Some(first) = args
            .get(spec.path.len() + 1)
            .and_then(|order| order.split(',').next())
    {
        return format!("slide:{first}");
    }
    if spec.destination_kind == "slide" {
        if let Some(slide) = response
            .get("newSlideNumber")
            .and_then(selector_value)
            .or_else(|| nested_destination_value(response, "number"))
            .or_else(|| flag_value(args, "--slide"))
        {
            return format!("slide:{slide}");
        }
        return "slide:1".to_string();
    }
    if spec.destination_kind == "field" {
        return "slide:1".to_string();
    }
    if spec.destination_kind == "master"
        && let Some(master) = flag_value(args, "--master")
    {
        return format!("master:{master}");
    }
    if spec.destination_kind == "layout"
        && let Some(layout) = response
            .get("newLayout")
            .and_then(selector_value)
            .or_else(|| response.get("newName").and_then(selector_value))
            .or_else(|| flag_value(args, "--name"))
            .or_else(|| flag_value(args, "--layout"))
    {
        return format!("layout:{layout}");
    }
    if matches!(spec.destination_kind, "shape" | "image")
        && let Some(target) =
            flag_value(args, "--target").or_else(|| flag_value(args, "--for-shape"))
    {
        return target;
    }
    if spec.destination_kind == "cell"
        && let Some(cell) = response
            .get("ref")
            .and_then(selector_value)
            .or_else(|| nested_destination_value(response, "range"))
    {
        return format!("cell:{cell}");
    }
    if spec.destination_kind == "range"
        && let Some(range) = response
            .get("range")
            .and_then(selector_value)
            .or_else(|| response.get("ref").and_then(selector_value))
            .or_else(|| nested_destination_value(response, "range"))
    {
        return format!("range:{range}");
    }
    if spec.destination_kind == "chart"
        && let Some(chart) = flag_value(args, "--chart")
    {
        return chart;
    }
    if spec.destination_kind == "name"
        && let Some(name) = flag_value(args, "--new-name").or_else(|| flag_value(args, "--name"))
    {
        return format!("name:{name}");
    }
    for (key, prefix) in [
        ("commentId", "comment"),
        ("table", "table"),
        ("fieldIndex", "field"),
        ("section", "section"),
        ("slideId", "slide"),
        ("slide", "slide"),
        ("shapeId", "shape"),
        ("shape", "shape"),
        ("chartId", "chart"),
        ("chart", "chart"),
        ("sheetId", "sheet"),
        ("sheet", "sheet"),
        ("range", "range"),
        ("cell", "cell"),
        ("name", "name"),
        ("layout", "layout"),
        ("master", "master"),
    ] {
        if let Some(value) = response.get(key).and_then(selector_value) {
            return format!("{prefix}:{value}");
        }
    }
    if let Some(index) = response
        .get("blockIndex")
        .or_else(|| response.get("index"))
        .and_then(Value::as_u64)
    {
        let prefix = match spec.destination_kind {
            "image" => "image",
            "chart" => "chart",
            "comment" => "comment",
            "table" => "table",
            _ => "block",
        };
        return format!("{prefix}:{index}");
    }
    match spec.destination_kind {
        "package" => "package".to_string(),
        "header" => "header:1".to_string(),
        "footer" => "footer:1".to_string(),
        other => format!("{other}:document"),
    }
}

fn first_addressable_response(response: &Value) -> Option<&Value> {
    if let Some(destination) =
        first_nested_envelope(response).and_then(|value| value.get("destination"))
    {
        return Some(destination);
    }
    let item = first_response_item(response)?;
    Some(item.get("destination").unwrap_or(item))
}

fn first_nested_envelope(response: &Value) -> Option<&Value> {
    response
        .get("applied")?
        .as_array()?
        .first()?
        .get("mutationEnvelope")
}

fn first_response_item(response: &Value) -> Option<&Value> {
    ["matches", "replacements"]
        .iter()
        .find_map(|key| response.get(*key)?.as_array()?.first())
}

fn nested_destination_value(response: &Value, key: &str) -> Option<String> {
    response
        .get("destination")?
        .get(key)
        .and_then(selector_value)
}

fn destination_part_selector(response: &Value) -> Option<String> {
    response
        .get("destination")?
        .get("sheetSelectors")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find_map(|selector| selector.strip_prefix("part:").map(str::to_string))
}

fn selector_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|number| number.to_string()))
        .or_else(|| value.as_u64().map(|number| number.to_string()))
}

fn response_summary(response: &Value) -> Map<String, Value> {
    let mut summary = Map::new();
    let Some(object) = response.as_object() else {
        return summary;
    };
    for (key, value) in object {
        if matches!(
            key.as_str(),
            "aliasesApplied"
                | "conformanceCommand"
                | "destination"
                | "mutationEnvelope"
                | "readbackCommand"
                | "validateCommand"
                | "warnings"
        ) {
            continue;
        }
        if value.is_null() || value.is_boolean() || value.is_number() || value.is_string() {
            summary.insert(key.clone(), value.clone());
        }
    }
    summary
}

fn response_hash(response: &Value, keys: &[&str]) -> Option<String> {
    direct_response_hash(response, keys).or_else(|| {
        let index = response
            .get("blockIndex")
            .or_else(|| response.get("index"))
            .and_then(Value::as_u64)?;
        response
            .get("blockHashes")?
            .as_array()?
            .iter()
            .find(|block| block.get("index").and_then(Value::as_u64) == Some(index))?
            .get("contentHash")?
            .as_str()
            .filter(|hash| is_sha256(hash))
            .map(str::to_string)
    })
}

fn direct_response_hash(response: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        response
            .get(*key)
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .map(str::to_string)
    })
}

fn is_removal(spec: &MutationCommandSpec) -> bool {
    matches!(
        spec.path.last().copied(),
        Some("delete" | "delete-row" | "delete-col" | "remove")
    )
}

fn is_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn response_warnings(response: &Value) -> Vec<Value> {
    let mut warnings = response
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(warning) = response.get("warning") {
        warnings.push(warning.clone());
    }
    warnings
}

fn command_for_args(args: &[String]) -> String {
    std::iter::once("ooxml".to_string())
        .chain(std::iter::once("--json".to_string()))
        .chain(args.iter().map(|arg| command_arg(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn response_readback_command(spec: &MutationCommandSpec, response: &Value) -> Option<String> {
    let candidates: &[&str] = match spec.destination_kind {
        "cell" => &[
            "readbackCommand",
            "cellsExtractCommand",
            "rangesExportCommand",
        ],
        "range" => &[
            "readbackCommand",
            "rangesExportCommand",
            "colwidthsShowCommand",
            "rowheightsShowCommand",
            "showCommand",
            "sheetShowCommand",
        ],
        "chart" => &[
            "readbackCommand",
            "chartShowCommand",
            "chartsListCommand",
            "showCommand",
        ],
        "name" => &[
            "readbackCommand",
            "nameShowCommand",
            "namesListCommand",
            "showCommand",
        ],
        "slide" | "shape" | "image" | "table" => {
            &["readbackCommand", "slideReadbackCommand", "listCommand"]
        }
        _ => &["readbackCommand", "listCommand", "sheetShowCommand"],
    };
    candidates
        .iter()
        .find_map(|key| response.get(*key).and_then(nonempty_string))
        .or_else(|| {
            first_nested_envelope(response)
                .and_then(|value| value.get("readbackCommand"))
                .and_then(nonempty_string)
        })
        .or_else(|| {
            first_response_item(response)
                .and_then(|value| value.get("readbackCommand"))
                .and_then(nonempty_string)
        })
}

fn inferred_readback_command(
    spec: &MutationCommandSpec,
    args: &[String],
    file: &str,
) -> Option<String> {
    if spec.path == ["xlsx", "colwidths", "autofit"] {
        let sheet = flag_value(args, "--sheet")?;
        // Autofit without an explicit span publishes the normalized range in the
        // command response and therefore does not need this fallback.
        let range = flag_value(args, "--range")?;
        return Some(format!(
            "ooxml --json xlsx colwidths show {} --sheet {} --range {}",
            command_arg(file),
            command_arg(&sheet),
            command_arg(&range)
        ));
    }
    None
}

fn readback_command(kind: ReadbackKind, file: &str, response: &Value) -> String {
    let file = command_arg(file);
    match kind {
        ReadbackKind::Blocks => format!("ooxml --json docx blocks {file}"),
        ReadbackKind::Comments => format!("ooxml --json docx comments list {file}"),
        ReadbackKind::Fields => format!("ooxml --json docx fields list {file}"),
        ReadbackKind::Footers => format!("ooxml --json docx footers list {file}"),
        ReadbackKind::Headers => format!("ooxml --json docx headers list {file}"),
        ReadbackKind::Images => format!("ooxml --json docx images list {file}"),
        ReadbackKind::Tables => response
            .get("table")
            .and_then(Value::as_u64)
            .map(|table| format!("ooxml --json docx tables show {file} --table {table}"))
            .unwrap_or_else(|| format!("ooxml --json docx tables show {file}")),
        ReadbackKind::Outline => format!("ooxml --json outline {file} --depth 3"),
    }
}

impl MutationEnvelope {
    pub(crate) fn from_input(input: MutationEnvelopeInput) -> Self {
        let file_arg = command_arg(&input.file);
        let render_command = matches!(input.family.as_str(), "docx" | "xlsx" | "pptx").then(|| {
            let render_dir_arg = command_arg(&format!("{}.render", input.file));
            format!("ooxml --json render {file_arg} --out {render_dir_arg}")
        });
        let layout_check_command = (input.family == "pptx")
            .then(|| format!("ooxml --json pptx validate-layout {file_arg}"));
        Self {
            file: input.file,
            family: input.family,
            command: input.command,
            destination: input.destination,
            changed: input.changed,
            readback_command: input.readback_command,
            validate_command: format!("ooxml --json validate --strict {file_arg}"),
            conformance_command: format!("ooxml --json conformance check {file_arg}"),
            check_command: format!("ooxml --json check {file_arg}"),
            render_command,
            layout_check_command,
            warnings: input.warnings,
            aliases_applied: input.aliases_applied,
            validated: input.validated,
        }
    }

    pub(crate) fn attach_to(self, response: &mut Value) -> CliResult<()> {
        let object = response.as_object_mut().ok_or_else(|| {
            CliError::unexpected("mutation result must be a JSON object before envelope attachment")
        })?;
        if object.contains_key("mutationEnvelope") {
            return Err(CliError::unexpected(
                "mutation result already contains mutationEnvelope",
            ));
        }
        object.insert(
            "mutationEnvelope".to_string(),
            serde_json::to_value(self).expect("serialize mutation envelope"),
        );
        Ok(())
    }
}

pub(crate) fn mutation_envelope_schema() -> CliResult<Value> {
    let schema: Value = serde_json::from_str(MUTATION_ENVELOPE_SCHEMA_JSON).map_err(|err| {
        CliError::unexpected(format!(
            "embedded mutation envelope schema is invalid: {err}"
        ))
    })?;
    if schema.get("$id").and_then(Value::as_str) != Some(MUTATION_ENVELOPE_SCHEMA_ID) {
        return Err(CliError::unexpected(
            "embedded mutation envelope schema has an unexpected $id",
        ));
    }
    Ok(schema)
}

pub(crate) fn schema_command(args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(args, &[], &[])?;
    Ok(json!({
        "schema": "mutation-envelope",
        "document": mutation_envelope_schema()?,
    }))
}

#[cfg(test)]
#[path = "mutation_envelope/proof_matrix_tests.rs"]
mod proof_matrix_tests;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn sample_envelope(family: &str) -> MutationEnvelope {
        MutationEnvelope::from_input(MutationEnvelopeInput {
            file: format!("out.{family}"),
            family: family.to_string(),
            command: format!("ooxml {family} sample mutate"),
            destination: MutationDestination {
                part_uri: "/word/document.xml".to_string(),
                primary_selector: "block:1".to_string(),
                selectors: vec!["block:1".to_string(), "paraId:00112233".to_string()],
                handle: "H:docx/main/para:id:00112233".to_string(),
                kind: "paragraph".to_string(),
                summary: Map::from_iter([("text".to_string(), json!("Hello"))]),
            },
            changed: vec![MutationChange {
                kind: "paragraph".to_string(),
                selector: "block:1".to_string(),
                handle: "H:docx/main/para:id:00112233".to_string(),
                before_hash: None,
                after_hash: Some(format!("sha256:{}", "a".repeat(64))),
            }],
            readback_command: format!("ooxml --json {family} sample show out.{family}"),
            warnings: Vec::new(),
            aliases_applied: Vec::new(),
            validated: true,
        })
    }

    #[test]
    fn schema_pins_required_fields_and_family_specific_proof_commands() {
        let schema = mutation_envelope_schema().expect("schema");
        assert_eq!(schema["$id"], MUTATION_ENVELOPE_SCHEMA_ID);
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().expect("required array");
        for field in [
            "file",
            "family",
            "command",
            "destination",
            "changed",
            "readbackCommand",
            "validateCommand",
            "conformanceCommand",
            "checkCommand",
            "warnings",
            "aliasesApplied",
            "validated",
        ] {
            assert!(required.iter().any(|value| value == field), "{field}");
        }
        assert_eq!(
            schema["allOf"][1]["then"]["required"][0],
            "layoutCheckCommand"
        );
    }

    #[test]
    fn attachment_preserves_legacy_keys_and_changed_boolean() {
        let mut response = json!({
            "file": "legacy.docx",
            "changed": true,
            "commandSpecificCount": 3,
        });
        sample_envelope("docx")
            .attach_to(&mut response)
            .expect("attach");
        assert_eq!(response["changed"], true);
        assert_eq!(response["commandSpecificCount"], 3);
        assert!(response["mutationEnvelope"]["changed"].is_array());
        assert_eq!(
            response["mutationEnvelope"]["checkCommand"],
            "ooxml --json check out.docx"
        );
        assert!(response["mutationEnvelope"]["renderCommand"].is_string());
        assert!(
            response["mutationEnvelope"]
                .get("layoutCheckCommand")
                .is_none()
        );
    }

    #[test]
    fn pptx_envelope_has_layout_and_render_commands() {
        let value = serde_json::to_value(sample_envelope("pptx")).expect("serialize");
        assert_eq!(
            value["layoutCheckCommand"],
            "ooxml --json pptx validate-layout out.pptx"
        );
        assert_eq!(
            value["renderCommand"],
            "ooxml --json render out.pptx --out out.pptx.render"
        );
        assert_eq!(value["aliasesApplied"], json!([]));
        assert_eq!(value["validated"], true);
    }

    #[test]
    fn docx_adoption_table_has_27_unique_mutating_leaf_commands() {
        let paths = DOCX_MUTATION_COMMANDS
            .iter()
            .map(|spec| spec.path.join(" "))
            .collect::<BTreeSet<_>>();
        assert_eq!(DOCX_MUTATION_COMMANDS.len(), 27);
        assert_eq!(paths.len(), DOCX_MUTATION_COMMANDS.len());
        assert!(paths.contains("docx scaffold"));
        assert!(paths.contains("docx images insert"));
        assert!(paths.contains("docx tables delete-row"));
        assert!(!paths.contains("docx blocks"));
        assert!(!paths.contains("docx styles list"));
    }

    #[test]
    fn xlsx_and_pptx_adoption_tables_each_have_60_unique_commands() {
        let paths = XLSX_MUTATION_COMMANDS
            .iter()
            .chain(PPTX_MUTATION_COMMANDS)
            .map(|spec| spec.path.join(" "))
            .collect::<BTreeSet<_>>();
        assert_eq!(XLSX_MUTATION_COMMANDS.len(), 60);
        assert_eq!(PPTX_MUTATION_COMMANDS.len(), 60);
        assert_eq!(paths.len(), 120);
        assert!(paths.contains("xlsx pivots create"));
        assert!(paths.contains("pptx slides compose"));
        assert!(paths.contains("pptx media replace"));
        assert!(!paths.contains("xlsx ranges get"));
        assert!(!paths.contains("pptx charts list"));
    }

    #[test]
    fn mutation_inventory_has_152_unique_commands() {
        let paths = DOCX_MUTATION_COMMANDS
            .iter()
            .chain(XLSX_MUTATION_COMMANDS)
            .chain(PPTX_MUTATION_COMMANDS)
            .chain(PACKAGE_MUTATION_COMMANDS)
            .map(|spec| spec.path.join(" "))
            .collect::<BTreeSet<_>>();
        assert_eq!(PACKAGE_MUTATION_COMMANDS.len(), 5);
        assert_eq!(paths.len(), 152);

        let read_only_find = vec!["find".to_string(), "seed.docx".to_string()];
        assert!(mutation_spec_for_args(&read_only_find).is_none());
        let mutating_find = vec![
            "find".to_string(),
            "seed.docx".to_string(),
            "--apply".to_string(),
        ];
        assert_eq!(
            mutation_spec_for_args(&mutating_find).map(|spec| spec.destination_kind),
            Some("text-match")
        );
    }
}
