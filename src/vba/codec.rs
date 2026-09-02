const MAX_DECOMPRESSED_CONTAINER_BYTES: usize = 256 * 1024 * 1024;
const MAX_DECOMPRESSED_CHUNK_BYTES: usize = 4096;

const WINDOWS_1252_CODE_PAGE: i32 = 1252;
const UTF8_CODE_PAGE: i32 = 65001;

const WINDOWS_1252_C1: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

pub(super) fn encode_module_source(
    source: &[u8],
    code_page: i32,
) -> Result<(Vec<u8>, Vec<String>), String> {
    let source = std::str::from_utf8(source)
        .map_err(|err| format!("VBA source is not valid UTF-8: {err}"))?;
    let mut text = normalize_vba_line_endings(source);
    let mut warnings = Vec::new();
    if !text.ends_with("\r\n") {
        text.push_str("\r\n");
        warnings.push("appended trailing CRLF to VBA source".to_string());
    }
    let encoded = match code_page {
        WINDOWS_1252_CODE_PAGE => encode_windows_1252(&text)?,
        UTF8_CODE_PAGE => text.into_bytes(),
        _ => encode_legacy_single_byte(&text, code_page)?,
    };
    Ok((encoded, warnings))
}

pub(super) fn encode_windows_1252(text: &str) -> Result<Vec<u8>, String> {
    text.chars()
        .map(|ch| {
            let value = u32::from(ch);
            if value <= 0x7F || (0xA0..=0xFF).contains(&value) {
                return Ok(value as u8);
            }
            WINDOWS_1252_C1
                .iter()
                .position(|candidate| *candidate == ch)
                .map(|index| 0x80 + index as u8)
                .ok_or_else(|| windows_1252_encoding_error(ch))
        })
        .collect()
}

pub(super) fn decode_windows_1252(data: &[u8]) -> String {
    data.iter()
        .map(|byte| match *byte {
            0x80..=0x9F => WINDOWS_1252_C1[usize::from(*byte - 0x80)],
            _ => char::from(*byte),
        })
        .collect()
}

fn windows_1252_encoding_error(ch: char) -> String {
    format!(
        "VBA source contains character {ch:?} (U+{:04X}) that cannot be encoded as Windows-1252 (code page 1252)",
        u32::from(ch)
    )
}

fn encode_legacy_single_byte(text: &str, code_page: i32) -> Result<Vec<u8>, String> {
    text.chars()
        .map(|ch| {
            if u32::from(ch) > 0xFF {
                return Err(format!(
                    "VBA source contains character {ch:?} that cannot be encoded with code page {code_page}"
                ));
            }
            Ok(ch as u8)
        })
        .collect()
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
        return out;
    }
    append_literal_chunk(&mut out, raw);
    out
}

fn append_raw_chunk(out: &mut Vec<u8>, raw: &[u8]) {
    debug_assert!(!raw.is_empty());
    debug_assert!(raw.len() <= MAX_DECOMPRESSED_CHUNK_BYTES);
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

pub(super) fn decompress_container(data: &[u8]) -> Result<Vec<u8>, String> {
    decompress_container_with_limit(data, MAX_DECOMPRESSED_CONTAINER_BYTES)
}

fn decompress_container_with_limit(data: &[u8], max_output_len: usize) -> Result<Vec<u8>, String> {
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
        let chunk_end = pos
            .checked_add(chunk_size)
            .ok_or_else(|| "compressed chunk exceeds stream size".to_string())?;
        if chunk_end > data.len() {
            return Err("compressed chunk exceeds stream size".to_string());
        }
        let compressed = header & 0x8000 != 0;
        let chunk_data = &data[pos + 2..chunk_end];
        let chunk_start = out.len();
        if !compressed {
            if chunk_data.len() > MAX_DECOMPRESSED_CHUNK_BYTES {
                return Err(format!(
                    "raw compressed chunk has {} bytes, limit 4096",
                    chunk_data.len()
                ));
            }
            ensure_decompressed_capacity(out.len(), chunk_data.len(), max_output_len)?;
            out.extend_from_slice(chunk_data);
        } else {
            decompress_chunk(chunk_data, chunk_start, &mut out, max_output_len)?;
        }
        pos = chunk_end;
    }
    Ok(out)
}

pub(super) fn decompress_chunk(
    data: &[u8],
    chunk_start: usize,
    out: &mut Vec<u8>,
    max_output_len: usize,
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
                push_decompressed_byte(out, chunk_start, data[pos], max_output_len)?;
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
                push_decompressed_byte(out, chunk_start, out[copy_start + i], max_output_len)?;
            }
        }
    }
    Ok(())
}

