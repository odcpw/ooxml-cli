use super::{CommandId, CommandSpec, ExecutionSupport};

const COMMAND_GROUP_REASON: &str = "it is a command group, not a leaf mutation command";
pub(super) const GROUP_COMMAND_COUNT: usize = 20;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum PptxCommandId {
    Pptx,
    Animations,
    Charts,
    Comments,
    Extract,
    Fields,
    Layouts,
    Masters,
    Media,
    Notes,
    Place,
    Replace,
    Shapes,
    Slides,
    Tables,
    Template,
    Text,
    Theme,
    Translate,
    XlsxBindings,
}

pub(super) fn command_specs() -> Vec<CommandSpec> {
    let specs = group_command_specs();
    // Owner slices append in live legacy order: diff, slides, template,
    // authoring, animations, masters_layouts, charts, tables,
    // extract_media_notes_comments, replace, render.
    specs
}

fn group_command_specs() -> Vec<CommandSpec> {
    vec![
        group(
            PptxCommandId::Pptx,
            &["pptx"],
            "pptx",
            "Work with PPTX presentations",
        ),
        group(
            PptxCommandId::Animations,
            &["pptx", "animations"],
            "animations",
            "Inspect per-slide animations and embedded media",
        ),
        group(
            PptxCommandId::Charts,
            &["pptx", "charts"],
            "charts",
            "Inspect and mutate slide charts",
        ),
        group(
            PptxCommandId::Comments,
            &["pptx", "comments"],
            "comments",
            "Inspect and mutate PPTX slide comments",
        ),
        group(
            PptxCommandId::Extract,
            &["pptx", "extract"],
            "extract",
            "Extract resources from presentations",
        ),
        group(
            PptxCommandId::Fields,
            &["pptx", "fields"],
            "fields",
            "Inspect and set header/footer/slide-number/date fields",
        ),
        group(
            PptxCommandId::Layouts,
            &["pptx", "layouts"],
            "layouts",
            "Inspect slide layouts",
        ),
        group(
            PptxCommandId::Masters,
            &["pptx", "masters"],
            "masters",
            "Inspect slide masters",
        ),
        group(
            PptxCommandId::Media,
            &["pptx", "media"],
            "media",
            "Embed, replace, and inspect slide audio/video media",
        ),
        group(
            PptxCommandId::Notes,
            &["pptx", "notes"],
            "notes",
            "Set, clear, and show slide speaker notes",
        ),
        group(
            PptxCommandId::Place,
            &["pptx", "place"],
            "place",
            "Place content on presentations",
        ),
        group(
            PptxCommandId::Replace,
            &["pptx", "replace"],
            "replace",
            "Replace content in presentations",
        ),
        group(
            PptxCommandId::Shapes,
            &["pptx", "shapes"],
            "shapes",
            "Inspect and mutate slide shapes",
        ),
        group(
            PptxCommandId::Slides,
            &["pptx", "slides"],
            "slides",
            "Inspect slides",
        ),
        group(
            PptxCommandId::Tables,
            &["pptx", "tables"],
            "tables",
            "Inspect and mutate PPTX tables",
        ),
        group(
            PptxCommandId::Template,
            &["pptx", "template"],
            "template",
            "Work with template manifests and compilation",
        ),
        group(
            PptxCommandId::Text,
            &["pptx", "text"],
            "text",
            "Set slide text run/paragraph styling",
        ),
        group(
            PptxCommandId::Theme,
            &["pptx", "theme"],
            "theme",
            "Inspect and modify presentation themes",
        ),
        group(
            PptxCommandId::Translate,
            &["pptx", "translate"],
            "translate",
            "Export and manage translations",
        ),
        group(
            PptxCommandId::XlsxBindings,
            &["pptx", "xlsx-bindings"],
            "xlsx-bindings",
            "Plan and apply workbook-driven PPTX updates",
        ),
    ]
}

fn group(
    id: PptxCommandId,
    path: &'static [&'static str],
    use_text: &'static str,
    short: &'static str,
) -> CommandSpec {
    CommandSpec {
        id: CommandId::Pptx(id),
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
