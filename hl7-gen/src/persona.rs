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

//! Per-message persona: one coherent patient / visit / facility world rolled
//! at the start of each message, so every segment that mentions the patient,
//! a doctor, a facility, or a timestamp agrees with the rest of the message.
//!
//! Dates are plain (year, month, day) with day capped at 28 so offset
//! arithmetic never needs month-length tables; the base "now" is derived
//! from the RNG, keeping generation fully seed-deterministic.

use crate::data;
use crate::Rng;

#[derive(Clone, Copy)]
pub struct Hl7Date {
    pub y: i64,
    pub m: i64,
    pub d: i64,
}

impl Hl7Date {
    pub fn ymd(&self) -> String {
        format!("{:04}{:02}{:02}", self.y, self.m, self.d)
    }

    /// Subtract days (day stays in 1..=28, so borrowing is simple).
    pub fn minus_days(&self, days: i64) -> Hl7Date {
        let mut y = self.y;
        let mut m = self.m;
        let mut d = self.d - days;
        while d < 1 {
            m -= 1;
            if m < 1 {
                m = 12;
                y -= 1;
            }
            d += 28;
        }
        Hl7Date { y, m, d }
    }

    pub fn minus_years(&self, years: i64) -> Hl7Date {
        Hl7Date {
            y: self.y - years,
            ..*self
        }
    }
}

pub struct Person {
    pub family: String,
    pub given: String,
    pub middle: String,
}

pub struct Doctor {
    pub id: String,
    pub family: String,
    pub given: String,
}

pub struct Persona {
    // Patient.
    pub patient: Person,
    pub sex: &'static str,
    pub dob: Hl7Date,
    pub mrn: String,
    pub account: String,
    pub phone: String,
    pub street: String,
    pub city: &'static str,
    pub state: &'static str,
    pub zip: &'static str,
    /// Next of kin / mother - usually shares the family name.
    pub kin: Person,
    // Providers & facility.
    pub doctors: Vec<Doctor>,
    pub facility_code: &'static str,
    pub facility_name: &'static str,
    pub sending_app: &'static str,
    pub receiving_app: &'static str,
    // Visit.
    pub patient_class: &'static str,
    pub ward: &'static str,
    pub room: String,
    pub bed: String,
    pub visit_number: String,
    pub admit: Hl7Date,
    pub discharge: Hl7Date,
    // Message clock.
    pub now: Hl7Date,
    pub now_time: String,
}

impl Persona {
    pub fn roll(rng: &mut Rng) -> Persona {
        let now = Hl7Date {
            y: 2023 + rng.range(3) as i64,
            m: 1 + rng.range(12) as i64,
            d: 1 + rng.range(28) as i64,
        };
        let sex = *rng.pick(&["M", "F"]);
        let givens = if sex == "M" {
            data::GIVEN_M
        } else {
            data::GIVEN_F
        };
        let family = (*rng.pick(data::SURNAMES)).to_string();
        let patient = Person {
            family: family.clone(),
            given: (*rng.pick(givens)).to_string(),
            middle: (*rng.pick(data::MIDDLE_INITIALS)).to_string(),
        };
        // Kin: opposite-pool given name, same family most of the time.
        let kin_givens = if sex == "M" {
            data::GIVEN_F
        } else {
            data::GIVEN_M
        };
        let kin = Person {
            family: if rng.chance(0.7) {
                family
            } else {
                (*rng.pick(data::SURNAMES)).to_string()
            },
            given: (*rng.pick(kin_givens)).to_string(),
            middle: (*rng.pick(data::MIDDLE_INITIALS)).to_string(),
        };

        let mut doctors: Vec<Doctor> = Vec::new();
        let first = rng.range(data::DOCTORS.len() as u64) as usize;
        for k in 0..2 {
            let (id, fam, giv) = data::DOCTORS[(first + k) % data::DOCTORS.len()];
            doctors.push(Doctor {
                id: id.to_string(),
                family: fam.to_string(),
                given: giv.to_string(),
            });
        }

        let (facility_code, facility_name) = *rng.pick(data::FACILITIES);
        let (city, state, zip) = *rng.pick(data::CITY_STATE_ZIP);
        let admit = now.minus_days(rng.range(20) as i64);
        let discharge = now.minus_days(rng.range(3) as i64); // admit ≤ discharge ≤ now

        Persona {
            sex,
            dob: now
                .minus_years(1 + rng.range(89) as i64)
                .minus_days(rng.range(300) as i64),
            mrn: format!("{}", 1_000_000 + rng.range(9_000_000)),
            account: format!("A{:08}", rng.range(100_000_000)),
            phone: format!("(916)555-{:04}", rng.range(10_000)),
            street: (*rng.pick(data::STREETS)).to_string(),
            city,
            state,
            zip,
            patient,
            kin,
            doctors,
            facility_code,
            facility_name,
            sending_app: rng.pick(data::SENDING_APPS),
            receiving_app: rng.pick(data::RECEIVING_APPS),
            patient_class: *rng.pick(&["I", "O", "E"]),
            ward: rng.pick(data::WARDS),
            room: format!("{}", 100 + rng.range(400)),
            bed: (*rng.pick(&["A", "B"])).to_string(),
            visit_number: format!("V{:06}", rng.range(1_000_000)),
            admit,
            discharge: if discharge.ymd() < admit.ymd() {
                admit
            } else {
                discharge
            },
            now,
            now_time: format!(
                "{:02}{:02}{:02}",
                6 + rng.range(14), // business-ish hours
                rng.range(60),
                rng.range(60)
            ),
        }
    }

    pub fn now_dtm(&self) -> String {
        format!("{}{}", self.now.ymd(), self.now_time)
    }
}
