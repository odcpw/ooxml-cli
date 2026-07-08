use super::super::codepage::encode_windows_1252_char;
use super::{VbaAuthoringError, VbaAuthoringResult};

pub(super) fn encode_module_source(
    source: &[u8],
    code_page: u16,
) -> VbaAuthoringResult<(Vec<u8>, Vec<String>)> {
    if code_page != 1252 {
        return Err(VbaAuthoringError::invalid_model(
            "pure VBA authoring currently supports only Windows-1252 code page 1252",
        ));
    }
    let mut text = normalize_vba_line_endings(&String::from_utf8_lossy(source));
    let mut warnings = Vec::new();
    if !text.ends_with("\r\n") {
        text.push_str("\r\n");
        warnings.push("appended trailing CRLF to VBA source".to_string());
    }

    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let Some(encoded) = encode_windows_1252_char(ch) else {
            return Err(VbaAuthoringError::invalid_model(format!(
                "VBA source contains character {ch:?} that cannot be encoded with code page {code_page}"
            )));
        };
        out.push(encoded);
    }
    Ok((out, warnings))
}

pub(super) fn normalize_vba_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

pub(super) fn compress_container_literals(mut raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x01];
    while raw.len() >= 4096 {
        append_raw_chunk(&mut out, &raw[..4096]);
        raw = &raw[4096..];
    }
    if raw.is_empty() {
        return out;
    }
    if literal_chunk_size(raw.len()) > 4096 {
        append_raw_chunk(&mut out, raw);
    } else {
        append_literal_chunk(&mut out, raw);
    }
    out
}

fn append_raw_chunk(out: &mut Vec<u8>, raw: &[u8]) {
    debug_assert!(!raw.is_empty());
    debug_assert!(raw.len() <= 4096);
    let header = ((raw.len() - 1) as u16) | 0x3000;
    out.extend(header.to_le_bytes());
    out.extend_from_slice(raw);
}

fn append_literal_chunk(out: &mut Vec<u8>, literal_chunk: &[u8]) {
    debug_assert!(literal_chunk_size(literal_chunk.len()) <= 4096);
    let mut chunk = Vec::with_capacity(literal_chunk_size(literal_chunk.len()));
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
}

fn literal_chunk_size(literal_len: usize) -> usize {
    literal_len + literal_len.div_ceil(8)
}

pub(super) fn utf16le_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

