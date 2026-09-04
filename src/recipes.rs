use serde_json::{Value, json};

use crate::{CliError, CliResult};

pub(crate) const RECIPE_CONTRACT_VERSION: &str = "ooxml-cli.recipe.v1";

#[derive(Clone, Copy)]
pub(crate) struct RecipeInput {
    pub(crate) placeholder: &'static str,
    pub(crate) purpose: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct RecipeStep {
    pub(crate) command: &'static str,
    pub(crate) purpose: &'static str,
    pub(crate) expected_fields: &'static [&'static str],
    pub(crate) proof_command: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct Recipe {
    pub(crate) name: &'static str,
    pub(crate) title: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) families: &'static [&'static str],
    pub(crate) inputs: &'static [RecipeInput],
    pub(crate) steps: &'static [RecipeStep],
}

const fn input(placeholder: &'static str, purpose: &'static str) -> RecipeInput {
    RecipeInput {
        placeholder,
        purpose,
    }
}

const fn step(
    command: &'static str,
    purpose: &'static str,
    expected_fields: &'static [&'static str],
    proof_command: &'static str,
) -> RecipeStep {
    RecipeStep {
        command,
        purpose,
        expected_fields,
        proof_command,
    }
}

const CHECK_PPTX: &str = "ooxml --json check <output.pptx> --openxml-sdk skip --fail-on error";
const CHECK_XLSX: &str = "ooxml --json check <output.xlsx> --openxml-sdk skip --fail-on error";
const CHECK_DOCX: &str = "ooxml --json check <output.docx> --openxml-sdk skip --fail-on error";
const CHECK_XLSM: &str = "ooxml --json check <output.xlsm> --openxml-sdk skip --fail-on error";
const CHECK_FILE: &str = "ooxml --json check <output-file> --openxml-sdk skip --fail-on error";

const DECK_FROM_SCRATCH_INPUTS: &[RecipeInput] = &[input(
    "<output.pptx>",
    "new presentation path; the recipe replaces it only because --force is explicit",
)];
const DECK_FROM_SCRATCH_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json pptx scaffold <output.pptx> --title Recipe --subtitle Generated --force",
        "Create a validated 16:9 presentation with the standard layouts and theme.",
        &["/output", "/validated", "/family", "/layouts"],
        CHECK_PPTX,
    ),
    step(
        "ooxml --json outline <output.pptx> --depth 2",
        "Read the generated slide tree and stable selectors before editing.",
        &["/type", "/summary/slides", "/slides"],
        CHECK_PPTX,
    ),
];

const DECK_FROM_TEMPLATE_INPUTS: &[RecipeInput] = &[
    input(
        "<template.pptx>",
        "source presentation whose master, layouts, and theme are inherited",
    ),
    input("<output.pptx>", "new presentation path"),
];
const DECK_FROM_TEMPLATE_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json pptx scaffold <output.pptx> --template <template.pptx> --title Recipe --force",
        "Create a presentation that inherits the template master, layouts, and theme.",
        &["/output", "/validated", "/template", "/layouts"],
        CHECK_PPTX,
    ),
    step(
        "ooxml --json outline <output.pptx> --depth 2",
        "Confirm the generated slide and layout tree.",
        &["/type", "/summary/slides", "/slides"],
        CHECK_PPTX,
    ),
];

const WORKBOOK_REPORT_INPUTS: &[RecipeInput] = &[
    input(
        "<workbook-spec.json>",
        "XLSX build specification from capabilities --schema xlsx-build",
    ),
    input("<output.xlsx>", "new workbook path"),
];
const WORKBOOK_REPORT_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json xlsx build --spec <workbook-spec.json> --out <output.xlsx> --check --force",
        "Compile and atomically publish the complete workbook report.",
        &["/output", "/validated", "/outline", "/check/summary/errors"],
        CHECK_XLSX,
    ),
    step(
        "ooxml --json xlsx ranges export <output.xlsx> --sheet Sales --range A1:D4 --include-types",
        "Read back the report header and typed data cells.",
        &["/sheet", "/range", "/values", "/types"],
        CHECK_XLSX,
    ),
];

