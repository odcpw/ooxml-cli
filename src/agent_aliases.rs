use serde_json::{Value, json};

use crate::{CliError, CliResult, parse_cell_ref};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FlagAliasTransform {
    Rename,
    FreezeAt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandAliasSpec {
    alias_path: &'static [&'static str],
    canonical_path: &'static [&'static str],
}

const COMMAND_ALIASES: &[CommandAliasSpec] = &[CommandAliasSpec {
    alias_path: &["pptx", "slides", "add"],
    canonical_path: &["pptx", "new-slide-from-layout"],
}];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FlagAliasSpec {
    path: &'static [&'static str],
    alias: &'static str,
    canonical_flags: &'static [&'static str],
    transform: FlagAliasTransform,
}

macro_rules! rename_alias {
    ($path:expr, $alias:literal, $canonical:literal) => {
        FlagAliasSpec {
            path: $path,
            alias: $alias,
            canonical_flags: &[$canonical],
            transform: FlagAliasTransform::Rename,
        }
    };
}

const FLAG_ALIASES: &[FlagAliasSpec] = &[
    rename_alias!(
        &["pptx", "slides", "import-slide"],
        "--after",
        "--insert-after"
    ),
    rename_alias!(&["pptx", "clone-slide"], "--after", "--insert-after"),
    rename_alias!(
        &["pptx", "new-slide-from-layout"],
        "--after",
        "--insert-after"
    ),
    rename_alias!(
        &["docx", "paragraphs", "insert"],
        "--after",
        "--insert-after"
    ),
    rename_alias!(
        &["docx", "paragraphs", "insert"],
        "--after-block",
        "--insert-after"
    ),
    rename_alias!(&["docx", "images", "insert"], "--after-block", "--after"),
    rename_alias!(&["docx", "images", "insert"], "--insert-after", "--after"),
    rename_alias!(&["docx", "paragraphs", "set"], "--block", "--index"),
    rename_alias!(&["docx", "paragraphs", "clear"], "--block", "--index"),
    rename_alias!(&["docx", "styles", "apply"], "--block", "--index"),
    rename_alias!(&["docx", "blocks"], "--index", "--block"),
    rename_alias!(&["docx", "blocks", "replace"], "--index", "--block"),
    rename_alias!(&["docx", "blocks", "delete"], "--index", "--block"),
    rename_alias!(&["docx", "blocks", "insert-after"], "--index", "--block"),
    rename_alias!(&["pptx", "place", "table"], "--values", "--data"),
    rename_alias!(&["pptx", "place", "table"], "--values-file", "--data"),
    rename_alias!(&["pptx", "place", "table"], "--data-format", "--format"),
    rename_alias!(&["pptx", "media", "add"], "--image", "--file"),
    rename_alias!(&["pptx", "media", "replace"], "--image", "--file"),
    rename_alias!(&["docx", "images", "insert"], "--image", "--file"),
    rename_alias!(&["xlsx", "colwidths", "show"], "--col", "--range"),
    rename_alias!(&["xlsx", "colwidths", "show"], "--cols", "--range"),
    rename_alias!(&["xlsx", "colwidths", "set"], "--col", "--range"),
    rename_alias!(&["xlsx", "colwidths", "set"], "--cols", "--range"),
    rename_alias!(&["xlsx", "cells", "extract"], "--cell", "--range"),
    rename_alias!(&["xlsx", "cells", "clear"], "--cell", "--ref"),
    rename_alias!(&["xlsx", "charts", "create"], "--values", "--range"),
    FlagAliasSpec {
        path: &["xlsx", "freeze", "set"],
        alias: "--at",
        canonical_flags: &["--rows", "--cols"],
        transform: FlagAliasTransform::FreezeAt,
    },
    FlagAliasSpec {
        path: &["xlsx", "freeze", "set"],
        alias: "--cell",
        canonical_flags: &["--rows", "--cols"],
        transform: FlagAliasTransform::FreezeAt,
    },
];

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppliedFlagAlias {
    pub(crate) alias: &'static str,
    pub(crate) canonical_flags: &'static [&'static str],
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppliedCommandAlias {
    pub(crate) alias: String,
    pub(crate) canonical_command: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub(crate) enum AppliedAlias {
    Flag(AppliedFlagAlias),
    Command(AppliedCommandAlias),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct NormalizedFlagArgs {
    pub(crate) args: Vec<String>,
    pub(crate) applied: Vec<AppliedAlias>,
}

pub(crate) fn command_aliases_for(command_path: &[&str]) -> Vec<String> {
    let mut aliases = COMMAND_ALIASES
        .iter()
        .filter(|spec| spec.canonical_path == command_path)
        .map(|spec| format!("ooxml {}", spec.alias_path.join(" ")))
        .collect::<Vec<_>>();
    aliases.sort();
    aliases.dedup();
    aliases
}

pub(crate) fn command_alias_registry_json() -> Value {
    json!(
        COMMAND_ALIASES
            .iter()
            .map(|spec| json!({
                "alias": format!("ooxml {}", spec.alias_path.join(" ")),
                "canonicalCommand": format!("ooxml {}", spec.canonical_path.join(" ")),
            }))
            .collect::<Vec<_>>()
    )
}

pub(crate) fn canonicalize_command_alias_path(path: &[String]) -> Vec<String> {
    COMMAND_ALIASES
        .iter()
        .find(|spec| {
            spec.alias_path.len() == path.len()
                && spec
                    .alias_path
                    .iter()
                    .copied()
                    .eq(path.iter().map(String::as_str))
        })
        .map(|spec| {
            spec.canonical_path
                .iter()
                .map(|part| (*part).to_string())
                .collect()
        })
        .unwrap_or_else(|| path.to_vec())
}

fn normalize_command_alias(args: &[String]) -> (Vec<String>, Option<AppliedCommandAlias>) {
    let Some(spec) = COMMAND_ALIASES.iter().find(|spec| {
        args.get(..spec.alias_path.len()).is_some_and(|actual| {
            actual
                .iter()
                .map(String::as_str)
                .eq(spec.alias_path.iter().copied())
        })
    }) else {
        return (args.to_vec(), None);
    };
    let normalized = spec
        .canonical_path
        .iter()
        .map(|part| (*part).to_string())
        .chain(args.iter().skip(spec.alias_path.len()).cloned())
        .collect();
    (
        normalized,
        Some(AppliedCommandAlias {
            alias: format!("ooxml {}", spec.alias_path.join(" ")),
            canonical_command: format!("ooxml {}", spec.canonical_path.join(" ")),
        }),
    )
}

pub(crate) fn flag_aliases_for(command_path: &[&str], canonical_flag: &str) -> Vec<&'static str> {
    let mut aliases = FLAG_ALIASES
        .iter()
        .filter(|spec| spec.path == command_path && spec.canonical_flags.contains(&canonical_flag))
        .map(|spec| spec.alias)
        .collect::<Vec<_>>();
    aliases.sort_unstable();
    aliases.dedup();
    aliases
}

pub(crate) fn flag_alias_registry_json() -> Value {
    json!(
        FLAG_ALIASES
            .iter()
            .map(|spec| json!({
                "path": format!("ooxml {}", spec.path.join(" ")),
                "alias": spec.alias,
                "canonicalFlags": spec.canonical_flags,
            }))
            .collect::<Vec<_>>()
    )
}

fn flag_name(arg: &str) -> &str {
    arg.split_once('=').map_or(arg, |(name, _)| name)
}

fn has_any_flag(args: &[String], flags: &[&str]) -> bool {
    let local_value_flags = crate::command_manifest::local_value_flag_names_for_argv(args);
    let mut index = 0;
    while index < args.len() {
        let name = flag_name(&args[index]);
        if flags.contains(&name) {
            return true;
        }
        index += if local_value_flags.contains(&name) && !args[index].contains('=') {
            2
        } else {
            1
        };
    }
    false
}

fn matching_aliases(args: &[String]) -> Vec<&'static FlagAliasSpec> {
    let path_matches = |spec: &&FlagAliasSpec| {
        args.get(..spec.path.len()).is_some_and(|actual| {
            actual
                .iter()
                .map(String::as_str)
                .eq(spec.path.iter().copied())
        })
    };
    let longest = FLAG_ALIASES
        .iter()
        .filter(path_matches)
        .map(|spec| spec.path.len())
        .max();
    FLAG_ALIASES
        .iter()
        .filter(path_matches)
        .filter(|spec| Some(spec.path.len()) == longest)
        .collect()
}

