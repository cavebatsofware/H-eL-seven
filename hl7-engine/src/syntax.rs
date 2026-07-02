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

//! Layer-1 syntactic parser: raw HL7 text -> zero-copy positional tree.
//!
//! HL7 v2 syntax is a fixed four-level delimiter hierarchy - segments split on
//! CR, fields on `|`, repetitions on `~`, components on `^`, subcomponents on
//! `&` - with the actual delimiter characters defined by the message itself in
//! MSH-1/MSH-2. This layer knows nothing about message semantics: every value
//! is a `&str` slice into the input, escape sequences are left encoded (see
//! `escape`), and nothing here ever rejects a structurally odd message beyond
//! the bare minimum needed to read the delimiters.

/// The five encoding characters, as declared by MSH-1/MSH-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delimiters {
    pub field: char,
    pub component: char,
    pub repetition: char,
    pub escape: char,
    pub subcomponent: char,
    /// Truncation character (HL7 2.7+, fifth char of MSH-2), if declared.
    pub truncation: Option<char>,
}

impl Default for Delimiters {
    fn default() -> Self {
        Delimiters {
            field: '|',
            component: '^',
            repetition: '~',
            escape: '\\',
            subcomponent: '&',
            truncation: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    /// The message does not begin with an MSH segment, so the delimiters are unknowable.
    NoMsh,
    /// MSH is too short to declare its delimiters.
    TruncatedMsh,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty message"),
            ParseError::NoMsh => write!(f, "message does not start with MSH"),
            ParseError::TruncatedMsh => write!(f, "MSH segment too short to declare delimiters"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug)]
pub struct RawMessage<'m> {
    pub delims: Delimiters,
    pub segments: Vec<RawSegment<'m>>,
}

#[derive(Debug)]
pub struct RawSegment<'m> {
    pub id: &'m str,
    /// fields[0] is HL7 field 1 (e.g. PID-1). For MSH, fields[0] is the field
    /// separator itself and fields[1] the raw encoding characters, per the spec.
    pub fields: Vec<RawField<'m>>,
}

#[derive(Debug)]
pub struct RawField<'m> {
    pub repeats: Vec<RawRepeat<'m>>,
}

#[derive(Debug)]
pub struct RawRepeat<'m> {
    pub components: Vec<RawComponent<'m>>,
}

#[derive(Debug)]
pub struct RawComponent<'m> {
    /// Leaf values, still escape-encoded.
    pub subcomponents: Vec<&'m str>,
}

impl<'m> RawSegment<'m> {
    /// 1-based HL7 field access (PID-3 -> `field(3)`).
    pub fn field(&self, n: usize) -> Option<&RawField<'m>> {
        n.checked_sub(1).and_then(|i| self.fields.get(i))
    }
}

impl<'m> RawField<'m> {
    /// First repetition / first component / first subcomponent - the value of
    /// a field when treated as a simple one.
    pub fn first(&self) -> &'m str {
        self.repeats
            .first()
            .and_then(|r| r.components.first())
            .and_then(|c| c.subcomponents.first())
            .copied()
            .unwrap_or("")
    }

    fn simple(value: &'m str) -> Self {
        RawField {
            repeats: vec![RawRepeat {
                components: vec![RawComponent {
                    subcomponents: vec![value],
                }],
            }],
        }
    }
}

/// Parse one HL7 message. Accepts `\r`, `\n`, or `\r\n` as segment separators
/// (the spec says `\r`; the wild says otherwise).
pub fn parse(input: &str) -> Result<RawMessage<'_>, ParseError> {
    // Trim only segment separators: spaces and tabs can be significant data.
    let input = input.trim_matches(|c| c == '\r' || c == '\n');
    if input.is_empty() {
        return Err(ParseError::Empty);
    }
    if !input.starts_with("MSH") {
        return Err(ParseError::NoMsh);
    }

    let delims = read_delimiters(input)?;

    let mut segments = Vec::new();
    for line in input.split(['\r', '\n']) {
        if line.is_empty() {
            continue;
        }
        segments.push(parse_segment(line, &delims));
    }
    Ok(RawMessage { delims, segments })
}