const DOCUMENT_REPORT_INPUTS: &[RecipeInput] = &[
    input(
        "<document-spec.json>",
        "DOCX build specification from capabilities --schema docx-build",
    ),
    input("<output.docx>", "new document path"),
];
const DOCUMENT_REPORT_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json docx build --spec <document-spec.json> --out <output.docx> --check --force",
        "Compile and atomically publish the styled document report.",
        &["/output", "/validated", "/outline", "/check/summary/errors"],
        CHECK_DOCX,
    ),
    step(
        "ooxml --json docx text <output.docx>",
        "Read back blocks, styles, lists, tables, and text.",
        &["/blocks", "/blockHashes", "/documentHash"],
        CHECK_DOCX,
    ),
];

const MACRO_WORKBOOK_INPUTS: &[RecipeInput] = &[
    input("<module.bas>", "trusted VBA standard-module source"),
    input("<base.xlsx>", "temporary non-macro workbook path"),
    input("<output.xlsm>", "new macro-enabled workbook path"),
];
const MACRO_WORKBOOK_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json xlsx scaffold <base.xlsx> --sheet Data --force",
        "Create the validated workbook host.",
        &["/output", "/validated", "/sheet"],
        "ooxml validate --strict <base.xlsx>",
    ),
    step(
        "ooxml --json vba create <base.xlsx> --pure --family xlsx --source <module.bas> --out <output.xlsm>",
        "Build and attach vbaProject.bin with the pure Rust authoring path.",
        &[
            "/output",
            "/authoring/modules",
            "/vba/hasVbaProject",
            "/validateCommand",
        ],
        CHECK_XLSM,
    ),
    step(
        "ooxml --json vba list <output.xlsm>",
        "Confirm that the expected VBA module is discoverable.",
        &[
            "/project/modules",
            "/project/moduleCount",
            "/project/family",
        ],
        CHECK_XLSM,
    ),
];

const FIND_REPLACE_INPUTS: &[RecipeInput] = &[
    input(
        "<input-file>",
        "source PPTX, XLSX, or DOCX package containing the literal text Hello",
    ),
    input(
        "<output-file>",
        "new package path with the same family as the input",
    ),
];
const FIND_REPLACE_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json find Hello <input-file> --replace Replaced --apply --out <output-file>",
        "Find exact text, generate supported mutations, validate, and publish in one call.",
        &["/opsCount", "/validated", "/applied", "/output"],
        CHECK_FILE,
    ),
    step(
        "ooxml --json outline <output-file> --depth 3",
        "Verify the replacement through the family-aware package outline.",
        &["/type", "/summary"],
        CHECK_FILE,
    ),
];

const TRANSLATE_DECK_INPUTS: &[RecipeInput] = &[
    input("<input.pptx>", "source presentation"),
    input(
        "<manifest.json>",
        "translation manifest captured from the export step and edited by a translator",
    ),
    input("<output.pptx>", "new translated presentation path"),
];
const TRANSLATE_DECK_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json pptx translate export <input.pptx> > <manifest.json>",
        "Export stable translation ids, source hashes, and text into a JSON manifest.",
        &["/metadata", "/entries"],
        "ooxml --json check <input.pptx> --openxml-sdk skip --fail-on error",
    ),
    step(
        "ooxml --json pptx translate apply <input.pptx> <manifest.json> --stale error --output <output.pptx>",
        "Apply the reviewed manifest with stale-source protection.",
        &["/entriesProcessed", "/entriesApplied", "/entriesSkipped"],
        CHECK_PPTX,
    ),
];

