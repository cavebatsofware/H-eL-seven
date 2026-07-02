// H-eL-seven - a schema-aware HL7 v2 to JSON translator
// Copyright (C) 2026 CavebatSoftware LLC - Grant DeFayette
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, version 3.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! HL7 escape sequence decoding, applied to leaf values at decode time.
//!
//! Standard escapes: `\F\` field sep, `\S\` component sep, `\T\` subcomponent
//! sep, `\R\` repetition sep, `\E\` escape char, `\Xdd..\` hex-encoded bytes.
//! Formatting escapes (`\.br\`, `\H\`, `\N\`, …) and anything unrecognized are
//! passed through verbatim - real-world messages contain escapes that were
//! never in any spec, and destroying data is worse than leaving it encoded.

use crate::syntax::Delimiters;
use std::borrow::Cow;

pub fn decode<'a>(value: &'a str, delims: &Delimiters) -> Cow<'a, str> {
    if !value.contains(delims.escape) {
        return Cow::Borrowed(value);
    }

    let esc = delims.escape;
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find(esc) {
        out.push_str(&rest[..start]);
        let after = &rest[start + esc.len_utf8()..];
        match after.find(esc) {
            None => {
                // Unterminated escape: emit verbatim and stop scanning.
                out.push(esc);
                out.push_str(after);
                rest = "";
                break;
            }
            Some(end) => {
                let body = &after[..end];
                match decode_body(body, delims) {
                    Some(decoded) => out.push_str(&decoded),
                    None => {
                        // Unknown escape: pass through verbatim, delimiters included.
                        out.push(esc);
                        out.push_str(body);
                        out.push(esc);
                    }
                }
                rest = &after[end + esc.len_utf8()..];
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

fn decode_body(body: &str, delims: &Delimiters) -> Option<String> {
    match body {
        "F" => Some(delims.field.to_string()),
        "S" => Some(delims.component.to_string()),
        "T" => Some(delims.subcomponent.to_string()),
        "R" => Some(delims.repetition.to_string()),
        "E" => Some(delims.escape.to_string()),
        _ => {
            if let Some(hex) = body.strip_prefix('X') {
                decode_hex(hex)
            } else {
                None
            }
        }
    }
}

/// `\Xdddd\` - pairs of hex digits. Decoded bytes are interpreted as UTF-8
/// when valid, otherwise as Latin-1, so nothing is ever lost to replacement
/// characters for the single-byte encodings seen in practice.
fn decode_hex(hex: &str) -> Option<String> {
    if hex.is_empty() || !hex.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        bytes.push(u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()?);
    }
    match String::from_utf8(bytes) {
        Ok(s) => Some(s),
        Err(e) => Some(e.into_bytes().iter().map(|&b| b as char).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> Delimiters {
        Delimiters::default()
    }

    #[test]
    fn plain_text_is_borrowed() {
        let v = decode("SMITH", &d());
        assert!(matches!(v, Cow::Borrowed("SMITH")));
    }

    #[test]
    fn standard_escapes() {
        assert_eq!(decode(r"A\F\B\S\C\T\D\R\E\E\F", &d()), "A|B^C&D~E\\F");
    }

    #[test]
    fn hex_escape() {
        assert_eq!(decode(r"caf\XC3A9\", &d()), "café");
        assert_eq!(decode(r"\X0D\", &d()), "\r");
        // Latin-1 fallback for non-UTF-8 bytes.
        assert_eq!(decode(r"\XE9\", &d()), "é");
    }

    #[test]
    fn unknown_escapes_pass_through() {
        assert_eq!(decode(r"line1\.br\line2", &d()), r"line1\.br\line2");
        assert_eq!(decode(r"\H\bold\N\", &d()), r"\H\bold\N\");
        assert_eq!(decode(r"\Xzz\", &d()), r"\Xzz\");
        assert_eq!(decode(r"\X1\", &d()), r"\X1\");
    }

    #[test]
    fn unterminated_escape_kept_verbatim() {
        assert_eq!(decode(r"A\Fb", &d()), r"A\Fb");
        assert_eq!(decode(r"trailing\", &d()), r"trailing\");
    }

    #[test]
    fn custom_escape_char() {
        let delims = Delimiters {
            escape: '\'',
            ..Delimiters::default()
        };
        assert_eq!(decode("A'F'B", &delims), "A|B");
    }
}
