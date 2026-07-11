use crate::command_manifest::{CommandId, DocxCommandId, PptxCommandId, XlsxCommandId};

struct ServeInspectSpec {
    id: CommandId,
    canonical: &'static str,
    aliases: &'static [&'static str],
}

const CONDITIONAL_FORMAT_LIST_ALIASES: &[&str] = &[
    "xlsx conditional-formatting list",
    "xlsx conditional-format list",
    "xlsx cf list",
];
const CONDITIONAL_FORMAT_SHOW_ALIASES: &[&str] = &[
    "xlsx conditional-formatting show",
    "xlsx conditional-format show",
    "xlsx cf show",
];

const SERVE_INSPECT_SPECS: &[ServeInspectSpec] = &[
    spec(
        CommandId::Xlsx(XlsxCommandId::RangesExport),
        "xlsx ranges export",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::CellsExtract),
        "xlsx cells extract",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::CommentsList),
        "xlsx comments list",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::ConditionalFormatsList),
        "xlsx conditional-formats list",
        CONDITIONAL_FORMAT_LIST_ALIASES,
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::ConditionalFormatsShow),
        "xlsx conditional-formats show",
        CONDITIONAL_FORMAT_SHOW_ALIASES,
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::SheetsList),
        "xlsx sheets list",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::SheetsShow),
        "xlsx sheets show",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::FreezeShow),
        "xlsx freeze show",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::HyperlinksList),
        "xlsx hyperlinks list",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::HyperlinksShow),
        "xlsx hyperlinks show",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::FiltersSortsShow),
        "xlsx filters-sorts show",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::NamesList),
        "xlsx names list",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::NamesShow),
        "xlsx names show",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::TablesList),
        "xlsx tables list",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::TablesShow),
        "xlsx tables show",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::TablesExport),
        "xlsx tables export",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::WorkbookMetadataInspect),
        "xlsx workbook metadata inspect",
        &[],
    ),
    spec(CommandId::Docx(DocxCommandId::Text), "docx text", &[]),
    spec(
        CommandId::Docx(DocxCommandId::FieldsList),
        "docx fields list",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::HeadersList),
        "docx headers list",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::FootersList),
        "docx footers list",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::HeadersShow),
        "docx headers show",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::FootersShow),
        "docx footers show",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::ImagesList),
        "docx images list",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::CommentsList),
        "docx comments list",
        &[],
    ),
    spec(CommandId::Docx(DocxCommandId::Blocks), "docx blocks", &[]),
    spec(
        CommandId::Docx(DocxCommandId::StylesList),
        "docx styles list",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::StylesShow),
        "docx styles show",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::TablesShow),
        "docx tables show",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::SlidesList),
        "pptx slides list",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::SlidesSelectors),
        "pptx slides selectors",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::SlidesShow),
        "pptx slides show",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::ExtractText),
        "pptx extract text",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::ExtractNotes),
        "pptx extract notes",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::NotesShow),
        "pptx notes show",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::CommentsList),
        "pptx comments list",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::MastersList),
        "pptx masters list",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::MastersShow),
        "pptx masters show",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::LayoutsList),
        "pptx layouts list",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::LayoutsShow),
        "pptx layouts show",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::TablesShow),
        "pptx tables show",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::ShapesShow),
        "pptx shapes show",
        &[],
    ),
];

const fn spec(
    id: CommandId,
    canonical: &'static str,
    aliases: &'static [&'static str],
) -> ServeInspectSpec {
    ServeInspectSpec {
        id,
        canonical,
        aliases,
    }
}

pub(super) fn resolve_serve_inspect_command(command: &str) -> Option<CommandId> {
    SERVE_INSPECT_SPECS
        .iter()
        .find(|spec| spec.canonical == command || spec.aliases.contains(&command))
        .map(|spec| spec.id)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn inspect_namespace_is_exact_unique_and_fully_resolvable() {
        const EXPECTED_LABELS: &[&str] = &[
            "xlsx ranges export",
            "xlsx cells extract",
            "xlsx comments list",
            "xlsx conditional-formats list",
            "xlsx conditional-formatting list",
            "xlsx conditional-format list",
            "xlsx cf list",
            "xlsx conditional-formats show",
            "xlsx conditional-formatting show",
            "xlsx conditional-format show",
            "xlsx cf show",
            "xlsx sheets list",
            "xlsx sheets show",
            "xlsx freeze show",
            "xlsx hyperlinks list",
            "xlsx hyperlinks show",
            "xlsx filters-sorts show",
            "xlsx names list",
            "xlsx names show",
            "xlsx tables list",
            "xlsx tables show",
            "xlsx tables export",
            "xlsx workbook metadata inspect",
            "docx text",
            "docx fields list",
            "docx headers list",
            "docx footers list",
            "docx headers show",
            "docx footers show",
            "docx images list",
            "docx comments list",
            "docx blocks",
            "docx styles list",
            "docx styles show",
            "docx tables show",
            "pptx slides list",
            "pptx slides selectors",
            "pptx slides show",
            "pptx extract text",
            "pptx extract notes",
            "pptx notes show",
            "pptx comments list",
            "pptx masters list",
            "pptx masters show",
            "pptx layouts list",
            "pptx layouts show",
            "pptx tables show",
            "pptx shapes show",
        ];
        let ids = SERVE_INSPECT_SPECS
            .iter()
            .map(|spec| spec.id)
            .collect::<BTreeSet<_>>();
        let canonicals = SERVE_INSPECT_SPECS
            .iter()
            .map(|spec| spec.canonical)
            .collect::<BTreeSet<_>>();
        let labels = SERVE_INSPECT_SPECS
            .iter()
            .flat_map(|spec| std::iter::once(spec.canonical).chain(spec.aliases.iter().copied()))
            .collect::<BTreeSet<_>>();

        assert_eq!(SERVE_INSPECT_SPECS.len(), 42);
        assert_eq!(ids.len(), 42);
        assert_eq!(canonicals.len(), 42);
        assert_eq!(labels.len(), 48);
        assert_eq!(
            labels,
            EXPECTED_LABELS.iter().copied().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            SERVE_INSPECT_SPECS
                .iter()
                .map(|spec| spec.aliases.len())
                .sum::<usize>(),
            6
        );
        for (family, count) in [("xlsx", 17), ("docx", 12), ("pptx", 13)] {
            assert_eq!(
                SERVE_INSPECT_SPECS
                    .iter()
                    .filter(|spec| spec.canonical.starts_with(family))
                    .count(),
                count
            );
        }
        for spec in SERVE_INSPECT_SPECS {
            assert_eq!(resolve_serve_inspect_command(spec.canonical), Some(spec.id));
            for alias in spec.aliases {
                assert_eq!(resolve_serve_inspect_command(alias), Some(spec.id));
            }
        }
        assert_eq!(resolve_serve_inspect_command("xlsx not-real"), None);
    }
}