/// MSH-1 is the byte after "MSH" and *is* the field separator; MSH-2 is the
/// run of encoding characters ending at the next field separator.
fn read_delimiters(input: &str) -> Result<Delimiters, ParseError> {
    let mut chars = input[3..].chars();
    let field = chars.next().ok_or(ParseError::TruncatedMsh)?;
    let enc: Vec<char> = chars.take_while(|&c| c != field).take(5).collect();
    // Encoding characters beyond the first are technically required, but apply
    // defaults for lenient handling of short MSH-2 values.
    let d = Delimiters::default();
    Ok(Delimiters {
        field,
        component: enc.first().copied().unwrap_or(d.component),
        repetition: enc.get(1).copied().unwrap_or(d.repetition),
        escape: enc.get(2).copied().unwrap_or(d.escape),
        subcomponent: enc.get(3).copied().unwrap_or(d.subcomponent),
        truncation: enc.get(4).copied(),
    })
}

fn parse_segment<'m>(line: &'m str, delims: &Delimiters) -> RawSegment<'m> {
    let mut tokens = line.split(delims.field);
    let id = tokens.next().unwrap_or("");

    let mut fields: Vec<RawField<'m>> = Vec::new();
    if id == "MSH" {
        // MSH-1 (the separator itself) and MSH-2 (encoding characters) are
        // field *values* that happen to be made of delimiters; they must not
        // be split further or escape-decoded. A bare "MSH" line (no separator
        // at all - only possible as a stray repeat) has no MSH-1/MSH-2.
        let sep_len = delims.field.len_utf8();
        if line.len() >= 3 + sep_len {
            fields.push(RawField::simple(&line[3..3 + sep_len]));
            if let Some(enc) = tokens.next() {
                fields.push(RawField::simple(enc));
            }
        }
    }
    for tok in tokens {
        fields.push(parse_field(tok, delims));
    }
    RawSegment { id, fields }
}

/// Re-serialize a parsed message to pipe-delimited text (CR-separated).
/// For any message parsed from CR-separated input, `render(parse(x)) == x`
/// byte-for-byte - the parse is lossless.
pub fn render(msg: &RawMessage<'_>) -> String {
    let d = &msg.delims;
    let mut out = String::new();
    for seg in &msg.segments {
        if !out.is_empty() {
            out.push('\r');
        }
        out.push_str(seg.id);
        let mut fields = seg.fields.iter();
        if seg.id == "MSH" {
            // MSH-1 is the field separator itself; MSH-2 follows it directly.
            fields.next();
            if let Some(enc) = fields.next() {
                out.push(d.field);
                out.push_str(enc.first());
            }
        }
        for field in fields {
            out.push(d.field);
            let mut first_rep = true;
            for rep in &field.repeats {
                if !first_rep {
                    out.push(d.repetition);
                }
                first_rep = false;
                let mut first_comp = true;
                for comp in &rep.components {
                    if !first_comp {
                        out.push(d.component);
                    }
                    first_comp = false;
                    let mut first_sub = true;
                    for sub in &comp.subcomponents {
                        if !first_sub {
                            out.push(d.subcomponent);
                        }
                        first_sub = false;
                        out.push_str(sub);
                    }
                }
            }
        }
    }
    out
}

