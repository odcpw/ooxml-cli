use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{CliError, CliResult};

const MAX_ZIP_PART_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ZIP_PACKAGE_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;

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

pub(crate) fn zip_entry_exists(entries: &[String], uri: &str) -> bool {
    let wanted = format!("/{}", uri.trim_start_matches('/'));
    entries
        .iter()
        .any(|entry| format!("/{}", entry.trim_start_matches('/')) == wanted)
}

pub(crate) fn zip_text(path: &str, name: &str) -> CliResult<String> {
    let mut archive = open_zip(path)?;
    let mut file = archive
        .by_name(name)
        .map_err(|err| CliError::unexpected(format!("missing zip part {name}: {err}")))?;
    let declared_size = file.size();
    read_zip_text_entry_limited(
        &mut file,
        name,
        declared_size,
        MAX_ZIP_PART_UNCOMPRESSED_BYTES,
    )
}

pub(crate) fn zip_bytes(path: &str, name: &str) -> CliResult<Vec<u8>> {
    let mut archive = open_zip(path)?;
    let mut file = archive
        .by_name(name)
        .map_err(|err| CliError::unexpected(format!("missing zip part {name}: {err}")))?;
    let declared_size = file.size();
    read_zip_bytes_entry_limited(
        &mut file,
        name,
        declared_size,
        MAX_ZIP_PART_UNCOMPRESSED_BYTES,
    )
}

pub(crate) fn with_zip_entry_reader<T>(
    path: &str,
    name: &str,
    parse: impl FnOnce(&mut dyn BufRead) -> CliResult<T>,
) -> CliResult<T> {
    let mut archive = open_zip(path)?;
    let file = archive
        .by_name(name)
        .map_err(|err| CliError::unexpected(format!("missing zip part {name}: {err}")))?;
    let declared_size = file.size();
    check_zip_entry_declared_size(name, declared_size, MAX_ZIP_PART_UNCOMPRESSED_BYTES)?;

    let counting = LimitedCountingReader::new(file, name, MAX_ZIP_PART_UNCOMPRESSED_BYTES);
    let mut reader = BufReader::new(counting);
    let parsed = parse(&mut reader);
    let drained = std::io::copy(&mut reader, &mut std::io::sink())
        .map_err(|err| CliError::unexpected(format!("failed to read zip entry {name}: {err}")));
    let actual_size = reader.get_ref().bytes_read;

    let parsed = parsed?;
    drained?;
    if actual_size != declared_size {
        return Err(CliError::unexpected(format!(
            "zip entry {name} uncompressed size mismatch ({actual_size} != declared {declared_size} bytes)"
        )));
    }
    Ok(parsed)
}

struct LimitedCountingReader<R> {
    inner: R,
    name: String,
    limit: u64,
    bytes_read: u64,
}

impl<R> LimitedCountingReader<R> {
    fn new(inner: R, name: &str, limit: u64) -> Self {
        Self {
            inner,
            name: name.to_string(),
            limit,
            bytes_read: 0,
        }
    }
}

impl<R: Read> Read for LimitedCountingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.bytes_read = self.bytes_read.saturating_add(read as u64);
        if self.bytes_read > self.limit {
            return Err(std::io::Error::other(format!(
                "zip entry {} exceeds uncompressed size limit ({} > {} bytes)",
                self.name, self.bytes_read, self.limit
            )));
        }
        Ok(read)
    }
}

fn open_zip(path: &str) -> CliResult<ZipArchive<File>> {
    let mut archive = open_zip_unchecked(path)?;
    check_zip_archive_uncompressed_size(&mut archive)?;
    Ok(archive)
}

fn open_zip_unchecked(path: &str) -> CliResult<ZipArchive<File>> {
    let file = File::open(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {path}"))
        } else {
            CliError::unexpected(err.to_string())
        }
    })?;
    ZipArchive::new(file).map_err(|err| CliError::unexpected(err.to_string()))
}

fn check_zip_archive_uncompressed_size<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> CliResult<()> {
    check_zip_archive_uncompressed_size_with_limits(
        archive,
        MAX_ZIP_PART_UNCOMPRESSED_BYTES,
        MAX_ZIP_PACKAGE_UNCOMPRESSED_BYTES,
    )
}