const PIVOT_REPORT_INPUTS: &[RecipeInput] = &[
    input(
        "<input.xlsx>",
        "source workbook containing table Sales with Region and Revenue columns",
    ),
    input("<output.xlsx>", "new workbook path"),
];
const PIVOT_REPORT_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json xlsx pivots create <input.xlsx> --sheet Data --table Sales --target-sheet Data --anchor D1 --name SalesPivot --rows Region --values Revenue:sum --out <output.xlsx>",
        "Create a PivotTable from a stable table selector and explicit field names.",
        &[
            "/output",
            "/mutationEnvelope/validated",
            "/pivotTableUri",
            "/pivotsListCommand",
        ],
        CHECK_XLSX,
    ),
    step(
        "ooxml --json xlsx pivots list <output.xlsx> --sheet Data",
        "Read back the authored pivot source, location, and fields.",
        &["/pivots", "/validateCommand"],
        CHECK_XLSX,
    ),
];

const BATCH_EDIT_INPUTS: &[RecipeInput] = &[
    input("<input-file>", "source PPTX, XLSX, or DOCX package"),
    input("<ops.json>", "ordered apply operation array"),
    input(
        "<output-file>",
        "new package path with the same family as the input",
    ),
];
const BATCH_EDIT_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json apply <input-file> --ops <ops.json> --out <output-file>",
        "Apply the ordered operation batch through the Serve/MCP mutation seam.",
        &["/output", "/validated", "/opsCount", "/applied"],
        CHECK_FILE,
    ),
    step(
        "ooxml --json outline <output-file> --depth 3",
        "Confirm the batch result using the family-aware package outline.",
        &["/type", "/summary"],
        CHECK_FILE,
    ),
];

const BUILD_FROM_SPEC_INPUTS: &[RecipeInput] = &[
    input("<presentation-spec.json>", "PPTX build specification"),
    input("<workbook-spec.json>", "XLSX build specification"),
    input("<document-spec.json>", "DOCX build specification"),
    input("<output.pptx>", "new presentation path"),
    input("<output.xlsx>", "new workbook path"),
    input("<output.docx>", "new document path"),
];
const BUILD_FROM_SPEC_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json pptx build --spec <presentation-spec.json> --out <output.pptx> --check --force",
        "Build and check the presentation specification.",
        &[
            "/output",
            "/validated",
            "/outline/type",
            "/check/summary/errors",
        ],
        CHECK_PPTX,
    ),
    step(
        "ooxml --json xlsx build --spec <workbook-spec.json> --out <output.xlsx> --check --force",
        "Build and check the workbook specification.",
        &[
            "/output",
            "/validated",
            "/outline/type",
            "/check/summary/errors",
        ],
        CHECK_XLSX,
    ),
    step(
        "ooxml --json docx build --spec <document-spec.json> --out <output.docx> --check --force",
        "Build and check the document specification.",
        &[
            "/output",
            "/validated",
            "/outline/type",
            "/check/summary/errors",
        ],
        CHECK_DOCX,
    ),
];

const BUILD_FROM_MARKDOWN_INPUTS: &[RecipeInput] = &[
    input("<deck.md>", "supported presentation Markdown source"),
    input("<document.md>", "supported document Markdown source"),
    input("<output.pptx>", "new presentation path"),
    input("<output.docx>", "new document path"),
];
const BUILD_FROM_MARKDOWN_STEPS: &[RecipeStep] = &[
    step(
        "ooxml --json pptx build --from-markdown <deck.md> --out <output.pptx> --check --force",
        "Convert Markdown to the PPTX build spec, publish, and check the result.",
        &[
            "/output",
            "/markdown",
            "/validated",
            "/outline/type",
            "/check/summary/errors",
        ],
        CHECK_PPTX,
    ),
    step(
        "ooxml --json docx build --from-markdown <document.md> --out <output.docx> --check --force",
        "Convert Markdown to the DOCX build spec, publish, and check the result.",
        &[
            "/output",
            "/markdown",
            "/validated",
            "/outline/type",
            "/check/summary/errors",
        ],
        CHECK_DOCX,
    ),
];

