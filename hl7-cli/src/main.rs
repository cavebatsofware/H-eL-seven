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

//! hl7json - translate HL7 v2 messages to JSON.
//!
//! Usage:
//!   hl7json [FILE]            read one or more messages from FILE (default stdin)
//!   hl7json --defs DIR [FILE] load definition snapshots from DIR instead of the embedded ones
//!   hl7json --compact [FILE]  one JSON document per line
//!
//! Multiple messages are accepted in one input: a new message starts at every
//! line beginning with "MSH". Output is a JSON document per message.

use hl7_engine::{defs::Definitions, Engine};
use std::io::Read;
use std::process::ExitCode;

static EMBEDDED: &[(&str, &[u8])] = &[
    ("2.3", include_bytes!("../../defs/hl7-2.3.json")),
    ("2.3.1", include_bytes!("../../defs/hl7-2.3.1.json")),
    ("2.4", include_bytes!("../../defs/hl7-2.4.json")),
    ("2.5", include_bytes!("../../defs/hl7-2.5.json")),
    ("2.5.1", include_bytes!("../../defs/hl7-2.5.1.json")),
    ("2.6", include_bytes!("../../defs/hl7-2.6.json")),
    ("2.7", include_bytes!("../../defs/hl7-2.7.json")),
    ("2.8", include_bytes!("../../defs/hl7-2.8.json")),
];

fn main() -> ExitCode {
    let mut defs_dir: Option<String> = None;
    let mut file: Option<String> = None;
    let mut compact = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--defs" => match args.next() {
                Some(dir) => defs_dir = Some(dir),
                None => return usage("--defs requires a directory"),
            },
            "--compact" => compact = true,
            "--help" | "-h" => return usage(""),
            _ if arg.starts_with('-') => return usage(&format!("unknown flag {arg}")),
            _ if file.is_none() => file = Some(arg),
            _ => return usage("only one input file is supported"),
        }
    }

    let engine = match defs_dir {
        Some(dir) => match Engine::load_dir(&dir) {
            Ok(engine) => engine,
            Err(e) => {
                eprintln!("hl7json: cannot load definitions from {dir}: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => {
            let mut engine = Engine::new();
            for (version, bytes) in EMBEDDED {
                match Definitions::from_json(bytes) {
                    Ok(defs) => engine.add(defs),
                    Err(e) => {
                        eprintln!("hl7json: embedded defs for {version} are corrupt: {e}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            engine
        }
    };

    let mut input = String::new();
    let read = match &file {
        Some(path) => std::fs::read_to_string(path).map(|s| input = s),
        None => std::io::stdin().read_to_string(&mut input).map(|_| ()),
    };
    if let Err(e) = read {
        eprintln!(
            "hl7json: cannot read {}: {e}",
            file.as_deref().unwrap_or("stdin")
        );
        return ExitCode::FAILURE;
    }

    let mut failed = false;
    for message in split_messages(&input) {
        match engine.translate(message) {
            Ok(doc) => {
                let rendered = if compact {
                    serde_json::to_string(&doc)
                } else {
                    serde_json::to_string_pretty(&doc)
                };
                println!("{}", rendered.expect("document serializes"));
            }
            Err(e) => {
                eprintln!("hl7json: {e}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Split concatenated messages: each starts at a line beginning with MSH.
/// MLLP framing bytes (0x0B … 0x1C 0x0D) are stripped if present.
fn split_messages(input: &str) -> Vec<&str> {
    let input = input.trim_matches(|c: char| c == '\u{0b}' || c == '\u{1c}' || c.is_whitespace());
    if input.is_empty() {
        return Vec::new();
    }
    let mut starts: Vec<usize> = Vec::new();
    let mut offset = 0;
    for line in input.split_inclusive(['\r', '\n']) {
        if line
            .trim_start_matches(['\u{0b}', '\u{1c}'])
            .starts_with("MSH")
        {
            starts.push(offset);
        }
        offset += line.len();
    }
    if starts.is_empty() {
        return vec![input];
    }
    let mut out = Vec::new();
    for (i, &start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(input.len());
        let chunk = input[start..end]
            .trim_matches(|c: char| c == '\u{0b}' || c == '\u{1c}' || c.is_whitespace());
        if !chunk.is_empty() {
            out.push(chunk);
        }
    }
    out
}

fn usage(err: &str) -> ExitCode {
    if !err.is_empty() {
        eprintln!("hl7json: {err}");
    }
    eprintln!("usage: hl7json [--defs DIR] [--compact] [FILE]");
    if err.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