fn check_zip_archive_uncompressed_size_with_limits<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    part_limit: u64,
    package_limit: u64,
) -> CliResult<()> {
    let mut total = 0_u64;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let size = entry.size();
        check_zip_entry_declared_size(&name, size, part_limit)?;
        total = total.saturating_add(size);
        if total > package_limit {
            return Err(CliError::unexpected(format!(
                "zip package exceeds total uncompressed size limit ({total} > {package_limit} bytes)"
            )));
        }
    }
    Ok(())
}

fn check_zip_entry_declared_size(name: &str, size: u64, limit: u64) -> CliResult<()> {
    if size > limit {
        return Err(CliError::unexpected(format!(
            "zip entry {name} is too large ({size} bytes uncompressed; limit {limit})"
        )));
    }
    Ok(())
}

fn read_zip_text_entry_limited<R: Read>(
    reader: R,
    name: &str,
    declared_size: u64,
    limit: u64,
) -> CliResult<String> {
    check_zip_entry_declared_size(name, declared_size, limit)?;
    let mut text = String::new();
    reader
        .take(limit.saturating_add(1))
        .read_to_string(&mut text)
        .map_err(|err| CliError::unexpected(format!("failed to read zip entry {name}: {err}")))?;
    if text.len() as u64 > limit {
        return Err(CliError::unexpected(format!(
            "zip entry {name} exceeds uncompressed size limit ({} > {limit} bytes)",
            text.len()
        )));
    }
    Ok(text)
}

fn read_zip_bytes_entry_limited<R: Read>(
    mut reader: R,
    name: &str,
    declared_size: u64,
    limit: u64,
) -> CliResult<Vec<u8>> {
    check_zip_entry_declared_size(name, declared_size, limit)?;
    let mut data = Vec::new();
    reader
        .by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut data)
        .map_err(|err| CliError::unexpected(format!("failed to read zip entry {name}: {err}")))?;
    if data.len() as u64 > limit {
        return Err(CliError::unexpected(format!(
            "zip entry {name} exceeds uncompressed size limit ({} > {limit} bytes)",
            data.len()
        )));
    }
    Ok(data)
}

fn copy_zip_entry_limited<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    name: &str,
    declared_size: u64,
    limit: u64,
) -> CliResult<u64> {
    check_zip_entry_declared_size(name, declared_size, limit)?;
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|err| {
            CliError::unexpected(format!("failed to read zip entry {name}: {err}"))
        })?;
        if read == 0 {
            return Ok(copied);
        }
        copied = copied.saturating_add(read as u64);
        if copied > limit {
            return Err(CliError::unexpected(format!(
                "zip entry {name} exceeds uncompressed size limit ({copied} > {limit} bytes)"
            )));
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|err| CliError::unexpected(err.to_string()))?;
    }
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
    copy_zip_with_text_and_binary_part_overrides_and_removals(
        input,
        output,
        overrides,
        &BTreeMap::new(),
        removals,
    )
}

pub(crate) fn copy_zip_with_binary_part_overrides_and_removals(
    input: &str,
    output: &str,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    removals: &BTreeSet<String>,
) -> CliResult<()> {
    copy_zip_with_text_and_binary_part_overrides_and_removals(
        input,
        output,
        text_overrides,
        binary_overrides,
        removals,
    )
}

