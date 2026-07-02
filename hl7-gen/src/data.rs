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

//! Curated pools and small clinical catalogs for realistic-looking values.
//! Everything is picked with the seeded RNG, so output stays reproducible.
//! A few entries deliberately contain HL7 delimiters or quotes ("A & B
//! ASSOCIATES", "O'BRIEN") so escape handling keeps getting exercised.

pub const SURNAMES: &[&str] = &[
    "SMITH",
    "JOHNSON",
    "GARCIA",
    "CHEN",
    "OKAFOR",
    "MULLER",
    "DUBOIS",
    "IVANOV",
    "O'BRIEN",
    "VAN DER BERG",
    "NGUYEN",
    "PATEL",
    "KOWALSKI",
    "HERNANDEZ",
    "KIM",
    "ANDERSON",
    "ROSSI",
];

pub const GIVEN_F: &[&str] = &[
    "MARY", "AISHA", "YUKI", "PRIYA", "ELENA", "GRACE", "MARIA", "CHLOE", "AMARA", "INGRID",
];

pub const GIVEN_M: &[&str] = &[
    "JOHN", "WEI", "CARLOS", "LARS", "DAVID", "AHMED", "MARCUS", "HIROSHI", "PAVEL", "SAMUEL",
];

pub const MIDDLE_INITIALS: &[&str] = &["A", "E", "J", "L", "M", "R", "T"];

pub const STREETS: &[&str] = &[
    "12 OAK AVE",
    "99 ELM ST",
    "4 PINE RD",
    "310 LAKE DR",
    "1500 MAPLE BLVD",
    "27 BIRCH LN",
    "88 CEDAR CT",
    "743 WILLOW WAY",
];

/// Coherent (city, state, zip) triples.
pub const CITY_STATE_ZIP: &[(&str, &str, &str)] = &[
    ("SACRAMENTO", "CA", "95814"),
    ("SPRINGFIELD", "IL", "62701"),
    ("RIVERTON", "UT", "84065"),
    ("LAKEWOOD", "CO", "80226"),
    ("FAIRVIEW", "OR", "97024"),
    ("MADISON", "WI", "53703"),
    ("ROCHESTER", "MN", "55901"),
];

/// (facility code, facility name)
pub const FACILITIES: &[(&str, &str)] = &[
    ("GHH", "GOOD HEALTH HOSPITAL"),
    ("STLUKES", "ST LUKES MEDICAL CENTER"),
    ("CMC", "COUNTY MEDICAL CENTER"),
    ("RVH", "RIVERVIEW HOSPITAL"),
];

pub const SENDING_APPS: &[&str] = &["ADT1", "LABGL1", "REGADT", "PHARMSYS"];
pub const RECEIVING_APPS: &[&str] = &["EHR", "LIS", "BILLSYS", "RADSYS"];

/// (id, family, given) - NPI-shaped ids.
pub const DOCTORS: &[(&str, &str, &str)] = &[
    ("1234567893", "HOUSE", "GREGORY"),
    ("1679576722", "WATSON", "JOAN"),
    ("1497758544", "RAMIREZ", "LUIS"),
    ("1093817465", "OYELARAN", "FUNMI"),
    ("1235186800", "BERGSTROM", "ANNA"),
];

/// Hospital wards for PL point-of-care.
pub const WARDS: &[&str] = &["ICU", "MED", "SURG", "PEDS", "ER", "ONC"];

/// (LOINC-style code, name, units, low, high, integer-valued)
pub const OBSERVATIONS: &[(&str, &str, &str, f64, f64, bool)] = &[
    ("718-7", "Hemoglobin", "g/dL", 12.0, 17.5, false),
    ("2345-7", "Glucose", "mg/dL", 70.0, 110.0, true),
    ("2160-0", "Creatinine", "mg/dL", 0.6, 1.3, false),
    ("2951-2", "Sodium", "mmol/L", 136.0, 145.0, true),
    ("2823-3", "Potassium", "mmol/L", 3.5, 5.1, false),
    ("6690-2", "Leukocytes", "10*3/uL", 4.5, 11.0, false),
    ("777-3", "Platelets", "10*3/uL", 150.0, 400.0, true),
    ("2093-3", "Cholesterol", "mg/dL", 120.0, 200.0, true),
    ("2571-8", "Triglyceride", "mg/dL", 50.0, 150.0, true),
    ("1975-2", "Bilirubin", "mg/dL", 0.2, 1.2, false),
];

/// Text-valued observations for the occasional ST/TX OBX.
pub const TEXT_OBSERVATIONS: &[(&str, &str, &str)] = &[
    ("600-7", "Blood culture", "No growth after 48 hours"),
    ("5778-6", "Urine color", "Yellow"),
    ("32710-6", "Specimen appearance", "Slightly hemolyzed"),
];

/// (ICD-10-style code, description)
pub const DIAGNOSES: &[(&str, &str)] = &[
    ("E11.9", "Type 2 diabetes mellitus without complications"),
    ("I10", "Essential (primary) hypertension"),
    ("J18.9", "Pneumonia, unspecified organism"),
    ("N39.0", "Urinary tract infection, site not specified"),
    (
        "K21.9",
        "Gastro-esophageal reflux disease without esophagitis",
    ),
    ("M54.5", "Low back pain"),
    ("E78.5", "Hyperlipidemia, unspecified"),
    ("J45.909", "Unspecified asthma, uncomplicated"),
];

/// (CPT-style code, description)
pub const PROCEDURES: &[(&str, &str)] = &[
    ("93000", "Electrocardiogram, routine ECG with 12 leads"),
    ("71046", "Radiologic examination, chest; 2 views"),
    ("80053", "Comprehensive metabolic panel"),
    ("36415", "Collection of venous blood by venipuncture"),
    ("45378", "Colonoscopy, flexible; diagnostic"),
];

/// (allergen code-ish, name, reaction)
pub const ALLERGENS: &[(&str, &str, &str)] = &[
    ("PCN", "PENICILLIN", "HIVES"),
    ("SULF", "SULFA DRUGS", "RASH"),
    ("PNUT", "PEANUTS", "ANAPHYLAXIS"),
    ("LTX", "LATEX", "CONTACT DERMATITIS"),
    ("ASA", "ASPIRIN", "WHEEZING"),
];

pub const INSURERS: &[&str] = &[
    "BLUE SHIELD OF CA",
    "AETNA",
    "KAISER PERMANENTE",
    "UNITEDHEALTH",
    "MEDI-CAL",
];

pub const PLAN_NAMES: &[&str] = &["PPO GOLD", "HMO SELECT", "MEDICARE PART B", "EPO BASIC"];

/// Organization-ish names for employers/guarantors; keeps `&`/`'` escape
/// paths exercised.
pub const ORGS: &[&str] = &[
    "ACME LOGISTICS",
    "A & B ASSOCIATES",
    "NORTHSIDE SCHOOL DISTRICT",
    "O'BRIEN FARMS",
    "CITY OF SPRINGFIELD",
    "LAKESIDE RETAIL GROUP",
];

pub const NOTES: &[&str] = &[
    "Patient fasted 12 hours.",
    "Sample slightly hemolyzed",
    "Follow-up in 2 weeks",
    "Ordered by attending physician",
    "Patient reports mild discomfort",
];

/// Neutral fallback words for fields we know nothing about - plausible
/// clinical-admin vocabulary instead of NATO noise.
pub const GENERIC: &[&str] = &[
    "ROUTINE",
    "GENERAL",
    "STANDARD",
    "PRIMARY",
    "MAIN CAMPUS",
    "NORTH WING",
    "OUTPATIENT",
    "REGIONAL",
];
