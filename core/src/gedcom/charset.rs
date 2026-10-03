//! Character-set detection, decoding and encoding for GEDCOM files.

use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charset {
    Utf8,
    Utf16Le,
    Utf16Be,
    Ansel,
    Ascii,
    Latin1,
    Cp1252,
}

/// Decode raw bytes. Returns text and the charset used. `warn` receives non-fatal notes.
pub fn decode(bytes: &[u8], warn: &mut dyn FnMut(String)) -> (String, Charset) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (
            String::from_utf8_lossy(&bytes[3..]).into_owned(),
            Charset::Utf8,
        );
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return (utf16(&bytes[2..], true), Charset::Utf16Le);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return (utf16(&bytes[2..], false), Charset::Utf16Be);
    }
    // BOM-less UTF-16: "0 HEAD" with interleaved NULs
    if bytes.len() >= 4 && bytes[0] == b'0' && bytes[1] == 0 {
        return (utf16(bytes, true), Charset::Utf16Le);
    }
    if bytes.len() >= 4 && bytes[0] == 0 && bytes[1] == b'0' {
        return (utf16(bytes, false), Charset::Utf16Be);
    }
    let declared = declared_charset(bytes);
    match declared.as_deref() {
        Some("ANSEL") => return (decode_ansel(bytes), Charset::Ansel),
        Some("ANSI") | Some("WINDOWS-1252") | Some("CP1252") | Some("WINDOWS") => {
            return (decode_cp1252(bytes), Charset::Cp1252)
        }
        Some("ISO-8859-1") | Some("ISO8859-1") | Some("LATIN1") | Some("IBMPC") => {
            return (bytes.iter().map(|&b| b as char).collect(), Charset::Latin1)
        }
        _ => {}
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => (
            s.to_string(),
            if s.is_ascii() {
                Charset::Ascii
            } else {
                Charset::Utf8
            },
        ),
        Err(_) => {
            warn(
                "file is not valid UTF-8 and declares no known charset; decoded as Windows-1252"
                    .into(),
            );
            (decode_cp1252(bytes), Charset::Cp1252)
        }
    }
}

fn declared_charset(bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(4096)];
    let text: String = head.iter().map(|&b| b as char).collect();
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("1 CHAR") {
            return Some(rest.trim().to_ascii_uppercase());
        }
    }
    None
}

fn utf16(bytes: &[u8], le: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if le {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
];

fn decode_cp1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x80..0xA0).contains(&b) {
                CP1252_HIGH[(b - 0x80) as usize]
            } else {
                b as char
            }
        })
        .collect()
}

fn ansel_special(b: u8) -> Option<char> {
    Some(match b {
        0xA1 => 'Ł',
        0xA2 => 'Ø',
        0xA3 => 'Đ',
        0xA4 => 'Þ',
        0xA5 => 'Æ',
        0xA6 => 'Œ',
        0xA7 => 'ʹ',
        0xA8 => '·',
        0xA9 => '♭',
        0xAA => '®',
        0xAB => '±',
        0xAC => 'Ơ',
        0xAD => 'Ư',
        0xAE => 'ʼ',
        0xB0 => 'ʻ',
        0xB1 => 'ł',
        0xB2 => 'ø',
        0xB3 => 'đ',
        0xB4 => 'þ',
        0xB5 => 'æ',
        0xB6 => 'œ',
        0xB7 => 'ʺ',
        0xB8 => 'ı',
        0xB9 => '£',
        0xBA => 'ð',
        0xBC => 'ơ',
        0xBD => 'ư',
        0xC0 => '°',
        0xC1 => 'ℓ',
        0xC2 => '℗',
        0xC3 => '©',
        0xC4 => '♯',
        0xC5 => '¿',
        0xC6 => '¡',
        0xCF => 'ß',
        _ => return None,
    })
}

fn ansel_combining(b: u8) -> Option<char> {
    Some(match b {
        0xE0 => '\u{0309}',
        0xE1 => '\u{0300}',
        0xE2 => '\u{0301}',
        0xE3 => '\u{0302}',
        0xE4 => '\u{0303}',
        0xE5 => '\u{0304}',
        0xE6 => '\u{0306}',
        0xE7 => '\u{0307}',
        0xE8 => '\u{0308}',
        0xE9 => '\u{030C}',
        0xEA => '\u{030A}',
        0xEE => '\u{030B}',
        0xF0 => '\u{0327}',
        0xF1 => '\u{0328}',
        0xF2 => '\u{0323}',
        0xF3 => '\u{0324}',
        0xF4 => '\u{0325}',
        0xF6 => '\u{0332}',
        _ => return None,
    })
}

