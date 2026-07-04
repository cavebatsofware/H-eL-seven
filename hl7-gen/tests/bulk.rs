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

//! Bulk test: generated messages driven through the full engine.
//!
//! Clean generation must translate with zero error-severity issues; messages
//! with injected defects must always produce at least one issue; and nothing,
//! ever, panics.

use hl7_engine::defs::Definitions;
use hl7_engine::{syntax, Engine};
use hl7_gen::{Config, Generator};

fn defs(version: &str) -> Definitions {
    let path = format!(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../defs/hl7-{}.json"),
        version
    );
    Definitions::from_json(&std::fs::read(path).unwrap()).unwrap()
}

fn engine() -> Engine {
    Engine::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../defs")).unwrap()
}

#[test]
fn clean_messages_translate_without_errors() {
    let engine = engine();
    for version in ["2.3.1", "2.5.1", "2.8"] {
        let defs = defs(version);
        let events = Generator::default_events(&defs);
        assert!(!events.is_empty(), "no default events for {version}");
        let mut generator = Generator::new(&defs, 0xC1EA4, Config::default());

        for i in 0..300 {
            let event = &events[i % events.len()];
            let gen = generator.generate(event).unwrap();
            let doc = engine
                .translate(&gen.text)
                .unwrap_or_else(|e| panic!("{version}/{event} #{i}: {e}\n{}", gen.text));

            assert_eq!(
                doc["_meta"]["event"].as_str(),
                Some(event.as_str()),
                "{version} #{i}: wrong event resolved\n{}",
                gen.text
            );
            let errors: Vec<&serde_json::Value> = doc["issues"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|i| i["severity"] == "error")
                .collect();
            assert!(
                errors.is_empty(),
                "{version}/{event} #{i} clean message produced errors: {errors:#?}\n{}",
                gen.text
            );
        }
    }
}

#[test]
fn defective_messages_always_produce_issues() {
    let engine = engine();
    let defs = defs("2.5.1");
    let events = Generator::default_events(&defs);
    let mut generator = Generator::new(
        &defs,
        0xDEFEC7,
        Config {
            messy: 1.0,
            ..Config::default()
        },
    );

    let mut with_defects = 0;
    for i in 0..300 {
        let event = &events[i % events.len()];
        let gen = generator.generate(event).unwrap();
        let doc = engine.translate(&gen.text).unwrap();
        if gen.defects.is_empty() {
            continue;
        }
        with_defects += 1;
        // A defect must surface as an error or warning. "At least one issue" is
        // too weak: clean high-quality messages already carry info-level notes
        // (length-over-spec, Z-segment preserved), so an injected defect that
        // produced only an info issue would be indistinguishable from clean and
        // is not actually detectable.
        let issues = doc["issues"].as_array().unwrap();
        let detectable = issues
            .iter()
            .any(|is| matches!(is["severity"].as_str(), Some("error") | Some("warning")));
        assert!(
            detectable,
            "#{i} {event} defects {:?} produced no error/warning issue\n{}\nissues: {issues:#?}",
            gen.defects, gen.text
        );
    }
    // messy=1.0 should manage to inject into the vast majority of messages.
    assert!(with_defects > 200, "only {with_defects}/300 got defects");
}

#[test]
fn messy_generation_is_deterministic() {
    let defs = defs("2.5.1");
    let events = Generator::default_events(&defs);
    let run = || {
        let mut g = Generator::new(
            &defs,
            0xBADF00D,
            Config {
                messy: 1.0,
                ..Config::default()
            },
        );
        (0..300)
            .map(|i| {
                let gen = g.generate(&events[i % events.len()]).unwrap();
                (gen.text, gen.defects)
            })
            .collect::<Vec<_>>()
    };
    // Same seed must reproduce both the messages and the injected defects
    // exactly, so a --report line always describes the message it sits beside.
    assert_eq!(run(), run(), "messy generation is not reproducible for a seed");
}

/// Regression: the "fields beyond the segment definition" defect must not
/// target a Z-segment. A Z-segment has no definition to exceed, so appended
/// fields are kept as lossless positional data with only an info note, leaving
/// the reported defect undetectable. Forcing a Z-segment onto every message
/// makes that interaction common, so absorption must be exactly zero.
#[test]
fn injected_defects_survive_z_segments() {
    let engine = engine();
    let defs = defs("2.5.1");
    let events = Generator::default_events(&defs);
    let mut generator = Generator::new(
        &defs,
        0xDEFEC7,
        Config {
            messy: 1.0,
            z_segment_p: 1.0,
            ..Config::default()
        },
    );
    for i in 0..500 {
        let gen = generator.generate(&events[i % events.len()]).unwrap();
        if gen.defects.is_empty() {
            continue;
        }
        let doc = engine.translate(&gen.text).unwrap();
        let detectable = doc["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|is| matches!(is["severity"].as_str(), Some("error") | Some("warning")));
        assert!(
            detectable,
            "#{i} defects {:?} absorbed (no error/warning) despite Z-segment\n{}",
            gen.defects, gen.text
        );
    }
}

#[test]
fn generated_messages_round_trip_losslessly() {
    let defs = defs("2.5.1");
    let events = Generator::default_events(&defs);
    let mut generator = Generator::new(&defs, 0x105514, Config::default());
    for i in 0..200 {
        let gen = generator.generate(&events[i % events.len()]).unwrap();
        let text = gen.text.trim_end_matches('\r');
        let parsed = syntax::parse(text).unwrap();
        assert_eq!(syntax::render(&parsed), text, "#{i} not lossless");
    }
}
