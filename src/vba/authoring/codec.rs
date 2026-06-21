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
        if u32::from(ch) > 0xFF {
            return Err(VbaAuthoringError::invalid_model(format!(
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
}