fn push_decompressed_byte(
    out: &mut Vec<u8>,
    chunk_start: usize,
    value: u8,
    max_output_len: usize,
) -> Result<(), String> {
    if out.len().saturating_sub(chunk_start) >= MAX_DECOMPRESSED_CHUNK_BYTES {
        return Err(format!(
            "decompressed VBA chunk exceeds {MAX_DECOMPRESSED_CHUNK_BYTES}-byte limit"
        ));
    }
    ensure_decompressed_capacity(out.len(), 1, max_output_len)?;
    out.push(value);
    Ok(())
}

fn ensure_decompressed_capacity(
    current_len: usize,
    additional_len: usize,
    max_output_len: usize,
) -> Result<(), String> {
    let Some(next_len) = current_len.checked_add(additional_len) else {
        return Err("VBA compressed container decompressed size overflows usize".to_string());
    };
    if next_len > max_output_len {
        return Err(format!(
            "VBA compressed container exceeds decompressed size limit ({next_len} > {max_output_len} bytes)"
        ));
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
    if code_page == UTF8_CODE_PAGE {
        return String::from_utf8_lossy(data).into_owned();
    }
    if code_page == WINDOWS_1252_CODE_PAGE {
        return decode_windows_1252(data);
    }
    data.iter().map(|value| char::from(*value)).collect()
}

pub(super) fn decode_utf16_le(data: &[u8]) -> String {
    let mut units = Vec::with_capacity(data.len() / 2);
    for chunk in data.as_chunks::<2>().0 {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        if value == 0 {
            break;
        }
        units.push(value);
    }
    String::from_utf16_lossy(&units)
}

pub(super) fn utf16le_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
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
    fn windows_1252_roundtrips_every_byte() {
        let bytes = (u8::MIN..=u8::MAX).collect::<Vec<_>>();
        let decoded = decode_windows_1252(&bytes);

        assert_eq!(decoded.chars().count(), 256);
        assert_eq!(encode_windows_1252(&decoded).unwrap(), bytes);
    }

    #[test]
    fn cp1252_decode_and_encode_cover_extension_bytes() {
        assert_eq!(
            decode_mbcs(&[0x80, 0x91, 0x92, 0x96, 0x97], 1252),
            "\u{20AC}\u{2018}\u{2019}\u{2013}\u{2014}"
        );

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
    fn cp1252_encode_preserves_undefined_c1_controls() {
        assert_eq!(
            encode_windows_1252("\u{0081}\u{008D}\u{008F}\u{0090}\u{009D}").unwrap(),
            [0x81, 0x8D, 0x8F, 0x90, 0x9D]
        );
    }

    #[test]
    fn cp1252_refusal_names_character_codepoint_and_encoding() {
        let error = encode_windows_1252("\u{03B1}").unwrap_err();
        assert_eq!(
            error,
            "VBA source contains character '\u{3b1}' (U+03B1) that cannot be encoded as Windows-1252 (code page 1252)"
        );
    }

    #[test]
    fn literal_compressor_never_emits_short_non_terminal_chunks() {
        for len in [3601, 4000, 4095, 4096, 4097, 8000] {
            let raw = vec![b'Z'; len];
            let compressed = compress_container_literals(&raw);
            let chunks = decompressed_chunk_lengths(&compressed);
            if chunks.len() > 1 {
                for chunk_len in &chunks[..chunks.len() - 1] {
                    assert_eq!(*chunk_len, 4096, "len {len}: chunks {chunks:?}");
                }
            }
            assert_eq!(decompress_container(&compressed).unwrap(), raw);
        }
    }

    #[test]
    fn decompressor_enforces_output_size_limit() {
        let compressed = compress_container_literals(b"12345678");
        let error = decompress_container_with_limit(&compressed, 7)
            .expect_err("limit should reject eighth byte");
        assert!(error.contains("decompressed size limit"));
    }

    #[test]
    fn decompressor_rejects_chunks_over_4096_bytes() {
        let mut compressed = vec![0x01];
        let payload = [0b0000_0010, b'A', 0xFE, 0x0F];
        let header = ((payload.len() - 1) as u16) | 0x3000 | 0x8000;
        compressed.extend(header.to_le_bytes());
        compressed.extend(payload);

        let error = decompress_container(&compressed).expect_err("chunk should exceed limit");
        assert!(error.contains("chunk exceeds 4096-byte limit"));
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

    fn decompressed_chunk_lengths(data: &[u8]) -> Vec<usize> {
        assert_eq!(data[0], 0x01);
        let mut pos = 1;
        let mut lengths = Vec::new();
        while pos < data.len() {
            let header = u16::from_le_bytes([data[pos], data[pos + 1]]);
            let chunk_size = usize::from(header & 0x0FFF) + 3;
            let chunk_end = pos + chunk_size;
            let before = decompress_container(&data[..pos]).unwrap_or_default().len();
            let after = decompress_container(&data[..chunk_end]).unwrap().len();
            lengths.push(after - before);
            pos = chunk_end;
        }
        lengths
    }
}
