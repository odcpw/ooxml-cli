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
#[path = "../../tests/support/inspect_probe_cases.rs"]
mod inspect_probe_cases;

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::command_manifest::{manifest_serve_inspect_ids, manifest_serve_inspect_prose_ids};
    use inspect_probe_cases::inspect_probe_cases;

    #[test]
    fn manifest_namespace_and_real_id_probes_are_exact_and_bidirectional() {
        let probes = inspect_probe_cases(|canonical| {
            resolve_serve_inspect_command(canonical)
                .expect("shared canonical inspect probe must resolve")
        });
        let manifest_ids = manifest_serve_inspect_ids()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let namespace_ids = SERVE_INSPECT_SPECS
            .iter()
            .map(|spec| spec.id)
            .collect::<BTreeSet<_>>();
        let probe_ids = probes
            .iter()
            .map(|probe| probe.key)
            .collect::<BTreeSet<_>>();
        let namespace_canonicals = SERVE_INSPECT_SPECS
            .iter()
            .map(|spec| spec.canonical)
            .collect::<BTreeSet<_>>();
        let probe_canonicals = probes
            .iter()
            .map(|probe| probe.canonical)
            .collect::<BTreeSet<_>>();
        let namespace_labels = SERVE_INSPECT_SPECS
            .iter()
            .flat_map(|spec| std::iter::once(spec.canonical).chain(spec.aliases.iter().copied()))
            .collect::<BTreeSet<_>>();
        let probe_labels = probes
            .iter()
            .flat_map(|probe| std::iter::once(probe.canonical).chain(probe.aliases.iter().copied()))
            .collect::<BTreeSet<_>>();

        assert_eq!(SERVE_INSPECT_SPECS.len(), 42);
        assert_eq!(probes.len(), 42);
        assert_eq!(manifest_ids.len(), 42);
        assert_eq!(namespace_ids.len(), 42);
        assert_eq!(probe_ids.len(), 42);
        assert_eq!(manifest_ids, namespace_ids);
        assert_eq!(namespace_ids, probe_ids);
        assert_eq!(namespace_canonicals, probe_canonicals);
        assert_eq!(namespace_labels.len(), 48);
        assert_eq!(namespace_labels, probe_labels);

        let mut namespace_families = BTreeMap::new();
        let mut probe_families = BTreeMap::new();
        let mut manifest_families = BTreeMap::new();
        for family in ["xlsx", "docx", "pptx"] {
            namespace_families.insert(
                family,
                SERVE_INSPECT_SPECS
                    .iter()
                    .filter(|spec| spec.canonical.starts_with(family))
                    .map(|spec| spec.id)
                    .collect::<BTreeSet<_>>(),
            );
            probe_families.insert(
                family,
                probes
                    .iter()
                    .filter(|probe| probe.family == family)
                    .map(|probe| probe.key)
                    .collect::<BTreeSet<_>>(),
            );
            manifest_families.insert(
                family,
                manifest_ids
                    .iter()
                    .copied()
                    .filter(|id| {
                        matches!(
                            (family, id),
                            ("xlsx", CommandId::Xlsx(_))
                                | ("docx", CommandId::Docx(_))
                                | ("pptx", CommandId::Pptx(_))
                        )
                    })
                    .collect::<BTreeSet<_>>(),
            );
        }
        for (family, count) in [("xlsx", 17), ("docx", 12), ("pptx", 13)] {
            assert_eq!(
                namespace_families[family], probe_families[family],
                "exact {family} ID set"
            );
            assert_eq!(
                manifest_families[family], namespace_families[family],
                "manifest {family} ID set"
            );
            assert_eq!(namespace_families[family].len(), count);
        }

        for probe in &probes {
            assert!(!probe.fixture.is_empty());
            assert!(probe.args.is_object());
            assert_eq!(
                probe
                    .direct_argv
                    .iter()
                    .filter(|arg| **arg == "{file}")
                    .count(),
                1
            );
            assert_eq!(probe.direct_argv.first().copied(), Some(probe.family));
            assert_eq!(
                resolve_serve_inspect_command(probe.canonical),
                Some(probe.key)
            );
            for alias in probe.aliases {
                assert_eq!(resolve_serve_inspect_command(alias), Some(probe.key));
            }
        }

        let prose_ids = manifest_serve_inspect_prose_ids()
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(prose_ids.len(), 23);
        assert!(prose_ids.is_subset(&manifest_ids));
        for (family, expected) in [("xlsx", 14), ("docx", 1), ("pptx", 8)] {
            assert_eq!(
                prose_ids.intersection(&namespace_families[family]).count(),
                expected
            );
        }

        assert_eq!(resolve_serve_inspect_command("xlsx not-real"), None);
    }
}
