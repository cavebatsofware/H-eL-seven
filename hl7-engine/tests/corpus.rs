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

//! End-to-end corpus, losslessness, and panic-safety tests.

use hl7_engine::{syntax, Engine};
use serde_json::Value;

fn engine() -> Engine {
    Engine::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../defs")).expect("defs snapshots")
}

fn path<'v>(doc: &'v Value, path: &str) -> &'v Value {
    let mut cur = doc;
    for part in path.split('.') {
        cur = match part.parse::<usize>() {
            Ok(i) => &cur[i],
            Err(_) => &cur[part],
        };
    }
    cur
}

const ADT: &str = "MSH|^~\\&|REG|GH|RIS|GH|20240315103000||ADT^A01^ADT_A01|CTRL-9|P|2.5.1\r\
EVN|A01|20240315103000\r\
PID|1||MRN-7^^^GH^MR||DOE^JANE^^^^^L||19751120|F|||42 OAK AVE^^SPRINGFIELD^IL^62704\r\
PV1|1|I|ICU^2^B|E|||1234^HOUSE^GREGORY|||MED||||7|||1234^HOUSE^GREGORY|I|V-1\r";

const ORU: &str = "MSH|^~\\&|LAB|GH|EHR|GH|20240102030405||ORU^R01^ORU_R01|MSG1|P|2.5.1\r\
PID|1||12345^^^GH^MR||SMITH^JOHN\r\
OBR|1|ORD1||24331-1^Lipid Panel^LN\r\
OBX|1|NM|2093-3^Cholesterol^LN||187|mg/dL^^UCUM|<200|N|||F\r\
NTE|1||Patient fasted 12 hours.\r\
OBX|2|NM|2571-8^Triglyceride^LN||150|mg/dL^^UCUM|<150|N|||F\r";

#[test]
fn adt_a01_translates_with_no_errors() {
    let doc = engine().translate(ADT).unwrap();
    assert_eq!(path(&doc, "_meta.event"), "ADT_A01");
    assert_eq!(path(&doc, "message.PID.patientName.0.givenName"), "JANE");
    assert_eq!(path(&doc, "message.PID.dateTimeOfBirth.time"), "1975-11-20");
    assert_eq!(path(&doc, "message.PV1.patientClass"), "I");
    assert_eq!(
        path(&doc, "message.PV1.assignedPatientLocation.pointOfCare"),
        "ICU"
    );
    let issues = doc["issues"].as_array().unwrap();
    assert!(
        issues.iter().all(|i| i["severity"] != "error"),
        "{issues:#?}"
    );
}

#[test]
fn oru_r01_groups_notes_and_numbers() {
    let doc = engine().translate(ORU).unwrap();
    let oo = path(&doc, "message.patientResult.0.orderObservation.0");
    // Two OBSERVATION instances; the NTE attaches to the first (after OBX 1).
    assert_eq!(oo["observation"].as_array().unwrap().len(), 2);
    assert_eq!(
        path(oo, "observation.0.OBX.observationValue.0"),
        &Value::from(187.0)
    );
    assert_eq!(
        path(oo, "observation.0.NTE.0.comment.0"),
        "Patient fasted 12 hours."
    );
    assert_eq!(path(oo, "observation.1.OBX.setIdObx"), 2);
}

#[test]
fn explicit_null_and_escapes() {
    let text = "MSH|^~\\&|A|F|B|F|20240101||ADT^A01^ADT_A01|1|P|2.5.1\r\
EVN|A01|20240101\r\
PID|1||M1^^^GH^MR||\"\"||19800101|M|||1 A\\S\\B ST^^X\\F\\Y\r\
PV1|1|O\r";
    let doc = engine().translate(text).unwrap();
    // Explicit HL7 null ("") -> JSON null.
    assert_eq!(path(&doc, "message.PID.patientName.0"), &Value::Null);
    // Escapes decode inside leaves.
    assert_eq!(
        path(
            &doc,
            "message.PID.patientAddress.0.streetAddress.streetOrMailingAddress"
        ),
        "1 A^B ST"
    );
    assert_eq!(path(&doc, "message.PID.patientAddress.0.city"), "X|Y");
}

