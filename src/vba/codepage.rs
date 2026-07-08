const WINDOWS_1252_EXTENSION: [Option<char>; 32] = [
    Some('\u{20AC}'),
    None,
    Some('\u{201A}'),
    Some('\u{0192}'),
    Some('\u{201E}'),
    Some('\u{2026}'),
    Some('\u{2020}'),
    Some('\u{2021}'),
    Some('\u{02C6}'),
    Some('\u{2030}'),
    Some('\u{0160}'),
    Some('\u{2039}'),
    Some('\u{0152}'),
    None,
    Some('\u{017D}'),
    None,
    None,
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{201C}'),
    Some('\u{201D}'),
    Some('\u{2022}'),
    Some('\u{2013}'),
    Some('\u{2014}'),
    Some('\u{02DC}'),
    Some('\u{2122}'),
    Some('\u{0161}'),
    Some('\u{203A}'),
    Some('\u{0153}'),
    None,
    Some('\u{017E}'),
    Some('\u{0178}'),
];

pub(crate) fn decode_windows_1252(data: &[u8]) -> String {
    data.iter()
        .map(|byte| decode_windows_1252_byte(*byte))
        .collect()
}

fn decode_windows_1252_byte(byte: u8) -> char {
    match byte {
        0x80..=0x9F => WINDOWS_1252_EXTENSION[usize::from(byte - 0x80)].unwrap_or('\u{FFFD}'),
        _ => char::from(byte),
    }
}

pub(crate) fn encode_windows_1252_char(ch: char) -> Option<u8> {
    let value = u32::from(ch);
    if value <= 0x7F || (0xA0..=0xFF).contains(&value) {
        return u8::try_from(value).ok();
    }
    WINDOWS_1252_EXTENSION
        .iter()
        .position(|candidate| *candidate == Some(ch))
        .and_then(|index| u8::try_from(0x80 + index).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_1252_decodes_defined_extension_bytes() {
        assert_eq!(
            decode_windows_1252(&[0x80, 0x91, 0x92, 0x96, 0x97, 0x9F]),
            "\u{20AC}\u{2018}\u{2019}\u{2013}\u{2014}\u{0178}"
        );
    }

    #[test]
    fn windows_1252_replaces_undefined_extension_bytes() {
        assert_eq!(
            decode_windows_1252(&[0x81, 0x8D, 0x8F, 0x90, 0x9D]),
            "\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}"
        );
    }

    #[test]
    fn windows_1252_encodes_defined_extension_chars() {
        let chars = [
            ('\u{20AC}', 0x80),
            ('\u{2018}', 0x91),
            ('\u{2019}', 0x92),
            ('\u{2013}', 0x96),
            ('\u{2014}', 0x97),
            ('\u{0178}', 0x9F),
        ];
        for (ch, byte) in chars {
            assert_eq!(encode_windows_1252_char(ch), Some(byte));
        }
    }

    #[test]
    fn windows_1252_rejects_c1_controls() {
        assert_eq!(encode_windows_1252_char('\u{0081}'), None);
    }
}
