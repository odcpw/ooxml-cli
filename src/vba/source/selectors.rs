use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::{CliError, CliResult, selector_candidates};

use super::super::output::vba_list_command;
use super::SourceModule;
use super::codec::extension_for_module_kind;
pub(super) fn with_source_module_selectors(mut module: SourceModule) -> SourceModule {
    let mut builder = SelectorBuilder::default();
    if !module.name.trim().is_empty() {
        module.primary_selector = format!("module:{}", module.name);
    } else if module.number > 0 {
        module.primary_selector = format!("module:{}", module.number);
    }
    builder.add(&module.primary_selector);
    if module.number > 0 {
        builder.add(&format!("module:{}", module.number));
        builder.add(&format!("#{}", module.number));
    }
    if !module.name.trim().is_empty() {
        builder.add(&format!("module:{}", module.name));
        builder.add(&format!("name:{}", module.name));
        builder.add(&format!("~{}", module.name));
        builder.add(&module.name);
    }
    if !module.stream_name.trim().is_empty() {
        builder.add(&format!("stream:{}", module.stream_name));
    }
    module.selectors = builder.values;
    module
}

#[derive(Default)]
struct SelectorBuilder {
    values: Vec<String>,
    seen: BTreeMap<String, bool>,
}

impl SelectorBuilder {
    fn add(&mut self, value: &str) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        let key = value.to_ascii_lowercase();
        if self.seen.contains_key(&key) {
            return;
        }
        self.seen.insert(key, true);
        self.values.push(value.to_string());
    }
}

pub(super) fn module_output_name(module: &SourceModule) -> String {
    let mut name = if module.name.trim().is_empty() {
        module.stream_name.clone()
    } else {
        module.name.clone()
    };
    if name.trim().is_empty() {
        name = format!("module-{}", module.number);
    }
    name = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ' ' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches([' ', '.'])
        .to_string();
    if name.is_empty() {
        name = format!("module-{}", module.number);
    }
    let extension = if module.extension.is_empty() {
        extension_for_module_kind(&module.kind)
    } else {
        &module.extension
    };
    let mut path = PathBuf::from(name);
    path.set_extension(extension.trim_start_matches('.'));
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("module.bas")
        .to_string()
}

pub(super) fn select_modules(
    file: &str,
    modules: &[SourceModule],
    selector: &str,
) -> CliResult<Vec<SourceModule>> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Ok(modules.to_vec());
    }
    let matches = modules
        .iter()
        .filter(|module| {
            module
                .selectors
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(selector))
        })
        .cloned()
        .collect::<Vec<_>>();
    match matches.len() {
        0 => Err(vba_module_not_found_error(file, modules, selector)),
        1 => Ok(matches),
        _ => Err(CliError::invalid_args(format!(
            "VBA module selector {selector:?} matched multiple modules ({}); use a more specific selector; discover with `{}`",
            ambiguous_module_selectors(&matches).join(", "),
            vba_list_command(file)
        ))),
    }
}

fn vba_module_not_found_error(file: &str, modules: &[SourceModule], selector: &str) -> CliError {
    let candidates = selector_candidates(
        &modules
            .iter()
            .map(|module| {
                (
                    module.primary_selector.as_str(),
                    module.selectors.as_slice(),
                )
            })
            .collect::<Vec<_>>(),
        selector,
        3,
    );
    let mut message = format!("VBA module not found: {selector}");
    if !candidates.is_empty() {
        message.push_str(&format!("; did you mean: {}", candidates.join(", ")));
    }
    message.push_str(&format!("; discover with `{}`", vba_list_command(file)));
    CliError::target_not_found(message)
}

fn ambiguous_module_selectors(modules: &[SourceModule]) -> Vec<String> {
    let mut primary_counts = BTreeMap::<String, usize>::new();
    for module in modules {
        let primary = module.primary_selector.trim();
        if !primary.is_empty() {
            *primary_counts
                .entry(primary.to_ascii_lowercase())
                .or_insert(0) += 1;
        }
    }
    modules
        .iter()
        .filter_map(|module| {
            let primary = module.primary_selector.trim();
            if !primary.is_empty()
                && primary_counts
                    .get(&primary.to_ascii_lowercase())
                    .copied()
                    .unwrap_or_default()
                    == 1
            {
                return Some(primary.to_string());
            }
            if module.number > 0 {
                return Some(format!("module:{}", module.number));
            }
            if !primary.is_empty() {
                return Some(primary.to_string());
            }
            None
        })
        .collect()
}
