use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{CliError, CliResult};

pub(crate) fn zip_entry_names(path: &str) -> CliResult<Vec<String>> {
    let mut archive = open_zip(path)?;
    let mut names = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        names.push(
            archive
                .by_index(i)
                .map_err(|err| CliError::unexpected(err.to_string()))?
                .name()
                .to_string(),
        );
    }
    Ok(names)
}

pub(crate) fn zip_entry_set(entries: &[String]) -> BTreeSet<String> {
    entries
        .iter()
        .map(|entry| format!("/{}", entry.trim_start_matches('/')))
        .collect()
}

pub(crate) fn zip_text(path: &str, name: &str) -> CliResult<String> {
    let mut archive = open_zip(path)?;
    let mut file = archive
        .by_name(name)
        .map_err(|err| CliError::unexpected(format!("missing zip part {name}: {err}")))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(text)
}

fn open_zip(path: &str) -> CliResult<ZipArchive<File>> {
    let file = File::open(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {path}"))
        } else {
            CliError::unexpected(err.to_string())
        }
    })?;
    ZipArchive::new(file).map_err(|err| CliError::unexpected(err.to_string()))
}

pub(crate) fn copy_zip_with_replacement(
    input: &str,
    output: &str,
    part: &str,
    old: &str,
    new: &str,
) -> CliResult<()> {
    if let Some(parent) = Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let in_file = File::open(input).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut archive =
        ZipArchive::new(in_file).map_err(|err| CliError::unexpected(err.to_string()))?;
    let out_file = File::create(output).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut writer = ZipWriter::new(out_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.is_dir() {
            writer
                .add_directory(entry.name(), options)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            continue;
        }
        writer
            .start_file(entry.name(), options)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.name() == part {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            writer
                .write_all(text.replace(old, new).as_bytes())
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        } else {
            std::io::copy(&mut entry, &mut writer)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        }
    }
    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

pub(crate) fn copy_zip_with_part_override(
    input: &str,
    output: &str,
    part: &str,
    text: &str,
) -> CliResult<()> {
    let mut overrides = BTreeMap::new();
    overrides.insert(part.to_string(), text.to_string());
    copy_zip_with_part_overrides(input, output, &overrides)
}

pub(crate) fn copy_zip_with_part_overrides(
    input: &str,
    output: &str,
    overrides: &BTreeMap<String, String>,
) -> CliResult<()> {
    copy_zip_with_part_overrides_and_removals(input, output, overrides, &BTreeSet::new())
}

pub(crate) fn copy_zip_with_part_overrides_and_removals(
    input: &str,
    output: &str,
    overrides: &BTreeMap<String, String>,
    removals: &BTreeSet<String>,
) -> CliResult<()> {
    if let Some(parent) = Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let in_file = File::open(input).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut archive =
        ZipArchive::new(in_file).map_err(|err| CliError::unexpected(err.to_string()))?;
    let out_file = File::create(output).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut writer = ZipWriter::new(out_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut written = BTreeSet::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.is_dir() {
            writer
                .add_directory(entry.name(), options)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            continue;
        }
        let name = entry.name().to_string();
        if removals.contains(&name) && !overrides.contains_key(&name) {
            continue;
        }
        writer
            .start_file(&name, options)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if let Some(text) = overrides.get(&name) {
            writer
                .write_all(text.as_bytes())
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        } else {
            std::io::copy(&mut entry, &mut writer)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        }
        written.insert(name);
    }
    for (name, text) in overrides {
        if written.contains(name) {
            continue;
        }
        if removals.contains(name) {
            continue;
        }
        writer
            .start_file(name, options)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        writer
            .write_all(text.as_bytes())
            .map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}