fn parse_field<'m>(tok: &'m str, delims: &Delimiters) -> RawField<'m> {
    RawField {
        repeats: tok
            .split(delims.repetition)
            .map(|rep| RawRepeat {
                components: rep
                    .split(delims.component)
                    .map(|comp| RawComponent {
                        subcomponents: comp.split(delims.subcomponent).collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADT: &str = "MSH|^~\\&|SENDAPP|SENDFAC|RCVAPP|RCVFAC|20240102030405||ADT^A01^ADT_A01|MSG00001|P|2.5.1\rEVN|A01|20240102030405\rPID|1||12345^^^HOSP^MR~67890^^^HOSP^SS||SMITH^JOHN^Q||19800101|M|||123 MAIN ST^APT 4^METROPOLIS^NY^10001\r";

    #[test]
    fn parses_segments_and_ids() {
        let msg = parse(ADT).unwrap();
        let ids: Vec<&str> = msg.segments.iter().map(|s| s.id).collect();
        assert_eq!(ids, ["MSH", "EVN", "PID"]);
    }

    #[test]
    fn msh_1_and_2_are_the_delimiters() {
        let msg = parse(ADT).unwrap();
        let msh = &msg.segments[0];
        assert_eq!(msh.field(1).unwrap().first(), "|");
        assert_eq!(msh.field(2).unwrap().first(), "^~\\&");
        assert_eq!(
            msh.field(9).unwrap().repeats[0].components[2].subcomponents[0],
            "ADT_A01"
        );
        assert_eq!(msh.field(12).unwrap().first(), "2.5.1");
    }

    #[test]
    fn repetitions_components_subcomponents() {
        let msg = parse(ADT).unwrap();
        let pid = &msg.segments[2];
        let ids = pid.field(3).unwrap();
        assert_eq!(ids.repeats.len(), 2);
        assert_eq!(ids.repeats[0].components[0].subcomponents[0], "12345");
        assert_eq!(ids.repeats[1].components[4].subcomponents[0], "SS");
        assert_eq!(
            pid.field(5).unwrap().repeats[0].components[1].subcomponents[0],
            "JOHN"
        );
    }

    #[test]
    fn nonstandard_delimiters() {
        let msg = parse("MSH#*+'!#SEND#FAC\rPID#1##A*B+C'D!E").unwrap();
        assert_eq!(msg.delims.field, '#');
        assert_eq!(msg.delims.component, '*');
        assert_eq!(msg.delims.repetition, '+');
        assert_eq!(msg.delims.escape, '\'');
        assert_eq!(msg.delims.subcomponent, '!');
        let pid = &msg.segments[1];
        let f3 = pid.field(3).unwrap();
        assert_eq!(f3.repeats.len(), 2); // A*B + C'D!E
        assert_eq!(f3.repeats[0].components[1].subcomponents[0], "B");
        assert_eq!(f3.repeats[1].components[0].subcomponents[1], "E");
    }

    #[test]
    fn truncation_char_in_27() {
        let msg = parse("MSH|^~\\&#|APP\r").unwrap();
        assert_eq!(msg.delims.truncation, Some('#'));
        assert_eq!(msg.segments[0].field(2).unwrap().first(), "^~\\&#");
        assert_eq!(msg.segments[0].field(3).unwrap().first(), "APP");
    }

    #[test]
    fn errors_are_errors_not_panics() {
        assert_eq!(parse("").unwrap_err(), ParseError::Empty);
        assert_eq!(parse("PID|1").unwrap_err(), ParseError::NoMsh);
        assert_eq!(parse("MSH").unwrap_err(), ParseError::TruncatedMsh);
        // Bare MSH + separator only: MSH-1 is the separator, MSH-2 is empty.
        let msg = parse("MSH|").unwrap();
        assert_eq!(msg.segments[0].fields.len(), 2);
        assert_eq!(msg.segments[0].field(1).unwrap().first(), "|");
        assert_eq!(msg.segments[0].field(2).unwrap().first(), "");
    }

    #[test]
    fn accepts_lf_and_crlf() {
        for sep in ["\n", "\r\n"] {
            let text = format!("MSH|^~\\&|A{sep}PID|1{sep}");
            let msg = parse(&text).unwrap();
            assert_eq!(msg.segments.len(), 2);
        }
    }

    #[test]
    fn empty_trailing_fields_preserved() {
        let msg = parse("MSH|^~\\&|A||C||\r").unwrap();
        // MSH-1..MSH-7: |, ^~\&, A, , C, , (trailing empty kept)
        assert_eq!(msg.segments[0].fields.len(), 7);
        assert_eq!(msg.segments[0].field(4).unwrap().first(), "");
    }
}
