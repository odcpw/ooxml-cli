use super::{VbaAuthoringError, VbaAuthoringResult};

pub(super) use super::super::codec::{
    compress_container_literals, normalize_vba_line_endings, utf16le_bytes,
};

pub(super) fn encode_module_source(
    source: &[u8],
    code_page: u16,
) -> VbaAuthoringResult<(Vec<u8>, Vec<String>)> {
    if code_page != 1252 {
        return Err(VbaAuthoringError::invalid_model(
            "pure VBA authoring currently supports only Windows-1252 code page 1252",
        ));
    }
    super::super::codec::encode_module_source(source, i32::from(code_page))
        .map_err(VbaAuthoringError::invalid_model)
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
    fn module_source_preserves_undefined_windows_1252_controls() {
        let (encoded, _) = encode_module_source(
            "Sub Hi()\n    MsgBox \"\u{0081}\"\nEnd Sub".as_bytes(),
            1252,
        )
        .expect("undefined CP1252 control should round-trip");
        assert!(encoded.contains(&0x81));
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