pub(crate) fn normalize_flag_aliases(args: &[String]) -> CliResult<NormalizedFlagArgs> {
    let (mut normalized, command_alias) = normalize_command_alias(args);
    let aliases = matching_aliases(&normalized);
    if aliases.is_empty() {
        return Ok(NormalizedFlagArgs {
            args: normalized,
            applied: command_alias
                .into_iter()
                .map(AppliedAlias::Command)
                .collect(),
        });
    }

    let local_value_flags = crate::command_manifest::local_value_flag_names_for_argv(&normalized);
    let mut applied = command_alias
        .into_iter()
        .map(AppliedAlias::Command)
        .collect::<Vec<_>>();
    let mut index = aliases[0].path.len();
    while index < normalized.len() {
        let name = flag_name(&normalized[index]);
        let Some(spec) = aliases.iter().find(|spec| spec.alias == name).copied() else {
            index += if local_value_flags.contains(&name) && !normalized[index].contains('=') {
                2
            } else {
                1
            };
            continue;
        };
        if has_any_flag(&normalized, spec.canonical_flags) {
            return Err(CliError::invalid_args(format!(
                "alias {} cannot be combined with canonical {}",
                spec.alias,
                spec.canonical_flags.join(" and ")
            )));
        }
        match spec.transform {
            FlagAliasTransform::Rename => {
                let replacement = normalized[index]
                    .split_once('=')
                    .map(|(_, value)| format!("{}={value}", spec.canonical_flags[0]))
                    .unwrap_or_else(|| spec.canonical_flags[0].to_string());
                normalized[index] = replacement;
                index += 1;
            }
            FlagAliasTransform::FreezeAt => {
                let (value, replaced) = if let Some((_, value)) = normalized[index].split_once('=')
                {
                    (value.to_string(), 1)
                } else {
                    let Some(value) = normalized.get(index + 1) else {
                        return Err(CliError::invalid_args(format!(
                            "{} requires a cell reference such as A2",
                            spec.alias
                        )));
                    };
                    (value.clone(), 2)
                };
                let (column, row) = parse_cell_ref(&value).map_err(|err| {
                    CliError::invalid_args(format!("invalid {}: {}", spec.alias, err.message))
                })?;
                normalized.splice(
                    index..index + replaced,
                    [
                        "--rows".to_string(),
                        row.saturating_sub(1).to_string(),
                        "--cols".to_string(),
                        column.saturating_sub(1).to_string(),
                    ],
                );
                index += 4;
            }
        }
        applied.push(AppliedAlias::Flag(AppliedFlagAlias {
            alias: spec.alias,
            canonical_flags: spec.canonical_flags,
        }));
    }
    Ok(NormalizedFlagArgs {
        args: normalized,
        applied,
    })
}