pub(crate) const RECIPES: &[Recipe] = &[
    Recipe {
        name: "deck-from-scratch",
        title: "Deck from scratch",
        summary: "Create, orient on, and prove a new presentation.",
        families: &["pptx"],
        inputs: DECK_FROM_SCRATCH_INPUTS,
        steps: DECK_FROM_SCRATCH_STEPS,
    },
    Recipe {
        name: "deck-from-template",
        title: "Deck from template",
        summary: "Create a presentation from an existing master, layouts, and theme.",
        families: &["pptx"],
        inputs: DECK_FROM_TEMPLATE_INPUTS,
        steps: DECK_FROM_TEMPLATE_STEPS,
    },
    Recipe {
        name: "workbook-report",
        title: "Workbook report",
        summary: "Build, read back, and prove a structured workbook report.",
        families: &["xlsx"],
        inputs: WORKBOOK_REPORT_INPUTS,
        steps: WORKBOOK_REPORT_STEPS,
    },
    Recipe {
        name: "document-report",
        title: "Document report",
        summary: "Build, read back, and prove a styled document report.",
        families: &["docx"],
        inputs: DOCUMENT_REPORT_INPUTS,
        steps: DOCUMENT_REPORT_STEPS,
    },
    Recipe {
        name: "macro-workbook",
        title: "Macro workbook",
        summary: "Create an XLSM through the pure Rust VBA authoring path.",
        families: &["xlsx", "xlsm", "vba"],
        inputs: MACRO_WORKBOOK_INPUTS,
        steps: MACRO_WORKBOOK_STEPS,
    },
    Recipe {
        name: "find-replace-package",
        title: "Find and replace package text",
        summary: "Find exact text, publish supported replacements, and verify the result.",
        families: &["pptx", "xlsx", "docx"],
        inputs: FIND_REPLACE_INPUTS,
        steps: FIND_REPLACE_STEPS,
    },
    Recipe {
        name: "translate-deck",
        title: "Translate a deck",
        summary: "Export stable translation ids and apply a reviewed manifest safely.",
        families: &["pptx"],
        inputs: TRANSLATE_DECK_INPUTS,
        steps: TRANSLATE_DECK_STEPS,
    },
    Recipe {
        name: "pivot-report",
        title: "Pivot report",
        summary: "Create and read back a PivotTable from a named source table.",
        families: &["xlsx"],
        inputs: PIVOT_REPORT_INPUTS,
        steps: PIVOT_REPORT_STEPS,
    },
    Recipe {
        name: "batch-edit-with-apply",
        title: "Batch edit with apply",
        summary: "Apply an ordered mutation batch atomically and verify its readback.",
        families: &["pptx", "xlsx", "docx"],
        inputs: BATCH_EDIT_INPUTS,
        steps: BATCH_EDIT_STEPS,
    },
    Recipe {
        name: "build-from-spec",
        title: "Build from specifications",
        summary: "Build and prove one package for each supported family from published JSON schemas.",
        families: &["pptx", "xlsx", "docx"],
        inputs: BUILD_FROM_SPEC_INPUTS,
        steps: BUILD_FROM_SPEC_STEPS,
    },
    Recipe {
        name: "build-from-markdown",
        title: "Build from Markdown",
        summary: "Build and prove PPTX and DOCX packages from the supported Markdown profile.",
        families: &["pptx", "docx"],
        inputs: BUILD_FROM_MARKDOWN_INPUTS,
        steps: BUILD_FROM_MARKDOWN_STEPS,
    },
];

pub(crate) fn names() -> Vec<&'static str> {
    RECIPES.iter().map(|recipe| recipe.name).collect()
}

pub(crate) fn find(name: &str) -> CliResult<&'static Recipe> {
    RECIPES
        .iter()
        .find(|recipe| recipe.name == name)
        .ok_or_else(|| {
            CliError::invalid_args(format!(
                "unknown recipe {name:?}; valid recipes: {}",
                names().join(", ")
            ))
        })
}