#[cfg(test)]
fn decompress_literal_container(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.first() != Some(&0x01) {
        return Err("missing compressed container signature".to_string());
    }
    let mut out = Vec::new();
    let mut pos = 1;
    while pos < data.len() {
        if pos + 2 > data.len() {
            return Err("truncated chunk header".to_string());
        }
        let header = u16::from_le_bytes([data[pos], data[pos + 1]]);
        if header & 0x7000 != 0x3000 {
            return Err("invalid chunk signature".to_string());
        }
        let chunk_size = usize::from(header & 0x0FFF) + 3;
        let chunk_end = pos + chunk_size;
        if chunk_end > data.len() {
            return Err("chunk exceeds stream size".to_string());
        }
        let compressed = header & 0x8000 != 0;
        let payload = &data[pos + 2..chunk_end];
        if !compressed {
            if payload.len() > 4096 {
                return Err("raw chunk exceeds decompressed chunk limit".to_string());
            }
            out.extend_from_slice(payload);
        } else {
            let mut payload_pos = 0;
            while payload_pos < payload.len() {
                let flags = payload[payload_pos];
                payload_pos += 1;
                for bit in 0..8 {
                    if payload_pos >= payload.len() {
                        break;
                    }
                    if flags & (1 << bit) != 0 {
                        return Err(
                            "authoring test decoder supports literal chunks only".to_string()
                        );
                    }
                    out.push(payload[payload_pos]);
                    payload_pos += 1;
                }
            }
        }
        pos = chunk_end;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_source_normalizes_line_endings_and_appends_trailing_crlf() {
        let (encoded, warnings) = encode_module_source(b"Sub Hi()\nEnd Sub", 1252).unwrap();
        assert_eq!(encoded, b"Sub Hi()\r\nEnd Sub\r\n");
        assert_eq!(warnings, vec!["appended trailing CRLF to VBA source"]);
    }

    #[test]
    fn module_source_encodes_windows_1252_extension_chars() {
        let (encoded, warnings) = encode_module_source(
            "Sub Hi()\n    MsgBox \"\u{20AC}\u{2013}\u{2014}\"\nEnd Sub".as_bytes(),
            1252,
        )
        .unwrap();
        assert_eq!(warnings, vec!["appended trailing CRLF to VBA source"]);
        assert!(
            encoded
                .windows(3)
                .any(|window| window == [0x80, 0x96, 0x97])
        );
    }

    #[test]
    fn module_source_rejects_undefined_windows_1252_controls() {
        let error = encode_module_source(
            "Sub Hi()\n    MsgBox \"\u{0081}\"\nEnd Sub".as_bytes(),
            1252,
        )
        .expect_err("undefined CP1252 control should fail");
        assert!(error.message.contains("cannot be encoded"));
    }

    #[test]
    fn literal_compression_roundtrips_small_source() {
        let raw = b"Attribute VB_Name = \"Module1\"\r\nSub Hi()\r\nEnd Sub\r\n";
        let compressed = compress_container_literals(raw);
        assert_eq!(decompress_literal_container(&compressed).unwrap(), raw);
    }

    #[test]
    fn literal_compression_roundtrips_large_source() {
        let raw = vec![b'A'; 5000];
        let compressed = compress_container_literals(&raw);
        assert_eq!(decompress_literal_container(&compressed).unwrap(), raw);
    }

    #[test]
    fn literal_compression_emits_expected_container_and_chunk_headers() {
        let raw = vec![b'X'; 4097];
        let compressed = compress_container_literals(&raw);

        assert_eq!(compressed[0], 0x01, "MS-OVBA compression signature");
        assert_eq!(read_u16_at(&compressed, 1), 0x3FFF);
        assert_eq!(&compressed[3..4099], &raw[..4096]);

        let tail_header_pos = 4099;
        assert_eq!(read_u16_at(&compressed, tail_header_pos), 0xB001);
        assert_eq!(compressed[tail_header_pos + 2], 0x00);
        assert_eq!(compressed[tail_header_pos + 3], b'X');
        assert_eq!(compressed.len(), tail_header_pos + 4);
    }

    #[test]
    fn literal_compression_headers_cover_boundary_lengths() {
        let cases = [
            (0, vec![]),
            (1, vec![0xB001]),
            (8, vec![0xB008]),
            (9, vec![0xB00A]),
            (3600, vec![0xBFD1]),
            (3601, vec![0xBFD3]),
            (3640, vec![0xBFFE]),
            (3641, vec![0x3E38]),
            (4095, vec![0x3FFE]),
            (4096, vec![0x3FFF]),
            (4097, vec![0x3FFF, 0xB001]),
        ];

        for (len, expected_headers) in cases {
            let raw = vec![b'Z'; len];
            let compressed = compress_container_literals(&raw);
            assert_eq!(chunk_headers(&compressed), expected_headers, "len {len}");
            assert_eq!(decompress_literal_container(&compressed).unwrap(), raw);
        }
    }

    #[test]
    fn literal_compression_never_emits_short_non_terminal_chunks() {
        for len in [3601, 4000, 4095, 4096, 4097, 8000] {
            let raw = vec![b'Z'; len];
            let compressed = compress_container_literals(&raw);
            let chunks = decompressed_chunk_lengths(&compressed);
            if chunks.len() > 1 {
                for chunk_len in &chunks[..chunks.len() - 1] {
                    assert_eq!(*chunk_len, 4096, "len {len}: chunks {chunks:?}");
                }
            }
            assert_eq!(decompress_literal_container(&compressed).unwrap(), raw);
        }
    }

    fn chunk_headers(data: &[u8]) -> Vec<u16> {
        assert_eq!(data[0], 0x01);
        let mut pos = 1;
        let mut headers = Vec::new();
        while pos < data.len() {
            let header = read_u16_at(data, pos);
            headers.push(header);
            let chunk_size = usize::from(header & 0x0FFF) + 3;
            pos += chunk_size;
        }
        assert_eq!(pos, data.len());
        headers
    }

    fn read_u16_at(data: &[u8], pos: usize) -> u16 {
        u16::from_le_bytes([data[pos], data[pos + 1]])
    }

    fn decompressed_chunk_lengths(data: &[u8]) -> Vec<usize> {
        assert_eq!(data[0], 0x01);
        let mut pos = 1;
        let mut lengths = Vec::new();
        while pos < data.len() {
            let header = read_u16_at(data, pos);
            let chunk_size = usize::from(header & 0x0FFF) + 3;
            let chunk_end = pos + chunk_size;
            let before = decompress_literal_container(&data[..pos])
                .unwrap_or_default()
                .len();
            let after = decompress_literal_container(&data[..chunk_end])
                .unwrap()
                .len();
            lengths.push(after - before);
            pos = chunk_end;
        }
        lengths
    }
}