pub(crate) struct InvalidArgsIntentHint {
    pub(crate) did_you_mean: &'static [&'static str],
    pub(crate) hint: &'static str,
}

const INVALID_ARGS_INTENT_HINTS: &[(&[&str], &str, InvalidArgsIntentHint)] = &[];

pub(crate) fn invalid_args_intent_hint(
    command_path: &[&str],
    wrong_flag: &str,
) -> Option<&'static InvalidArgsIntentHint> {
    INVALID_ARGS_INTENT_HINTS
        .iter()
        .find(|(path, flag, _)| *path == command_path && *flag == wrong_flag)
        .map(|(_, _, hint)| hint)
}

pub(crate) const CAPABILITY_OBJECT_KINDS: &[&str] = &[
    "package",
    "template",
    "slide",
    "shape",
    "animation",
    "master",
    "layout",
    "placeholder",
    "sheet",
    "range",
    "form",
    "conditional-format",
    "data-validation",
    "cell",
    "hyperlink",
    "table",
    "pivot",
    "name",
    "block",
    "paragraph",
    "section",
    "style",
    "theme",
    "comment",
    "chart",
    "field",
    "header",
    "footer",
    "image",
    "media",
    "module",
];

pub(crate) const CAPABILITY_COMMAND_FAMILY_FILTERS: &[&str] = &[
    "pptx",
    "xlsx",
    "docx",
    "vba",
    "apply",
    "convert",
    "diff",
    "repair",
    "template",
    "capabilities",
    "help",
    "doctor",
    "find",
    "robot-docs",
    "agent",
    "agent-triage",
    "completion",
    "conformance",
    "serve",
    "mcp",
    "version",
];