fn copy_zip_with_text_and_binary_part_overrides_and_removals(
    input: &str,
    output: &str,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    removals: &BTreeSet<String>,
) -> CliResult<()> {
    ensure_zip_rewrite_paths_are_distinct(input, output)?;
    let mut archive = open_zip_unchecked(input)?;
    preflight_zip_rewrite_with_limits(
        &mut archive,
        text_overrides,
        binary_overrides,
        removals,
        MAX_ZIP_PART_UNCOMPRESSED_BYTES,
        MAX_ZIP_PACKAGE_UNCOMPRESSED_BYTES,
    )?;

    if let Some(parent) = Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let out_file = File::create(output).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut writer = ZipWriter::new(out_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut written = BTreeSet::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.is_dir() {
            writer
                .add_directory(entry.name(), options)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            continue;
        }
        let name = entry.name().to_string();
        if removals.contains(&name)
            && !text_overrides.contains_key(&name)
            && !binary_overrides.contains_key(&name)
        {
            continue;
        }
        if let Some(data) = binary_overrides.get(&name) {
            writer
                .start_file(&name, options)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            writer
                .write_all(data)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        } else if let Some(text) = text_overrides.get(&name) {
            writer
                .start_file(&name, options)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            writer
                .write_all(text.as_bytes())
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        } else {
            writer.raw_copy_file(entry).map_err(|err| {
                CliError::unexpected(format!("failed to raw-copy zip entry {name}: {err}"))
            })?;
        }
        written.insert(name);
    }
    for (name, text) in text_overrides {
        if binary_overrides.contains_key(name) {
            continue;
        }
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
    for (name, data) in binary_overrides {
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
            .write_all(data)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn ensure_zip_rewrite_paths_are_distinct(input: &str, output: &str) -> CliResult<()> {
    let input_path = Path::new(input);
    let output_path = Path::new(output);
    let alias_error = || {
        CliError::unexpected(format!(
            "refusing to rewrite zip archive in place: output resolves to input ({output})"
        ))
    };

    if input_path == output_path {
        return Err(alias_error());
    }

    if let (Ok(input_canonical), Ok(output_canonical)) =
        (fs::canonicalize(input_path), fs::canonicalize(output_path))
        && input_canonical == output_canonical
    {
        return Err(alias_error());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        if let (Ok(input_metadata), Ok(output_metadata)) =
            (fs::metadata(input_path), fs::metadata(output_path))
            && input_metadata.dev() == output_metadata.dev()
            && input_metadata.ino() == output_metadata.ino()
        {
            return Err(alias_error());
        }
    }

    Ok(())
}

fn preflight_zip_rewrite_with_limits<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    removals: &BTreeSet<String>,
    part_limit: u64,
    package_limit: u64,
) -> CliResult<u64> {
    let mut final_output_size = 0_u64;
    let mut written = BTreeSet::new();
    let mut output_names = BTreeSet::new();
    let mut duplicate_output_name = false;

    for i in 0..archive.len() {
        let (name, is_dir) = {
            let entry = archive.by_index_raw(i).map_err(|err| {
                CliError::unexpected(format!("failed to inspect zip entry {i}: {err}"))
            })?;
            (entry.name().to_string(), entry.is_dir())
        };
        if is_dir {
            duplicate_output_name |= !output_names.insert(name);
            continue;
        }
        if removals.contains(&name)
            && !text_overrides.contains_key(&name)
            && !binary_overrides.contains_key(&name)
        {
            continue;
        }
        duplicate_output_name |= !output_names.insert(name.clone());

        let actual_size = if let Some(data) = binary_overrides.get(&name) {
            data.len() as u64
        } else if let Some(text) = text_overrides.get(&name) {
            text.len() as u64
        } else {
            let mut entry = archive.by_index(i).map_err(|err| {
                CliError::unexpected(format!(
                    "failed to preflight unchanged zip entry {name}: {err}"
                ))
            })?;
            let declared_size = entry.size();
            let actual_size = copy_zip_entry_limited(
                &mut entry,
                &mut std::io::sink(),
                &name,
                declared_size,
                part_limit,
            )?;
            if actual_size != declared_size {
                return Err(CliError::unexpected(format!(
                    "zip entry {name} uncompressed size mismatch ({actual_size} != declared {declared_size} bytes)"
                )));
            }
            actual_size
        };
        add_final_output_actual_size(
            &mut final_output_size,
            &name,
            actual_size,
            part_limit,
            package_limit,
        )?;
        written.insert(name);
    }

    for (name, text) in text_overrides {
        if binary_overrides.contains_key(name) || written.contains(name) || removals.contains(name)
        {
            continue;
        }
        duplicate_output_name |= !output_names.insert(name.clone());
        add_final_output_actual_size(
            &mut final_output_size,
            name,
            text.len() as u64,
            part_limit,
            package_limit,
        )?;
    }
    for (name, data) in binary_overrides {
        if written.contains(name) || removals.contains(name) {
            continue;
        }
        duplicate_output_name |= !output_names.insert(name.clone());
        add_final_output_actual_size(
            &mut final_output_size,
            name,
            data.len() as u64,
            part_limit,
            package_limit,
        )?;
    }

    if duplicate_output_name {
        return Err(CliError::unexpected(
            "invalid Zip archive: Duplicate filename",
        ));
    }
    Ok(final_output_size)
}

fn add_final_output_actual_size(
    total: &mut u64,
    name: &str,
    actual_size: u64,
    part_limit: u64,
    package_limit: u64,
) -> CliResult<()> {
    if actual_size > part_limit {
        return Err(CliError::unexpected(format!(
            "zip output entry {name} is too large ({actual_size} bytes uncompressed; limit {part_limit})"
        )));
    }
    *total = total
        .checked_add(actual_size)
        .ok_or_else(|| CliError::unexpected("zip output total uncompressed size overflow"))?;
    if *total > package_limit {
        return Err(CliError::unexpected(format!(
            "zip output exceeds total uncompressed size limit ({} > {package_limit} bytes)",
            *total
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[derive(Debug, PartialEq, Eq)]
    struct ZipEntrySnapshot {
        name: String,
        method: CompressionMethod,
        crc32: u32,
        compressed_size: u64,
        uncompressed_size: u64,
        last_modified: Option<zip::DateTime>,
        unix_mode: Option<u32>,
        raw_payload: Vec<u8>,
        content: Vec<u8>,
    }

    fn zip_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, body) in entries {
            writer.start_file(*name, options).expect("start zip entry");
            writer.write_all(body).expect("write zip entry");
        }
        writer.finish().expect("finish zip").into_inner()
    }

    fn temp_zip_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ooxml-{label}-{}-{}.zip",
            std::process::id(),
            crate::chrono_like_counter()
        ))
    }

    fn snapshot_entry(path: &Path, wanted: &str) -> ZipEntrySnapshot {
        let mut archive = ZipArchive::new(File::open(path).expect("open zip")).expect("archive");
        let index = (0..archive.len())
            .find(|index| archive.by_index_raw(*index).expect("raw entry").name() == wanted)
            .expect("entry index");
        let (
            name,
            method,
            crc32,
            compressed_size,
            uncompressed_size,
            last_modified,
            unix_mode,
            raw_payload,
        ) = {
            let mut entry = archive.by_index_raw(index).expect("raw entry");
            let metadata = (
                entry.name().to_string(),
                entry.compression(),
                entry.crc32(),
                entry.compressed_size(),
                entry.size(),
                entry.last_modified(),
                entry.unix_mode(),
            );
            let mut raw_payload = Vec::new();
            entry.read_to_end(&mut raw_payload).expect("raw payload");
            (
                metadata.0,
                metadata.1,
                metadata.2,
                metadata.3,
                metadata.4,
                metadata.5,
                metadata.6,
                raw_payload,
            )
        };
        let mut content = Vec::new();
        archive
            .by_index(index)
            .expect("normal entry")
            .read_to_end(&mut content)
            .expect("content");
        ZipEntrySnapshot {
            name,
            method,
            crc32,
            compressed_size,
            uncompressed_size,
            last_modified,
            unix_mode,
            raw_payload,
            content,
        }
    }

    fn signature_offsets(data: &[u8], signature: &[u8]) -> Vec<usize> {
        data.windows(signature.len())
            .enumerate()
            .filter_map(|(index, window)| (window == signature).then_some(index))
            .collect()
    }

    fn overwrite_u16_for_entry(
        data: &mut [u8],
        entry_index: usize,
        local_offset: usize,
        central_offset: usize,
        value: u16,
    ) {
        let local = signature_offsets(data, b"PK\x03\x04")[entry_index];
        let central = signature_offsets(data, b"PK\x01\x02")[entry_index];
        data[local + local_offset..local + local_offset + 2].copy_from_slice(&value.to_le_bytes());
        data[central + central_offset..central + central_offset + 2]
            .copy_from_slice(&value.to_le_bytes());
    }

    fn overwrite_u32_for_entry(
        data: &mut [u8],
        entry_index: usize,
        local_offset: usize,
        central_offset: usize,
        value: u32,
    ) {
        let local = signature_offsets(data, b"PK\x03\x04")[entry_index];
        let central = signature_offsets(data, b"PK\x01\x02")[entry_index];
        data[local + local_offset..local + local_offset + 4].copy_from_slice(&value.to_le_bytes());
        data[central + central_offset..central + central_offset + 4]
            .copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn zip_text_reader_rejects_declared_oversize_entry() {
        let err = read_zip_text_entry_limited(
            Cursor::new(b"small".as_slice()),
            "word/document.xml",
            11,
            10,
        )
        .expect_err("declared oversize entry should fail");

        assert_eq!(err.code, "unexpected");
        assert!(err.message.contains("word/document.xml is too large"));
        assert!(err.message.contains("limit 10"));
    }

    #[test]
    fn zip_text_reader_rejects_underdeclared_stream_oversize() {
        let err = read_zip_text_entry_limited(
            Cursor::new(b"abcdef".as_slice()),
            "word/document.xml",
            3,
            5,
        )
        .expect_err("stream past limit should fail");

        assert_eq!(err.code, "unexpected");
        assert!(
            err.message
                .contains("word/document.xml exceeds uncompressed size limit")
        );
    }

    #[test]
    fn zip_archive_check_rejects_declared_part_oversize() {
        let data = zip_with_entries(&[("xl/workbook.xml", b"abcdef")]);
        let mut archive = ZipArchive::new(Cursor::new(data)).expect("open test zip");

        let err = check_zip_archive_uncompressed_size_with_limits(&mut archive, 5, 100)
            .expect_err("part over limit should fail");

        assert_eq!(err.code, "unexpected");
        assert!(err.message.contains("xl/workbook.xml is too large"));
    }

    #[test]
    fn zip_archive_check_rejects_total_uncompressed_oversize() {
        let data = zip_with_entries(&[("a.xml", b"abc"), ("b.xml", b"def")]);
        let mut archive = ZipArchive::new(Cursor::new(data)).expect("open test zip");

        let err = check_zip_archive_uncompressed_size_with_limits(&mut archive, 10, 5)
            .expect_err("package over limit should fail");

        assert_eq!(err.code, "unexpected");
        assert!(
            err.message
                .contains("zip package exceeds total uncompressed size limit")
        );
    }

    #[test]
    fn zip_entry_reader_drains_after_callback_and_checks_crc() {
        let temp = std::env::temp_dir().join(format!(
            "ooxml-zip-reader-{}-{}.zip",
            std::process::id(),
            crate::chrono_like_counter()
        ));
        fs::write(&temp, zip_with_entries(&[("sheet.xml", b"abcdef")])).expect("write zip");
        let value =
            with_zip_entry_reader(temp.to_str().expect("temp path"), "sheet.xml", |reader| {
                let mut prefix = [0_u8; 2];
                reader
                    .read_exact(&mut prefix)
                    .map_err(|err| CliError::unexpected(err.to_string()))?;
                Ok(prefix)
            })
            .expect("stream zip entry");
        assert_eq!(&value, b"ab");
        let _ = fs::remove_file(temp);
    }

    #[test]
    fn zip_entry_reader_rejects_bad_crc_even_after_short_callback() {
        let mut data = zip_with_entries(&[("sheet.xml", b"payload with a nonzero crc")]);
        overwrite_crc_after_signature(&mut data, b"PK\x03\x04", 14);
        overwrite_crc_after_signature(&mut data, b"PK\x01\x02", 16);
        let temp = std::env::temp_dir().join(format!(
            "ooxml-zip-reader-bad-crc-{}-{}.zip",
            std::process::id(),
            crate::chrono_like_counter()
        ));
        fs::write(&temp, data).expect("write corrupt zip");
        let err =
            with_zip_entry_reader(temp.to_str().expect("temp path"), "sheet.xml", |_reader| {
                Ok(())
            })
            .expect_err("bad CRC must fail");
        assert!(err.message.to_ascii_lowercase().contains("checksum"));
        let _ = fs::remove_file(temp);
    }

    fn overwrite_crc_after_signature(data: &mut [u8], signature: &[u8], crc_offset: usize) {
        let start = data
            .windows(signature.len())
            .position(|window| window == signature)
            .expect("zip signature");
        data[start + crc_offset..start + crc_offset + 4].copy_from_slice(&0_u32.to_le_bytes());
    }

    #[test]
    fn raw_copy_preserves_stored_and_deflated_payloads_and_metadata() {
        let input = temp_zip_path("raw-copy-input");
        let output = temp_zip_path("raw-copy-output");
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let stored_options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .last_modified_time(
                zip::DateTime::from_date_and_time(2024, 5, 6, 7, 8, 10).expect("date"),
            )
            .unix_permissions(0o640);
        let deflated_options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(
                zip::DateTime::from_date_and_time(2025, 6, 7, 8, 9, 12).expect("date"),
            )
            .unix_permissions(0o600);
        writer
            .start_file("stored.bin", stored_options)
            .expect("stored");
        writer.write_all(b"stored payload").expect("stored payload");
        writer
            .start_file("deflated.bin", deflated_options)
            .expect("deflated");
        writer
            .write_all(b"deflated payload deflated payload deflated payload")
            .expect("deflated payload");
        writer
            .start_file("changed.xml", deflated_options)
            .expect("changed");
        writer.write_all(b"old").expect("old");
        fs::write(&input, writer.finish().expect("finish").into_inner()).expect("input");

        let stored_before = snapshot_entry(&input, "stored.bin");
        let deflated_before = snapshot_entry(&input, "deflated.bin");
        copy_zip_with_part_override(
            input.to_str().expect("input path"),
            output.to_str().expect("output path"),
            "changed.xml",
            "new",
        )
        .expect("copy zip");
        assert_eq!(snapshot_entry(&output, "stored.bin"), stored_before);
        assert_eq!(snapshot_entry(&output, "deflated.bin"), deflated_before);
        assert_eq!(snapshot_entry(&output, "changed.xml").content, b"new");
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn corrupt_late_unchanged_entry_fails_before_output_is_truncated() {
        let mut data = zip_with_entries(&[("first.xml", b"first"), ("late.xml", b"late")]);
        overwrite_u32_for_entry(&mut data, 1, 14, 16, 0);
        let input = temp_zip_path("raw-copy-corrupt-input");
        let output = temp_zip_path("raw-copy-corrupt-output");
        fs::write(&input, data).expect("corrupt input");
        fs::write(&output, b"sentinel output").expect("sentinel");
        copy_zip_with_part_override(
            input.to_str().expect("input path"),
            output.to_str().expect("output path"),
            "first.xml",
            "replacement",
        )
        .expect_err("late CRC error must fail preflight");
        assert_eq!(
            fs::read(&output).expect("sentinel after"),
            b"sentinel output"
        );
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn underdeclared_unchanged_entry_fails_before_output_is_truncated() {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        writer
            .start_file(
                "underdeclared.bin",
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .expect("entry");
        writer.write_all(b"abcdef").expect("payload");
        let mut data = writer.finish().expect("finish").into_inner();
        overwrite_u32_for_entry(&mut data, 0, 22, 24, 3);
        let input = temp_zip_path("raw-copy-underdeclared-input");
        let output = temp_zip_path("raw-copy-underdeclared-output");
        fs::write(&input, data).expect("input");
        fs::write(&output, b"sentinel output").expect("sentinel");
        copy_zip_with_part_overrides(
            input.to_str().expect("input path"),
            output.to_str().expect("output path"),
            &BTreeMap::new(),
        )
        .expect_err("underdeclared entry must fail preflight");
        assert_eq!(
            fs::read(&output).expect("sentinel after"),
            b"sentinel output"
        );
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    fn assert_rewrite_alias_rejected_without_input_mutation(input: &Path, output: &Path) {
        let input_before = fs::read(input).expect("input before");
        let err = copy_zip_with_part_overrides(
            input.to_str().expect("input path"),
            output.to_str().expect("output path"),
            &BTreeMap::new(),
        )
        .expect_err("input/output alias must be rejected");
        assert!(err.message.contains("output resolves to input"));
        assert_eq!(fs::read(input).expect("input after"), input_before);
    }

    #[test]
    fn rewrite_rejects_exact_input_path_without_mutation() {
        let input = temp_zip_path("raw-copy-exact-alias");
        fs::write(&input, zip_with_entries(&[("sheet.xml", b"unchanged")])).expect("input");

        assert_rewrite_alias_rejected_without_input_mutation(&input, &input);

        let _ = fs::remove_file(input);
    }

    #[cfg(unix)]
    #[test]
    fn rewrite_rejects_symlink_to_input_without_mutation() {
        use std::os::unix::fs::symlink;

        let input = temp_zip_path("raw-copy-symlink-input");
        let output = temp_zip_path("raw-copy-symlink-output");
        fs::write(&input, zip_with_entries(&[("sheet.xml", b"unchanged")])).expect("input");
        symlink(&input, &output).expect("output symlink");

        assert_rewrite_alias_rejected_without_input_mutation(&input, &output);

        let _ = fs::remove_file(output);
        let _ = fs::remove_file(input);
    }

    #[cfg(unix)]
    #[test]
    fn rewrite_rejects_hardlink_to_input_without_mutation() {
        let input = temp_zip_path("raw-copy-hardlink-input");
        let output = temp_zip_path("raw-copy-hardlink-output");
        fs::write(&input, zip_with_entries(&[("sheet.xml", b"unchanged")])).expect("input");
        fs::hard_link(&input, &output).expect("output hardlink");

        assert_rewrite_alias_rejected_without_input_mutation(&input, &output);

        let _ = fs::remove_file(output);
        let _ = fs::remove_file(input);
    }

    #[test]
    fn final_output_actual_size_helper_enforces_part_and_aggregate_limits() {
        let mut total = 0;
        add_final_output_actual_size(&mut total, "a.xml", 4, 5, 7).expect("first entry");
        let aggregate = add_final_output_actual_size(&mut total, "b.xml", 4, 5, 7)
            .expect_err("aggregate limit");
        assert!(aggregate.message.contains("total uncompressed size limit"));

        let mut total = 0;
        let part = add_final_output_actual_size(&mut total, "large.xml", 6, 5, 100)
            .expect_err("part limit");
        assert!(part.message.contains("large.xml is too large"));
    }

    #[test]
    fn rewrite_preflight_counts_override_and_new_bytes_but_excludes_removals() {
        let data = zip_with_entries(&[("changed.xml", b"old-old"), ("removed.xml", b"remove")]);
        let mut archive = ZipArchive::new(Cursor::new(data)).expect("archive");
        let text_overrides = BTreeMap::from([
            ("changed.xml".to_string(), "four".to_string()),
            ("new.xml".to_string(), "more".to_string()),
        ]);
        let removals = BTreeSet::from(["removed.xml".to_string()]);
        let total = preflight_zip_rewrite_with_limits(
            &mut archive,
            &text_overrides,
            &BTreeMap::new(),
            &removals,
            10,
            8,
        )
        .expect("exact final size");
        assert_eq!(total, 8);

        let data = zip_with_entries(&[("changed.xml", b"old-old"), ("removed.xml", b"remove")]);
        let mut archive = ZipArchive::new(Cursor::new(data)).expect("archive");
        let err = preflight_zip_rewrite_with_limits(
            &mut archive,
            &text_overrides,
            &BTreeMap::new(),
            &removals,
            10,
            7,
        )
        .expect_err("new and override bytes exceed aggregate");
        assert!(err.message.contains("total uncompressed size limit"));
    }

    #[test]
    fn raw_copy_preserves_override_removal_and_duplicate_name_precedence() {
        let input = temp_zip_path("raw-copy-precedence-input");
        let output = temp_zip_path("raw-copy-precedence-output");
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        writer.add_directory("folder/", options).expect("directory");
        for (name, body) in [
            ("a.xml", b"old-a".as_slice()),
            ("b.xml", b"old-b".as_slice()),
            ("c.xml", b"old-c".as_slice()),
        ] {
            writer.start_file(name, options).expect("entry");
            writer.write_all(body).expect("body");
        }
        fs::write(&input, writer.finish().expect("finish").into_inner()).expect("input");
        let text_overrides = BTreeMap::from([
            ("a.xml".to_string(), "text-loses".to_string()),
            ("removed-new.xml".to_string(), "not-added".to_string()),
            ("new.xml".to_string(), "new-text".to_string()),
        ]);
        let binary_overrides = BTreeMap::from([("a.xml".to_string(), b"binary-wins".to_vec())]);
        let removals = BTreeSet::from([
            "a.xml".to_string(),
            "b.xml".to_string(),
            "removed-new.xml".to_string(),
        ]);
        copy_zip_with_binary_part_overrides_and_removals(
            input.to_str().expect("input path"),
            output.to_str().expect("output path"),
            &text_overrides,
            &binary_overrides,
            &removals,
        )
        .expect("rewrite");

        let mut archive = ZipArchive::new(File::open(&output).expect("output")).expect("archive");
        let mut entries = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).expect("entry");
            let mut body = Vec::new();
            entry.read_to_end(&mut body).expect("body");
            entries.push((entry.name().to_string(), entry.is_dir(), body));
        }
        assert_eq!(
            entries,
            vec![
                ("folder/".to_string(), true, Vec::new()),
                ("a.xml".to_string(), false, b"binary-wins".to_vec()),
                ("c.xml".to_string(), false, b"old-c".to_vec()),
                ("new.xml".to_string(), false, b"new-text".to_vec()),
            ]
        );
        for index in [1, 3] {
            assert_eq!(
                archive
                    .by_index_raw(index)
                    .expect("raw entry")
                    .compression(),
                CompressionMethod::Deflated
            );
        }
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn duplicate_central_names_remain_deduplicated_like_zip_archive_input() {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, body) in [
            ("duplicate-a.xml", b"first".as_slice()),
            ("duplicate-b.xml", b"second".as_slice()),
        ] {
            writer.start_file(name, options).expect("entry");
            writer.write_all(body).expect("body");
        }
        let mut data = writer.finish().expect("finish").into_inner();
        let old_name = b"duplicate-b.xml";
        let new_name = b"duplicate-a.xml";
        let offsets = data
            .windows(old_name.len())
            .enumerate()
            .filter_map(|(index, window)| (window == old_name).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(offsets.len(), 2, "local and central names");
        for offset in offsets {
            data[offset..offset + old_name.len()].copy_from_slice(new_name);
        }
        let input = temp_zip_path("raw-copy-duplicate-input");
        let output = temp_zip_path("raw-copy-duplicate-output");
        fs::write(&input, data).expect("input");
        copy_zip_with_part_overrides(
            input.to_str().expect("input path"),
            output.to_str().expect("output path"),
            &BTreeMap::new(),
        )
        .expect("deduplicated rewrite");
        let before = snapshot_entry(&input, "duplicate-a.xml");
        let after = snapshot_entry(&output, "duplicate-a.xml");
        assert_eq!(before, after);
        let archive = ZipArchive::new(File::open(&output).expect("output")).expect("archive");
        assert_eq!(archive.len(), 1);
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn new_override_colliding_with_existing_directory_fails_before_output_mutation() {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        writer
            .add_directory("collision/", SimpleFileOptions::default())
            .expect("directory");
        let input = temp_zip_path("raw-copy-directory-collision-input");
        let output = temp_zip_path("raw-copy-directory-collision-output");
        fs::write(&input, writer.finish().expect("finish").into_inner()).expect("input");
        fs::write(&output, b"sentinel output").expect("sentinel");
        let err = copy_zip_with_part_overrides(
            input.to_str().expect("input path"),
            output.to_str().expect("output path"),
            &BTreeMap::from([("collision/".to_string(), "file".to_string())]),
        )
        .expect_err("directory collision");
        assert!(err.message.contains("Duplicate filename"));
        assert_eq!(
            fs::read(&output).expect("sentinel after"),
            b"sentinel output"
        );
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn encrypted_or_unsupported_unchanged_entries_fail_preflight() {
        for (label, patch) in [
            ("encrypted", (6, 8, 1_u16)),
            ("unsupported", (8, 10, 99_u16)),
        ] {
            let mut data = zip_with_entries(&[("unchanged.xml", b"payload")]);
            overwrite_u16_for_entry(&mut data, 0, patch.0, patch.1, patch.2);
            let input = temp_zip_path(&format!("raw-copy-{label}-input"));
            let output = temp_zip_path(&format!("raw-copy-{label}-output"));
            fs::write(&input, data).expect("input");
            fs::write(&output, b"sentinel output").expect("sentinel");
            copy_zip_with_part_overrides(
                input.to_str().expect("input path"),
                output.to_str().expect("output path"),
                &BTreeMap::new(),
            )
            .expect_err("unsupported unchanged entry must fail");
            assert_eq!(
                fs::read(&output).expect("sentinel after"),
                b"sentinel output"
            );
            let _ = fs::remove_file(input);
            let _ = fs::remove_file(output);
        }
    }
}
