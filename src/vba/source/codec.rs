use sha2::{Digest, Sha256};

use crate::{CliError, CliResult};
pub(super) fn encode_module_source(
    source: &[u8],
    code_page: i32,
) -> CliResult<(Vec<u8>, Vec<String>)> {
    let mut text = normalize_vba_line_endings(&String::from_utf8_lossy(source));
    let mut warnings = Vec::new();
    if !text.ends_with("\r\n") {
        text.push_str("\r\n");
        warnings.push("appended trailing CRLF to VBA source".to_string());
    }
    if code_page == 65001 {
        return Ok((text.into_bytes(), warnings));
    }
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        if u32::from(ch) > 0xFF {
            return Err(CliError::invalid_args(format!(
                "VBA source contains character {ch:?} that cannot be encoded with code page {code_page}"
            )));
        }
        out.push(ch as u8);
    }
    Ok((out, warnings))
}

pub(super) fn normalize_vba_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

pub(super) fn source_sha256(encoded_source: &[u8], code_page: i32) -> String {
    let decoded = decode_module_source(encoded_source, code_page);
    let mut hasher = Sha256::new();
    hasher.update(decoded.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(super) fn compress_container_literals(mut raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x01];
    while raw.len() >= 4096 {
        let header = 0x3000_u16 | 0x0FFF;
        out.extend(header.to_le_bytes());
        out.extend_from_slice(&raw[..4096]);
        raw = &raw[4096..];
    }
    if raw.is_empty() {
        return out;
    }
    while !raw.is_empty() {
        let literal_len = raw.len().min(3600);
        let literal_chunk = &raw[..literal_len];
        let mut chunk = Vec::new();
        let mut offset = 0;
        while offset < literal_chunk.len() {
            let n = (literal_chunk.len() - offset).min(8);
            chunk.push(0x00);
            chunk.extend_from_slice(&literal_chunk[offset..offset + n]);
            offset += n;
        }
        let header = ((chunk.len() - 1) as u16) | 0x3000 | 0x8000;
        out.extend(header.to_le_bytes());
        out.extend(chunk);
        raw = &raw[literal_len..];
    }
    out
}

pub(super) fn decompress_container(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("compressed container is empty".to_string());
    }
    if data[0] != 0x01 {
        return Err(format!(
            "compressed container signature 0x{:02x}, want 0x01",
            data[0]
        ));
    }
    let mut out = Vec::new();
    let mut pos = 1;
    while pos < data.len() {
        if pos + 2 > data.len() {
            return Err("truncated compressed chunk header".to_string());
        }
        let header = u16::from_le_bytes([data[pos], data[pos + 1]]);
        if header == 0 {
            break;
        }
        if header & 0x7000 != 0x3000 {
            return Err(format!(
                "invalid compressed chunk signature in header 0x{header:04x}"
            ));
        }
        let chunk_size = usize::from(header & 0x0FFF) + 3;
        let chunk_end = pos + chunk_size;
        if chunk_end > data.len() {
            return Err("compressed chunk exceeds stream size".to_string());
        }
        let compressed = header & 0x8000 != 0;
        let chunk_data = &data[pos + 2..chunk_end];
        let chunk_start = out.len();
        if !compressed {
            if chunk_data.len() != 4096 {
                return Err(format!(
                    "raw compressed chunk has {} bytes, want 4096",
                    chunk_data.len()
                ));
            }
            out.extend_from_slice(chunk_data);
        } else {
            decompress_chunk(chunk_data, chunk_start, &mut out)?;
        }
        pos = chunk_end;
    }
    Ok(out)
}

pub(super) fn decompress_chunk(
    data: &[u8],
    chunk_start: usize,
    out: &mut Vec<u8>,
) -> Result<(), String> {
    let mut pos = 0;
    while pos < data.len() {
        let flags = data[pos];
        pos += 1;
        for bit in 0..8 {
            if pos >= data.len() {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(data[pos]);
                pos += 1;
                continue;
            }
            if pos + 2 > data.len() {
                return Err("truncated copy token".to_string());
            }
            let token = u16::from_le_bytes([data[pos], data[pos + 1]]);
            pos += 2;
            let (offset, length) = unpack_copy_token(token, out.len() - chunk_start);
            if offset > out.len() || out.len() - offset < chunk_start {
                return Err(format!(
                    "copy token offset {offset} precedes decompressed chunk"
                ));
            }
            let copy_start = out.len() - offset;
            for i in 0..length {
                out.push(out[copy_start + i]);
            }
        }
    }
    Ok(())
}

pub(super) fn unpack_copy_token(token: u16, difference: usize) -> (usize, usize) {
    let mut bit_count = 4;
    let mut limit = 16;
    while difference > limit && bit_count < 12 {
        bit_count += 1;
        limit <<= 1;
    }
    let length_bits = 16 - bit_count;
    let length_mask = (1_u16 << length_bits) - 1;
    let length = usize::from(token & length_mask) + 3;
    let offset = usize::from(token >> length_bits) + 1;
    (offset, length)
}

pub(super) fn decode_module_source(data: &[u8], code_page: i32) -> String {
    let end = data
        .iter()
        .rposition(|value| *value != 0)
        .map(|idx| idx + 1)
        .unwrap_or(0);
    decode_mbcs(&data[..end], code_page)
}

pub(super) fn decode_mbcs(data: &[u8], code_page: i32) -> String {
    if data.is_empty() {
        return String::new();
    }
    if code_page == 65001 {
        return String::from_utf8_lossy(data).into_owned();
    }
    data.iter().map(|value| char::from(*value)).collect()
}

pub(super) fn decode_utf16_le(data: &[u8]) -> String {
    let mut units = Vec::with_capacity(data.len() / 2);
    for chunk in data.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        if value == 0 {
            break;
        }
        units.push(value);
    }
    String::from_utf16_lossy(&units)
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

pub(super) fn utf16le_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
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
    fn literal_compressor_roundtrips_through_real_decompressor() {
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

    #[test]
    fn decompressor_rejects_truncated_or_invalid_containers() {
        assert!(decompress_container(&[]).unwrap_err().contains("empty"));
        assert!(
            decompress_container(&[0x00])
                .unwrap_err()
                .contains("signature")
        );
        assert!(
            decompress_container(&[0x01, 0x00])
                .unwrap_err()
                .contains("truncated")
        );
        assert!(
            decompress_container(&[0x01, 0x00, 0x20])
                .unwrap_err()
                .contains("invalid compressed chunk signature")
        );
    }
}