#[test]
fn seeded_errors_are_reported_exactly() {
    // PID-8 "Q" is not in HL7 table 0001? (0001 is User-typed -> NOT validated.)
    // OBX-2 (table 0125, HL7-typed) gets "XX" -> warning. OBX-11 missing -> error.
    // OBX-3 CE is fine; OBX-5 NM "abc" -> warning.
    let text = "MSH|^~\\&|LAB|GH|EHR|GH|20240101||ORU^R01^ORU_R01|1|P|2.5.1\r\
PID|1||M1^^^GH^MR||S^J||19800101|Q\r\
OBR|1|O1||X^Y^LN\r\
OBX|1|XX|2093-3^Chol^LN||abc|||||||\r";
    let doc = engine().translate(text).unwrap();
    let issues: Vec<(String, String)> = doc["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["severity"].as_str().unwrap().to_string(),
                i["message"].as_str().unwrap().to_string(),
            )
        })
        .collect();

    let has = |sev: &str, frag: &str| issues.iter().any(|(s, m)| s == sev && m.contains(frag));
    assert!(has("warning", "not in HL7 table 0125"), "{issues:?}");
    assert!(has("error", "required field 11"), "{issues:?}");
    // User-typed table 0001 must NOT produce a warning for "Q".
    assert!(
        !issues.iter().any(|(_, m)| m.contains("table 0001")),
        "{issues:?}"
    );
    // OBX-5 with unknown OBX-2 type falls back to VARIES -> no NM complaint.
    assert_eq!(
        path(
            &doc,
            "message.patientResult.0.orderObservation.0.observation.0.OBX.observationValue.0"
        ),
        "abc"
    );
}

#[test]
fn version_fallback_warns() {
    let text = "MSH|^~\\&|A|F|B|F|20240101||ADT^A01^ADT_A01|1|P|2.9\rEVN|A01|20240101\rPID|1||M^^^G^MR\rPV1|1|I\r";
    let doc = engine().translate(text).unwrap();
    assert_eq!(path(&doc, "_meta.definitionsVersion"), "2.8");
    assert!(doc["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["message"].as_str().unwrap().contains("2.9")));
}

// ---------- losslessness ----------

/// Tiny deterministic PRNG (xorshift) so the property test needs no deps.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() % items.len() as u64) as usize]
    }
    fn range(&mut self, max: u64) -> u64 {
        self.next() % max
    }
}

#[test]
fn parse_render_round_trip_is_byte_identical() {
    let seg_ids = ["PID", "PV1", "OBX", "NTE", "ZZZ", "OBR", "IN1"];
    let chars = [
        "A", "b", "3", " ", "", "\\X\\", "&&", "~", "^^", "..", "\"\"",
    ];
    let mut rng = Rng(0x5EED);

    for case in 0..500 {
        let mut msg = String::from("MSH|^~\\&|APP|FAC|B|F|20240101||ADT^A01^ADT_A01|1|P|2.5.1");
        for _ in 0..rng.range(6) {
            msg.push('\r');
            msg.push_str(rng.pick(&seg_ids));
            for _ in 0..rng.range(8) {
                msg.push('|');
                // Random field content assembled from delimiter-free atoms
                // joined by real delimiters at random.
                for _ in 0..rng.range(5) {
                    msg.push_str(rng.pick(&chars));
                    msg.push_str(rng.pick(&["", "^", "~", "&"]));
                }
            }
        }
        let parsed = syntax::parse(&msg).unwrap_or_else(|e| panic!("case {case}: {e}\n{msg:?}"));
        let rendered = syntax::render(&parsed);
        assert_eq!(rendered, msg, "case {case} not lossless");
    }
}

// ---------- panic safety ----------

#[test]
fn hostile_inputs_never_panic() {
    let engine = engine();
    let fixed: &[&str] = &[
        "",
        "M",
        "MSH",
        "MSH|",
        "MSH|^",
        "MSH|^~\\&",
        "MSH|^~\\&|",
        "MSH||||||",
        "MSHMSHMSH",
        "MSH|^~\\&|A\rMSH",
        "MSH|^~\\&|A\rMSH|^~\\&|B",
        "MSH|^~\\&|A\r\r\r\rPID",
        "MSH|^~\\&|A|B|C|D|E||ADT^A01|1|P|banana",
        "MSH|^~\\&|A|B|C|D|E||^^^^^^^|1|P|2.5.1",
        "MSH|^~\\&\rPID|~~~~~|^^^^^|&&&&&",
        "MSH|^~\\&\rOBX|1|ZZ|x||~~^^&&\\|
",
        "MSH#^~\\&#A\rPID#1",
        "MSH|^~\\&|\u{0}\u{ff}\rPID|\u{7f}",
    ];
    for (i, text) in fixed.iter().enumerate() {
        let _ = engine.translate(text); // must return, Ok or Err - never panic
        let _ = i;
    }

    // Randomized garbage, biased toward HL7-ish bytes.
    let mut rng = Rng(0xBADC0DE);
    let alphabet: Vec<char> = "MSHPIDOBX|^~\\&\r\n.0123456789\"Z\u{e9}".chars().collect();
    for _ in 0..2000 {
        let len = rng.range(120) as usize;
        let mut s = String::with_capacity(len + 3);
        if rng.range(2) == 0 {
            s.push_str("MSH");
        }
        for _ in 0..len {
            s.push(*rng.pick(&alphabet));
        }
        let _ = engine.translate(&s);
    }
}