pub(crate) const CAPABILITY_FILTER_ALIASES: &[(&str, &str)] = &[
    ("slides", "slide"),
    ("shapes", "shape"),
    ("animations", "animation"),
    ("masters", "master"),
    ("layouts", "layout"),
    ("placeholders", "placeholder"),
    ("sheets", "sheet"),
    ("ranges", "range"),
    ("forms", "form"),
    ("conditional-formats", "conditional-format"),
    ("conditional-formatting", "conditional-format"),
    ("cf", "conditional-format"),
    ("data-validations", "data-validation"),
    ("dv", "data-validation"),
    ("cells", "cell"),
    ("hyperlinks", "hyperlink"),
    ("tables", "table"),
    ("pivots", "pivot"),
    ("names", "name"),
    ("blocks", "block"),
    ("paragraphs", "paragraph"),
    ("styles", "style"),
    ("themes", "theme"),
    ("comments", "comment"),
    ("charts", "chart"),
    ("fields", "field"),
    ("headers", "header"),
    ("footers", "footer"),
    ("images", "image"),
    ("modules", "module"),
    ("macros", "module"),
    ("macro", "module"),
];

pub(crate) const CONDITIONAL_FORMAT_TOPIC_ALIASES: &[&str] =
    &["conditional-format", "conditional-formatting", "cf"];
pub(crate) const DATA_VALIDATION_TOPIC_ALIASES: &[&str] = &["data-validation", "dv"];

pub(crate) fn normalize_capability_filter(raw: &str) -> String {
    let mut filter = raw.trim().to_ascii_lowercase().replace('_', "-");
    if let Some(stripped) = filter.strip_prefix("ooxml ") {
        filter = stripped.to_string();
    }
    CAPABILITY_FILTER_ALIASES
        .iter()
        .find_map(|(alias, canonical)| (*alias == filter).then_some((*canonical).to_string()))
        .unwrap_or(filter)
}

pub(crate) fn capability_filter_aliases_json() -> Value {
    json!(
        CAPABILITY_FILTER_ALIASES
            .iter()
            .map(|(alias, canonical)| json!({
                "alias": alias,
                "canonical": canonical
            }))
            .collect::<Vec<_>>()
    )
}

pub(crate) fn capability_filter_alias_strings() -> Vec<String> {
    CAPABILITY_FILTER_ALIASES
        .iter()
        .map(|(alias, canonical)| format!("{alias} -> {canonical}"))
        .collect()
}

pub(crate) fn capability_filter_suggestions(filter: &str) -> Vec<String> {
    let mut suggestions = Vec::new();
    for candidate in capability_known_filters() {
        if candidate.contains(filter) || filter.contains(&candidate) {
            suggestions.push(candidate);
        }
    }
    if suggestions.is_empty() {
        suggestions.extend([
            "pptx".to_string(),
            "xlsx".to_string(),
            "docx".to_string(),
            "slide".to_string(),
            "sheet".to_string(),
            "range".to_string(),
            "conditional-format".to_string(),
        ]);
    }
    suggestions.sort();
    suggestions.dedup();
    suggestions.truncate(8);
    suggestions
}

pub(crate) fn capability_known_filters() -> Vec<String> {
    let mut filters = Vec::new();
    filters.extend(
        CAPABILITY_COMMAND_FAMILY_FILTERS
            .iter()
            .map(|filter| (*filter).to_string()),
    );
    filters.extend(
        CAPABILITY_OBJECT_KINDS
            .iter()
            .map(|filter| (*filter).to_string()),
    );
    filters.extend(
        CAPABILITY_FILTER_ALIASES
            .iter()
            .map(|(alias, _)| (*alias).to_string()),
    );
    filters.sort();
    filters.dedup();
    filters
}