pub(crate) fn recipe_json(recipe: &Recipe) -> Value {
    json!({
        "contractVersion": RECIPE_CONTRACT_VERSION,
        "name": recipe.name,
        "title": recipe.title,
        "summary": recipe.summary,
        "families": recipe.families,
        "command": format!("ooxml --json robot-docs recipe {}", recipe.name),
        "inputs": recipe.inputs.iter().map(|input| json!({
            "placeholder": input.placeholder,
            "purpose": input.purpose,
        })).collect::<Vec<_>>(),
        "followUps": recipe_follow_ups(recipe.name),
        "typedMcpTools": typed_mcp_tools(recipe.name),
        "steps": recipe.steps.iter().enumerate().map(|(index, step)| json!({
            "index": index + 1,
            "command": step.command,
            "purpose": step.purpose,
            "expectedFields": step.expected_fields,
            "proofCommand": step.proof_command,
        })).collect::<Vec<_>>(),
    })
}

pub(crate) fn recipes_json() -> Value {
    Value::Array(RECIPES.iter().map(recipe_json).collect())
}

pub(crate) fn catalog_json() -> Value {
    json!({
        "tool": "ooxml",
        "version": env!("CARGO_PKG_VERSION"),
        "contractVersion": RECIPE_CONTRACT_VERSION,
        "recipes": recipes_json(),
    })
}

pub(crate) fn recipe_markdown(recipe: &Recipe, heading_level: usize) -> String {
    let heading = "#".repeat(heading_level);
    let mut out = format!(
        "{heading} `{}` — {}\n\n{}\n\n",
        recipe.name, recipe.title, recipe.summary
    );
    out.push_str("Inputs:\n\n");
    for input in recipe.inputs {
        out.push_str(&format!("- `{}` — {}\n", input.placeholder, input.purpose));
    }
    out.push_str("\nTyped MCP tools for the same intent: `");
    out.push_str(&typed_mcp_tools(recipe.name).join("`, `"));
    out.push_str("`.\n");
    out.push_str("\nSteps:\n\n");
    for (index, step) in recipe.steps.iter().enumerate() {
        out.push_str(&format!("{}. {}\n\n", index + 1, step.purpose));
        out.push_str("   ```console\n   ");
        out.push_str(step.command);
        out.push_str("\n   ```\n\n");
        out.push_str("   Expected JSON fields: `");
        out.push_str(&step.expected_fields.join("`, `"));
        out.push_str("`.\n\n");
        out.push_str("   Proof:\n\n   ```console\n   ");
        out.push_str(step.proof_command);
        out.push_str("\n   ```\n\n");
    }
    out.push_str("Follow-ups:\n\n");
    for command in recipe_follow_ups(recipe.name) {
        out.push_str("```console\n");
        out.push_str(command);
        out.push_str("\n```\n\n");
    }
    out
}

pub(crate) fn catalog_markdown() -> String {
    let mut out = String::from(
        "## Runnable recipes\n\nThese sections are generated from `ooxml robot-docs recipes`; run `make docs-recipes` after changing the recipe contract. Replace angle-bracket placeholders with paths you control.\n\n",
    );
    for recipe in RECIPES {
        out.push_str(&recipe_markdown(recipe, 3));
    }
    out
}

