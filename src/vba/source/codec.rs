use sha2::{Digest, Sha256};

use crate::{CliError, CliResult};

pub(super) use super::super::codec::{
    compress_container_literals, decode_mbcs, decode_module_source, decode_utf16_le,
    decompress_container, utf16le_bytes,
};

pub(super) fn encode_module_source(
    source: &[u8],
    code_page: i32,
) -> CliResult<(Vec<u8>, Vec<String>)> {
    super::super::codec::encode_module_source(source, code_page).map_err(CliError::invalid_args)
}

pub(super) fn source_sha256(encoded_source: &[u8], code_page: i32) -> String {
    let decoded = decode_module_source(encoded_source, code_page);
    let mut hasher = Sha256::new();
    hasher.update(decoded.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(super) fn count_source_lines(source: &str) -> usize {
    if source.is_empty() {
        return 0;
    }
    let lines = source.matches('\n').count();
    if source.ends_with('\n') {
        lines
    } else {
        lines + 1
    }
}

pub(super) fn source_line_ending_style(source: &str) -> &'static str {
    let mut has_crlf = false;
    let mut has_lf = false;
    let mut has_cr = false;
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if index + 1 < bytes.len() && bytes[index + 1] == b'\n' => {
                has_crlf = true;
                index += 2;
                continue;
            }
            b'\r' => has_cr = true,
            b'\n' => has_lf = true,
            _ => {}
        }
        index += 1;
    }
    let kinds = [has_crlf, has_lf, has_cr]
        .into_iter()
        .filter(|present| *present)
        .count();
    match (kinds, has_crlf, has_lf) {
        (0, _, _) => "none",
        (2.., _, _) => "mixed",
        (_, true, _) => "crlf",
        (_, _, true) => "lf",
        _ => "cr",
    }
}

pub(super) fn source_has_trailing_line_ending(source: &str) -> bool {
    source.ends_with('\n') || source.ends_with('\r')
}

pub(super) fn extension_for_module_kind(kind: &str) -> &'static str {
    match kind {
        "class" => ".cls",
        "userform" => ".frm",
        _ => ".bas",
    }
}

pub(super) fn read_u16(data: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| "truncated VBA dir stream".to_string())?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

pub(super) fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| "truncated VBA dir stream".to_string())?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_boundary_uses_shared_literal_compressor() {
        let small = b"Attribute VB_Name = \"Module1\"\r\nSub Hi()\r\nEnd Sub\r\n".to_vec();
        let large = (0..9000)
            .map(|idx| b'A' + (idx % 26) as u8)
            .collect::<Vec<_>>();

        for raw in [&small[..], &large[..]] {
            let compressed = compress_container_literals(raw);
            assert_eq!(compressed[0], 0x01);
            assert_eq!(decompress_container(&compressed).unwrap(), raw);
        }
    }
}