pub(crate) fn is_command_family_filter(filter: &str) -> bool {
    CAPABILITY_COMMAND_FAMILY_FILTERS.contains(&filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_registered_flag_alias_normalizes_to_its_canonical_flags() {
        let mut identities = BTreeSet::new();
        for spec in FLAG_ALIASES {
            assert!(
                identities.insert((spec.path, spec.alias)),
                "duplicate alias {} for {}",
                spec.alias,
                spec.path.join(" ")
            );
            let mut args = spec
                .path
                .iter()
                .map(|token| (*token).to_string())
                .collect::<Vec<_>>();
            args.push(spec.alias.to_string());
            args.push(
                if spec.transform == FlagAliasTransform::FreezeAt {
                    "B3"
                } else {
                    "value"
                }
                .to_string(),
            );
            let normalized = normalize_flag_aliases(&args).expect("normalize registered alias");
            assert_eq!(
                normalized.applied,
                vec![AppliedAlias::Flag(AppliedFlagAlias {
                    alias: spec.alias,
                    canonical_flags: spec.canonical_flags,
                })],
                "{} {}",
                spec.path.join(" "),
                spec.alias
            );
            let expected_tail = if spec.transform == FlagAliasTransform::FreezeAt {
                vec!["--rows", "2", "--cols", "1"]
            } else {
                vec![spec.canonical_flags[0], "value"]
            };
            assert_eq!(
                &normalized.args[spec.path.len()..],
                expected_tail,
                "{} {}",
                spec.path.join(" "),
                spec.alias
            );
        }
        assert_eq!(identities.len(), FLAG_ALIASES.len());
    }

    #[test]
    fn registered_aliases_never_shadow_a_distinct_leaf_flag() {
        let commands = crate::command_manifest::capability_commands();
        for spec in FLAG_ALIASES {
            let path = format!("ooxml {}", spec.path.join(" "));
            let command = commands
                .iter()
                .find(|command| command["path"] == path)
                .unwrap_or_else(|| panic!("alias owner is absent from manifest: {path}"));
            let local_flags = command["localFlags"]
                .as_array()
                .unwrap_or_else(|| panic!("alias owner has no localFlags array: {path}"));
            assert!(
                !local_flags.iter().any(|flag| {
                    flag["name"] == spec.alias && !spec.canonical_flags.contains(&spec.alias)
                }),
                "{} aliases {} to {}, but that leaf already defines {} with a different meaning",
                path,
                spec.alias,
                spec.canonical_flags.join(" and "),
                spec.alias
            );
        }
    }

    #[test]
    fn inline_alias_values_and_freeze_coordinates_are_preserved() {
        let renamed = normalize_flag_aliases(&[
            "xlsx".to_string(),
            "colwidths".to_string(),
            "set".to_string(),
            "--col=C:E".to_string(),
        ])
        .expect("inline rename alias");
        assert_eq!(renamed.args, ["xlsx", "colwidths", "set", "--range=C:E"]);

        let freeze = normalize_flag_aliases(&[
            "xlsx".to_string(),
            "freeze".to_string(),
            "set".to_string(),
            "--at=$C$4".to_string(),
        ])
        .expect("inline freeze alias");
        assert_eq!(
            freeze.args,
            ["xlsx", "freeze", "set", "--rows", "3", "--cols", "2"]
        );
    }

    #[test]
    fn alias_and_canonical_flags_refuse_ambiguous_duplicates() {
        let error = normalize_flag_aliases(&[
            "docx".to_string(),
            "styles".to_string(),
            "apply".to_string(),
            "--block".to_string(),
            "1".to_string(),
            "--index".to_string(),
            "2".to_string(),
        ])
        .expect_err("alias plus canonical must be refused");
        assert_eq!(
            error.message,
            "alias --block cannot be combined with canonical --index"
        );
    }

    #[test]
    fn alias_like_values_of_other_flags_are_not_rewritten() {
        let args = [
            "docx".to_string(),
            "paragraphs".to_string(),
            "insert".to_string(),
            "--text".to_string(),
            "--after".to_string(),
        ];
        let normalized = normalize_flag_aliases(&args).expect("preserve alias-like value");
        assert_eq!(normalized.args, args);
        assert!(normalized.applied.is_empty());
    }

    #[test]
    fn command_alias_normalizes_before_its_flags_and_reports_both_aliases() {
        let normalized = normalize_flag_aliases(&[
            "pptx".to_string(),
            "slides".to_string(),
            "add".to_string(),
            "deck.pptx".to_string(),
            "--after".to_string(),
            "2".to_string(),
        ])
        .expect("normalize command and flag aliases");
        assert_eq!(
            normalized.args,
            [
                "pptx",
                "new-slide-from-layout",
                "deck.pptx",
                "--insert-after",
                "2"
            ]
        );
        assert_eq!(
            normalized.applied,
            vec![
                AppliedAlias::Command(AppliedCommandAlias {
                    alias: "ooxml pptx slides add".to_string(),
                    canonical_command: "ooxml pptx new-slide-from-layout".to_string(),
                }),
                AppliedAlias::Flag(AppliedFlagAlias {
                    alias: "--after",
                    canonical_flags: &["--insert-after"],
                }),
            ]
        );
    }

    #[test]
    fn command_alias_registry_is_unique_and_canonical() {
        let mut aliases = BTreeSet::new();
        for spec in COMMAND_ALIASES {
            assert!(
                aliases.insert(spec.alias_path),
                "duplicate command alias {}",
                spec.alias_path.join(" ")
            );
            assert_ne!(spec.alias_path, spec.canonical_path);
        }
        assert_eq!(
            command_aliases_for(&["pptx", "new-slide-from-layout"]),
            ["ooxml pptx slides add"]
        );
    }
}