/// ANSEL puts combining marks *before* their base letter; Unicode puts them after.
fn decode_ansel(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut pending: Vec<char> = Vec::new();
    for &b in bytes {
        if let Some(m) = ansel_combining(b) {
            pending.push(m);
            continue;
        }
        let c = if b < 0x80 {
            b as char
        } else {
            ansel_special(b).unwrap_or('\u{FFFD}')
        };
        out.push(c);
        if !pending.is_empty() && c != '\n' && c != '\r' {
            out.extend(pending.drain(..));
        } else {
            pending.clear();
        }
    }
    out.nfc().collect()
}

/// Encode text for output. Unmappable characters become `?` for single-byte sets.
pub fn encode(text: &str, cs: Charset) -> Vec<u8> {
    match cs {
        Charset::Utf8 | Charset::Ansel | Charset::Cp1252 => text.as_bytes().to_vec(),
        Charset::Utf16Le => {
            let mut v = vec![0xFF, 0xFE];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_le_bytes());
            }
            v
        }
        Charset::Utf16Be => {
            let mut v = vec![0xFE, 0xFF];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_be_bytes());
            }
            v
        }
        Charset::Latin1 => text
            .chars()
            .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' })
            .collect(),
        Charset::Ascii => text
            .chars()
            .map(|c| if c.is_ascii() { c as u8 } else { fold_ascii(c) })
            .collect(),
    }
}

fn fold_ascii(c: char) -> u8 {
    let s: String = c.to_string().nfd().filter(|x| x.is_ascii()).collect();
    s.bytes().next().unwrap_or(b'?')
}

pub fn gedcom_char_name(cs: Charset) -> &'static str {
    match cs {
        Charset::Utf8 => "UTF-8",
        Charset::Utf16Le | Charset::Utf16Be => "UNICODE",
        Charset::Ansel => "ANSEL",
        Charset::Ascii => "ASCII",
        Charset::Latin1 => "ISO-8859-1",
        Charset::Cp1252 => "ANSI",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(b: &[u8]) -> (String, Charset) {
        decode(b, &mut |_| {})
    }

    #[test]
    fn utf8_bom_and_utf16() {
        assert_eq!(dec(b"\xEF\xBB\xBF0 HEAD").0, "0 HEAD");
        let u16le = encode("0 HEAD\n1 NAME Ayşe", Charset::Utf16Le);
        let (t, cs) = dec(&u16le);
        assert_eq!(t, "0 HEAD\n1 NAME Ayşe");
        assert_eq!(cs, Charset::Utf16Le);
        let be = encode("0 HEAD é", Charset::Utf16Be);
        assert_eq!(dec(&be).0, "0 HEAD é");
    }

    #[test]
    fn ansel_diacritics_reorder() {
        // ANSEL acute (E2) precedes base letter; "Müller" with diaeresis E8, "José" with acute E2
        let b = b"1 CHAR ANSEL\n1 NAME M\xE8uller Jos\xE2e \xA5 \xB1\n";
        let (t, cs) = dec(b);
        assert_eq!(cs, Charset::Ansel);
        assert!(t.contains("Müller"));
        assert!(t.contains("José"));
        assert!(t.contains("Æ ł"));
    }

    #[test]
    fn latin1_and_cp1252_fallback() {
        let b = b"1 CHAR ISO-8859-1\n1 NAME J\xF6rg";
        assert!(dec(b).0.contains("Jörg"));
        let mut warned = false;
        let (t, cs) = decode(b"1 NAME Caf\xE9 \x93q\x94", &mut |_| warned = true);
        assert!(warned);
        assert_eq!(cs, Charset::Cp1252);
        assert!(t.contains("Café “q”"));
    }

    #[test]
    fn encode_lossy_sets() {
        assert_eq!(encode("Şeker", Charset::Ascii), b"Seker");
        assert_eq!(encode("é€", Charset::Latin1), vec![0xE9, b'?']);
    }
}