pub(crate) fn recipes_for(family: Option<&str>, request: Option<&str>) -> Vec<&'static Recipe> {
    let request = request.unwrap_or_default().to_ascii_lowercase();
    let requested = [
        ("translate", "translate-deck"),
        ("pivot", "pivot-report"),
        ("macro", "macro-workbook"),
        ("vba", "macro-workbook"),
        ("template", "deck-from-template"),
        ("markdown", "build-from-markdown"),
        ("spec", "build-from-spec"),
        ("batch", "batch-edit-with-apply"),
        ("apply", "batch-edit-with-apply"),
        ("replace", "find-replace-package"),
        ("presentation", "deck-from-scratch"),
        ("deck", "deck-from-scratch"),
        ("workbook", "workbook-report"),
        ("excel", "workbook-report"),
        ("document", "document-report"),
        ("word", "document-report"),
    ]
    .into_iter()
    .filter(|(needle, _)| request.contains(needle))
    .map(|(_, name)| name)
    .collect::<Vec<_>>();

    let defaults: &[&str] = match family {
        Some("pptx") => &[
            "deck-from-scratch",
            "translate-deck",
            "find-replace-package",
        ],
        Some("xlsx") => &["workbook-report", "pivot-report", "batch-edit-with-apply"],
        Some("docx") => &[
            "document-report",
            "find-replace-package",
            "build-from-markdown",
        ],
        Some("xlsm") => &["macro-workbook", "workbook-report", "find-replace-package"],
        _ => &[
            "build-from-spec",
            "build-from-markdown",
            "batch-edit-with-apply",
        ],
    };
    requested
        .into_iter()
        .chain(defaults.iter().copied())
        .filter_map(|name| RECIPES.iter().find(|recipe| recipe.name == name))
        .fold(Vec::new(), |mut recipes, recipe| {
            if !recipes
                .iter()
                .any(|existing: &&Recipe| existing.name == recipe.name)
            {
                recipes.push(recipe);
            }
            recipes
        })
}

fn recipe_follow_ups(name: &str) -> &'static [&'static str] {
    match name {
        "deck-from-scratch" | "deck-from-template" | "translate-deck" => &[
            "ooxml --json outline <output.pptx> --depth 3",
            CHECK_PPTX,
            "ooxml --json design-check <output.pptx>",
        ],
        "workbook-report" | "pivot-report" => {
            &["ooxml --json outline <output.xlsx> --depth 3", CHECK_XLSX]
        }
        "document-report" => &["ooxml --json outline <output.docx> --depth 3", CHECK_DOCX],
        "macro-workbook" => &["ooxml --json outline <output.xlsm> --depth 3", CHECK_XLSM],
        "find-replace-package" | "batch-edit-with-apply" => {
            &["ooxml --json outline <output-file> --depth 3", CHECK_FILE]
        }
        "build-from-spec" => &[
            "ooxml --json outline <output.pptx> --depth 3",
            "ooxml --json outline <output.xlsx> --depth 3",
            "ooxml --json outline <output.docx> --depth 3",
            CHECK_PPTX,
            CHECK_XLSX,
            CHECK_DOCX,
            "ooxml --json design-check <output.pptx>",
        ],
        "build-from-markdown" => &[
            "ooxml --json outline <output.pptx> --depth 3",
            "ooxml --json outline <output.docx> --depth 3",
            CHECK_PPTX,
            CHECK_DOCX,
            "ooxml --json design-check <output.pptx>",
        ],
        _ => &[],
    }
}

fn typed_mcp_tools(name: &str) -> &'static [&'static str] {
    match name {
        "deck-from-scratch" | "deck-from-template" => &[
            "build_presentation",
            "outline_package",
            "check_package",
            "render_preview",
        ],
        "workbook-report" => &["build_workbook", "outline_package", "check_package"],
        "document-report" => &["build_document", "outline_package", "check_package"],
        "macro-workbook" => &[
            "edit_package",
            "outline_package",
            "check_package",
            "validate_package",
        ],
        "find-replace-package" => &[
            "find_text",
            "replace_text",
            "outline_package",
            "check_package",
        ],
        "translate-deck" => &["edit_package", "outline_package", "check_package"],
        "pivot-report" | "batch-edit-with-apply" => {
            &["edit_package", "outline_package", "check_package"]
        }
        "build-from-spec" => &[
            "build_presentation",
            "build_workbook",
            "build_document",
            "outline_package",
            "check_package",
        ],
        "build-from-markdown" => &[
            "build_presentation",
            "build_document",
            "outline_package",
            "check_package",
        ],
        _ => &[],
    }
}
