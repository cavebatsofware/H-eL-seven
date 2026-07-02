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

//! hl7gen - generate HL7 v2 test messages from the definition snapshots.
//!
//! Usage:
//!   hl7gen [--count N] [--seed S] [--version V] [--events A,B,C] [--messy P]
//!          [--defs DIR] [--report]
//!
//! Messages go to stdout (CR segment separators, one message per line group).
//! --report writes one line per message to stderr: event + injected defects.
//! Without --seed a fresh seed is created and printed to stderr so the run
//! can be reproduced.

use hl7_engine::defs::Definitions;
use hl7_gen::{Config, Generator};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut count: u64 = 10;
    let mut seed: Option<u64> = None;
    let mut version = String::from("2.5.1");
    let mut events: Option<Vec<String>> = None;
    let mut messy: f64 = 0.0;
    let mut defs_dir = String::from("defs");
    let mut report = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut take = |what: &str| {
            args.next()
                .ok_or_else(|| format!("{what} requires a value"))
        };
        match arg.as_str() {
            "--count" => {
                match take("--count").and_then(|v| v.parse().map_err(|e| format!("{e}"))) {
                    Ok(v) => count = v,
                    Err(e) => return usage(&e),
                }
            }
            "--seed" => match take("--seed").and_then(|v| v.parse().map_err(|e| format!("{e}"))) {
                Ok(v) => seed = Some(v),
                Err(e) => return usage(&e),
            },
            "--version" => match take("--version") {
                Ok(v) => version = v,
                Err(e) => return usage(&e),
            },
            "--events" => match take("--events") {
                Ok(v) => events = Some(v.split(',').map(str::to_string).collect()),
                Err(e) => return usage(&e),
            },
            "--messy" => {
                match take("--messy").and_then(|v| v.parse().map_err(|e| format!("{e}"))) {
                    Ok(v) => messy = v,
                    Err(e) => return usage(&e),
                }
            }
            "--defs" => match take("--defs") {
                Ok(v) => defs_dir = v,
                Err(e) => return usage(&e),
            },
            "--report" => report = true,
            "--help" | "-h" => return usage(""),
            other => return usage(&format!("unknown argument {other}")),
        }
    }

    let path = format!("{defs_dir}/hl7-{version}.json");
    let defs = match std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|b| Definitions::from_json(&b).map_err(|e| e.to_string()))
    {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hl7gen: cannot load {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let events = events.unwrap_or_else(|| Generator::default_events(&defs));
    let unknown: Vec<&String> = events
        .iter()
        .filter(|e| !defs.events.contains_key(*e))
        .collect();
    if !unknown.is_empty() {
        eprintln!("hl7gen: events not in HL7 v{version}: {unknown:?}");
        return ExitCode::FAILURE;
    }
    if events.is_empty() {
        eprintln!("hl7gen: no events to generate");
        return ExitCode::FAILURE;
    }

    // No --seed -> create one from the clock and report it, so any run can
    // be reproduced.
    let seed = seed.unwrap_or_else(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let seed = now.as_secs() ^ u64::from(now.subsec_nanos()).wrapping_mul(0x9E37_79B9);
        eprintln!("hl7gen: seed {seed} (pass --seed {seed} to reproduce)");
        seed
    });

    let mut generator = Generator::new(
        &defs,
        seed,
        Config {
            messy,
            ..Config::default()
        },
    );

    for i in 0..count {
        let event = &events[(i as usize) % events.len()];
        let gen = generator.generate(event).expect("event checked above");
        println!("{}", gen.text);
        if report {
            eprintln!(
                "#{:06} {} {}",
                i + 1,
                gen.event,
                if gen.defects.is_empty() {
                    "clean".to_string()
                } else {
                    format!("defects: {}", gen.defects.join("; "))
                }
            );
        }
    }
    ExitCode::SUCCESS
}

fn usage(err: &str) -> ExitCode {
    if !err.is_empty() {
        eprintln!("hl7gen: {err}");
    }
    eprintln!(
        "usage: hl7gen [--count N] [--seed S] [--version V] [--events A,B,C] [--messy P] [--defs DIR] [--report]"
    );
    if err.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
