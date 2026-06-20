use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use zip::DateTime;

use crate::{CliError, CliResult};

use super::types::PartInfo;
use super::util::diag;

pub(super) struct ZipEntryMetadata {
    modified_times: BTreeMap<String, ZipModifiedTime>,
}

#[derive(Clone, Copy)]
struct ZipModifiedTime {
    datepart: u16,
    timepart: u16,
}

pub(super) fn read_zip_entry_metadata(file: &str) -> CliResult<ZipEntryMetadata> {
    let data = fs::read(file).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {file}"))
        } else {
            CliError::unexpected(err.to_string())
        }
    })?;
    let mut modified_times = BTreeMap::new();
    let mut i = 0;
    while i + 46 <= data.len() {
        if data[i..i + 4] != [0x50, 0x4b, 0x01, 0x02] {
            i += 1;
            continue;
        }
        let timepart = read_u16_le(&data, i + 12);
        let datepart = read_u16_le(&data, i + 14);
        let name_len = read_u16_le(&data, i + 28) as usize;
        let extra_len = read_u16_le(&data, i + 30) as usize;
        let comment_len = read_u16_le(&data, i + 32) as usize;
        let name_start = i + 46;
        let name_end = name_start.saturating_add(name_len);
        let header_end = name_end
            .saturating_add(extra_len)
            .saturating_add(comment_len);
        if header_end > data.len() {
            i += 1;
            continue;
        }
        if let Ok(name) = std::str::from_utf8(&data[name_start..name_end])
            && !name.ends_with('/')
        {
            modified_times.insert(name.to_string(), ZipModifiedTime { datepart, timepart });
        }
        i = header_end.max(i + 1);
    }
    Ok(ZipEntryMetadata { modified_times })
}

pub(super) fn check_zip_entry_metadata(metadata: &ZipEntryMetadata, part: &PartInfo) -> Vec<Value> {
    let modified = metadata.modified_times.get(&part.entry_name).copied();
    let Some(modified) = modified else {
        return Vec::new();
    };
    if zip_modified_time_is_invalid(modified) {
        return vec![diag(
            "OOXML_ZIP_TIMESTAMP_INVALID",
            format!(
                "{} has invalid ZIP modified time {}; Office may repair packages with zero or pre-1980 ZIP dates",
                part.uri,
                format_zip_modified_time(modified)
            ),
        )];
    }
    Vec::new()
}

fn zip_modified_time_is_invalid(modified: ZipModifiedTime) -> bool {
    match DateTime::try_from_msdos(modified.datepart, modified.timepart) {
        Ok(modified) => modified < DateTime::default(),
        Err(_) => true,
    }
}

fn format_zip_modified_time(modified: ZipModifiedTime) -> String {
    match DateTime::try_from_msdos(modified.datepart, modified.timepart) {
        Ok(modified) => format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            modified.year(),
            modified.month(),
            modified.day(),
            modified.hour(),
            modified.minute(),
            modified.second()
        ),
        Err(_) if modified.datepart == 0 && modified.timepart == 0 => {
            "1979-11-30T00:00:00Z".to_string()
        }
        Err(_) => {
            let second = ((modified.timepart & 0b0000_0000_0001_1111) << 1) as u8;
            let minute = ((modified.timepart & 0b0000_0111_1110_0000) >> 5) as u8;
            let hour = (modified.timepart >> 11) as u8;
            let day = (modified.datepart & 0b0000_0000_0001_1111) as u8;
            let month = ((modified.datepart & 0b0000_0001_1110_0000) >> 5) as u8;
            let year = (modified.datepart >> 9) + 1980;
            format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
        }
    }
}

fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}
