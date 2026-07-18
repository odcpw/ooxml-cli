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
    let file = File::open(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {path}"))
        } else {
            CliError::unexpected(err.to_string())
        }
    })?;
    let mut archive = ZipArchive::new(file).map_err(|err| CliError::unexpected(err.to_string()))?;
    check_zip_archive_uncompressed_size(&mut archive)?;
    Ok(archive)
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
    if let Some(parent) = Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let mut archive = open_zip(input)?;
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
        if removals.contains(&name)
            && !text_overrides.contains_key(&name)
            && !binary_overrides.contains_key(&name)
        {
            continue;
        }
        writer
            .start_file(&name, options)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if let Some(data) = binary_overrides.get(&name) {
            writer
                .write_all(data)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        } else if let Some(text) = text_overrides.get(&name) {
            writer
                .write_all(text.as_bytes())
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        } else {
            let declared_size = entry.size();
            copy_zip_entry_limited(
                &mut entry,
                &mut writer,
                &name,
                declared_size,
                MAX_ZIP_PART_UNCOMPRESSED_BYTES,
            )?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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
}
