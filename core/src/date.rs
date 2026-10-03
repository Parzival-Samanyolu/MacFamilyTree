//! Structured genealogical date engine: parsing, formatting, calendars, sort keys.
//!
//! Years are *historical* signed years: `-44` is 44 BC and there is no year 0.
//! The sort key is a Julian Day Number, so any calendar maps to one integer axis.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Calendar {
    #[default]
    Gregorian,
    Julian,
    Hebrew,
    French,
    Hijri,
}

impl Calendar {
    pub fn gedcom_escape(self) -> Option<&'static str> {
        match self {
            Calendar::Gregorian => None,
            Calendar::Julian => Some("@#DJULIAN@"),
            Calendar::Hebrew => Some("@#DHEBREW@"),
            Calendar::French => Some("@#DFRENCH R@"),
            Calendar::Hijri => Some("@#DHIJRI@"),
        }
    }
    fn from_escape(s: &str) -> Option<Calendar> {
        match s.to_ascii_uppercase().as_str() {
            "@#DGREGORIAN@" => Some(Calendar::Gregorian),
            "@#DJULIAN@" => Some(Calendar::Julian),
            "@#DHEBREW@" => Some(Calendar::Hebrew),
            "@#DFRENCH R@" => Some(Calendar::French),
            "@#DHIJRI@" | "@#DISLAMIC@" => Some(Calendar::Hijri),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Qualifier {
    Exact,
    About,
    Estimated,
    Calculated,
    Before,
    After,
    Between,
    From,
    To,
    FromTo,
}

/// One calendar point; month/day may be absent (partial dates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Part {
    pub year: i32,
    pub month: Option<u8>,
    pub day: Option<u8>,
    /// Dual-dating alternate year, e.g. `1750/51` stores 1751.
    pub dual_year: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenDate {
    pub calendar: Calendar,
    pub qualifier: Qualifier,
    pub a: Option<Part>,
    pub b: Option<Part>,
    /// Free-text fallback ("in the reign of Victoria"). When set with no `a`, the date is a pure phrase.
    pub phrase: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    Tr,
}

// ---------- calendar arithmetic ----------

fn astro(year: i32) -> i64 {
    if year < 0 {
        year as i64 + 1
    } else {
        year as i64
    }
}
fn unastro(y: i64) -> i32 {
    if y <= 0 {
        (y - 1) as i32
    } else {
        y as i32
    }
}

pub fn gregorian_to_jdn(year: i32, month: u8, day: u8) -> i64 {
    let y = astro(year);
    let a = (14 - month as i64) / 12;
    let y2 = y + 4800 - a;
    let m2 = month as i64 + 12 * a - 3;
    day as i64 + (153 * m2 + 2) / 5 + 365 * y2 + y2.div_euclid(4) - y2.div_euclid(100)
        + y2.div_euclid(400)
        - 32045
}

pub fn julian_to_jdn(year: i32, month: u8, day: u8) -> i64 {
    let y = astro(year);
    let a = (14 - month as i64) / 12;
    let y2 = y + 4800 - a;
    let m2 = month as i64 + 12 * a - 3;
    day as i64 + (153 * m2 + 2) / 5 + 365 * y2 + y2.div_euclid(4) - 32083
}

pub fn jdn_to_gregorian(jdn: i64) -> (i32, u8, u8) {
    let a = jdn + 32044;
    let b = (4 * a + 3).div_euclid(146097);
    let c = a - 146097 * b / 4;
    let d = (4 * c + 3) / 1461;
    let e = c - 1461 * d / 4;
    let m = (5 * e + 2) / 153;
    let day = e - (153 * m + 2) / 5 + 1;
    let month = m + 3 - 12 * (m / 10);
    let year = 100 * b + d - 4800 + m / 10;
    (unastro(year), month as u8, day as u8)
}

pub fn jdn_to_julian(jdn: i64) -> (i32, u8, u8) {
    let c = jdn + 32082;
    let d = (4 * c + 3).div_euclid(1461);
    let e = c - 1461 * d / 4;
    let m = (5 * e + 2) / 153;
    let day = e - (153 * m + 2) / 5 + 1;
    let month = m + 3 - 12 * (m / 10);
    let year = d - 4800 + m / 10;
    (unastro(year), month as u8, day as u8)
}

fn gregorian_leap(y: i32) -> bool {
    let y = astro(y);
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
fn julian_leap(y: i32) -> bool {
    astro(y) % 4 == 0
}

/// Days in a month for Gregorian/Julian; other calendars use fixed approximations (30/29).
pub fn days_in_month(cal: Calendar, year: i32, month: u8) -> u8 {
    match cal {
        Calendar::Gregorian | Calendar::Julian => {
            let leap = if cal == Calendar::Gregorian {
                gregorian_leap(year)
            } else {
                julian_leap(year)
            };
            match month {
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                4 | 6 | 9 | 11 => 30,
                _ => {
                    if leap {
                        29
                    } else {
                        28
                    }
                }
            }
        }
        Calendar::Hijri => {
            if month % 2 == 1 || (month == 12 && hijri_leap(year)) {
                30
            } else {
                29
            }
        }
        Calendar::French => {
            if month == 13 {
                if french_leap(year) {
                    6
                } else {
                    5
                }
            } else {
                30
            }
        }
        Calendar::Hebrew => hebrew_days_in_month(year, month),
    }
}

// Hijri (tabular / civil, epoch Friday 16 Jul 622 Julian)
fn hijri_leap(y: i32) -> bool {
    (14 + 11 * y as i64).rem_euclid(30) < 11
}
const HIJRI_EPOCH: i64 = 1948440;
fn hijri_to_jdn(y: i32, m: u8, d: u8) -> i64 {
    d as i64
        + (29.5 * (m as f64 - 1.0)).ceil() as i64
        + (y as i64 - 1) * 354
        + (3 + 11 * y as i64).div_euclid(30)
        + HIJRI_EPOCH
        - 1
}

// French Republican. Historical sextile years were III, VII and XI; from year XV the
// arithmetic (Romme-style) rule is used. Epoch: 22 Sep 1792 = 1 Vendémiaire An I.
fn french_leap(y: i32) -> bool {
    if y < 15 {
        matches!(y, 3 | 7 | 11)
    } else {
        (y % 4 == 0) && !(y % 100 == 0 && y % 400 != 0)
    }
}
const FRENCH_EPOCH: i64 = 2375840;
fn french_leaps_before(y: i64) -> i64 {
    if y <= 15 {
        [3i64, 7, 11].iter().filter(|&&l| l < y).count() as i64
    } else {
        let n = y - 1;
        n.div_euclid(4) - n.div_euclid(100) + n.div_euclid(400)
    }
}
fn french_to_jdn(y: i32, m: u8, d: u8) -> i64 {
    let y64 = y as i64;
    FRENCH_EPOCH - 1 + 365 * (y64 - 1) + french_leaps_before(y64) + 30 * (m as i64 - 1) + d as i64
}

// Hebrew calendar (Calendrical Calculations, Reingold & Dershowitz)
const HEBREW_EPOCH: i64 = 347998;
fn hebrew_leap(y: i32) -> bool {
    (7 * y as i64 + 1).rem_euclid(19) < 7
}
fn hebrew_delay1(y: i64) -> i64 {
    let months = (235 * y - 234).div_euclid(19);
    let parts = 12084 + 13753 * months;
    let day = months * 29 + parts.div_euclid(25920);
    if (3 * (day + 1)).rem_euclid(7) < 3 {
        day + 1
    } else {
        day
    }
}
fn hebrew_delay2(y: i64) -> i64 {
    let last = hebrew_delay1(y - 1);
    let present = hebrew_delay1(y);
    let next = hebrew_delay1(y + 1);
    if next - present == 356 {
        2
    } else if present - last == 382 {
        1
    } else {
        0
    }
}
fn hebrew_new_year(y: i32) -> i64 {
    HEBREW_EPOCH + hebrew_delay1(y as i64) + hebrew_delay2(y as i64)
}
fn hebrew_year_days(y: i32) -> i64 {
    hebrew_new_year(y + 1) - hebrew_new_year(y)
}
/// Month numbering: 1 = Nisan … 6 = Elul, 7 = Tishri … 12 = Adar (13 = Adar II in leap years).
fn hebrew_days_in_month(y: i32, m: u8) -> u8 {
    let ylen = hebrew_year_days(y);
    match m {
        2 | 4 | 6 | 10 | 13 => 29,
        12 if !hebrew_leap(y) => 29,
        8 if ylen % 10 != 5 => 29,
        9 if ylen % 10 == 3 => 29,
        _ => 30,
    }
}
fn hebrew_to_jdn(y: i32, m: u8, d: u8) -> i64 {
    let mut jdn = hebrew_new_year(y) + d as i64 - 1;
    let months_in_year: u8 = if hebrew_leap(y) { 13 } else { 12 };
    if m < 7 {
        for mm in 7..=months_in_year {
            jdn += hebrew_days_in_month(y, mm) as i64;
        }
        for mm in 1..m {
            jdn += hebrew_days_in_month(y, mm) as i64;
        }
    } else {
        for mm in 7..m {
            jdn += hebrew_days_in_month(y, mm) as i64;
        }
    }
    jdn
}

pub fn to_jdn(cal: Calendar, year: i32, month: u8, day: u8) -> i64 {
    match cal {
        Calendar::Gregorian => gregorian_to_jdn(year, month, day),
        Calendar::Julian => julian_to_jdn(year, month, day),
        Calendar::Hijri => hijri_to_jdn(year, month, day),
        Calendar::French => french_to_jdn(year, month, day),
        Calendar::Hebrew => hebrew_to_jdn(year, month, day),
    }
}

impl Part {
    pub fn ymd(year: i32, month: u8, day: u8) -> Part {
        Part { year, month: Some(month), day: Some(day), dual_year: None }
    }
    pub fn year_only(year: i32) -> Part {
        Part { year, month: None, day: None, dual_year: None }
    }
    /// Earliest and latest JDN covered by this (possibly partial) point.
    pub fn span(&self, cal: Calendar) -> (i64, i64) {
        match (self.month, self.day) {
            (Some(m), Some(d)) => {
                let j = to_jdn(cal, self.year, m, d);
                (j, j)
            }
            (Some(m), None) => (
                to_jdn(cal, self.year, m, 1),
                to_jdn(cal, self.year, m, days_in_month(cal, self.year, m)),
            ),
            _ => {
                let (first, last_m) = match cal {
                    Calendar::Hebrew => (7u8, 6u8),
                    Calendar::Hijri => (1, 12),
                    Calendar::French => (1, 13),
                    _ => (1, 12),
                };
                match cal {
                    Calendar::Hebrew => {
                        // Hebrew civil year runs Tishri(7) .. Elul(6)
                        (hebrew_new_year(self.year), hebrew_new_year(self.year + 1) - 1)
                    }
                    _ => (
                        to_jdn(cal, self.year, first, 1),
                        to_jdn(cal, self.year, last_m, days_in_month(cal, self.year, last_m)),
                    ),
                }
            }
        }
    }
}

// ---------- names of months ----------

const GREG_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const GREG_EN_LONG: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];
const GREG_TR_LONG: [&str; 12] = [
    "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim",
    "Kasım", "Aralık",
];
const HEBREW_G: [&str; 13] = [
    "NSN", "IYR", "SVN", "TMZ", "AAV", "ELL", "TSH", "CSH", "KSL", "TVT", "SHV", "ADR", "ADS",
];
const FRENCH_G: [&str; 13] = [
    "VEND", "BRUM", "FRIM", "NIVO", "PLUV", "VENT", "GERM", "FLOR", "PRAI", "MESS", "THER",
    "FRUC", "COMP",
];
const HIJRI_G: [&str; 12] = [
    "MUHAR", "SAFAR", "RABIA", "RABIT", "JUMAA", "JUMAT", "RAJAB", "SHAAB", "RAMAD", "SHAWW",
    "DHUAQ", "DHUAH",
];

fn month_table(cal: Calendar) -> &'static [&'static str] {
    match cal {
        Calendar::Gregorian | Calendar::Julian => &GREG_EN,
        Calendar::Hebrew => &HEBREW_G,
        Calendar::French => &FRENCH_G,
        Calendar::Hijri => &HIJRI_G,
    }
}

fn normalize_alpha(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ı' | 'İ' | 'I' | 'i' => 'i',
            'ş' | 'Ş' => 's',
            'ğ' | 'Ğ' => 'g',
            'ç' | 'Ç' => 'c',
            'ö' | 'Ö' => 'o',
            'ü' | 'Ü' => 'u',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

fn parse_month(cal: Calendar, tok: &str) -> Option<u8> {
    let t = normalize_alpha(tok.trim_end_matches('.'));
    if t.len() < 3 {
        return None;
    }
    if matches!(cal, Calendar::Gregorian | Calendar::Julian) {
        for i in 0..12 {
            let en = GREG_EN_LONG[i].to_ascii_lowercase();
            let tr = normalize_alpha(GREG_TR_LONG[i]);
            if t == en || t == tr || (t.len() == 3 && en.starts_with(&t)) || (t == "sept" && i == 8) {
                return Some(i as u8 + 1);
            }
        }
        None
    } else {
        month_table(cal)
            .iter()
            .position(|m| m.to_ascii_lowercase() == t)
            .map(|i| i as u8 + 1)
    }
}

// ---------- parsing ----------

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DateError {
    #[error("empty date")]
    Empty,
    #[error("invalid date: {0}")]
    Invalid(String),
}

fn parse_year_token(tok: &str) -> Option<(i32, Option<i32>)> {
    // "1750/51" or "1750/1751" dual year; plain "1850".
    if let Some((y, alt)) = tok.split_once('/') {
        let y: i32 = y.parse().ok()?;
        let a: i32 = alt.parse().ok()?;
        let alt_full = if alt.len() <= 2 { (y / 100) * 100 + a + if a < y % 100 { 100 } else { 0 } } else { a };
        return Some((y, Some(alt_full)));
    }
    if tok.is_empty() || tok.len() > 5 || !tok.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    tok.parse().ok().map(|y| (y, None))
}

fn parse_part(cal: Calendar, toks: &[String]) -> Result<Part, DateError> {
    let err = || DateError::Invalid(toks.join(" "));
    let mut toks: Vec<String> = toks.to_vec();
    let mut bc = false;
    if let Some(last) = toks.last() {
        let l = last.to_ascii_uppercase();
        if l == "BC" || l == "B.C." || l == "BCE" || l == "B.C.E." {
            bc = true;
            toks.pop();
        } else if l == "AD" || l == "A.D." || l == "CE" || l == "C.E." {
            toks.pop();
        }
    }
    let fin = |mut p: Part| {
        if bc {
            p.year = -p.year.abs();
        }
        Ok(p)
    };
    match toks.len() {
        1 => {
            // year, or ISO yyyy-mm-dd, or d.m.yyyy / d/m/yyyy
            let t = &toks[0];
            if let Some((y, alt)) = parse_year_token(t) {
                return fin(Part { year: y, month: None, day: None, dual_year: alt });
            }
            for sep in ['-', '.', '/'] {
                let parts: Vec<&str> = t.split(sep).collect();
                if parts.len() == 3 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty()) {
                    let (y, m, d) = if parts[0].len() == 4 {
                        (parts[0], parts[1], parts[2])
                    } else {
                        (parts[2], parts[1], parts[0])
                    };
                    let (y, m, d): (i32, u8, u8) =
                        (y.parse().map_err(|_| err())?, m.parse().map_err(|_| err())?, d.parse().map_err(|_| err())?);
                    validate(cal, y, Some(m), Some(d)).ok_or_else(err)?;
                    return fin(Part::ymd(y, m, d));
                }
                if parts.len() == 2 && sep == '-' && parts[0].len() == 4 {
                    let y: i32 = parts[0].parse().map_err(|_| err())?;
                    let m: u8 = parts[1].parse().map_err(|_| err())?;
                    validate(cal, y, Some(m), None).ok_or_else(err)?;
                    return fin(Part { year: y, month: Some(m), day: None, dual_year: None });
                }
            }
            Err(err())
        }
        2 => {
            // "MAR 1850" or "1850 MAR"? only month-year supported
            let m = parse_month(cal, &toks[0]).ok_or_else(err)?;
            let (y, alt) = parse_year_token(&toks[1]).ok_or_else(err)?;
            validate(cal, y, Some(m), None).ok_or_else(err)?;
            fin(Part { year: y, month: Some(m), day: None, dual_year: alt })
        }
        3 => {
            // "3 MAR 1850" or "March 3, 1850" / "March 3 1850"
            let (d, m, ytok) = if let Some(m) = parse_month(cal, &toks[1]) {
                (toks[0].as_str(), m, &toks[2])
            } else if let Some(m) = parse_month(cal, &toks[0]) {
                (toks[1].trim_end_matches(',').trim_end_matches("st").trim_end_matches("nd").trim_end_matches("rd").trim_end_matches("th"), m, &toks[2])
            } else {
                return Err(err());
            };
            let d: u8 = d.parse().map_err(|_| err())?;
            let (y, alt) = parse_year_token(ytok).ok_or_else(err)?;
            validate(cal, y, Some(m), Some(d)).ok_or_else(err)?;
            fin(Part { year: y, month: Some(m), day: Some(d), dual_year: alt })
        }
        _ => Err(err()),
    }
}

fn validate(cal: Calendar, y: i32, m: Option<u8>, d: Option<u8>) -> Option<()> {
    if y == 0 {
        return None;
    }
    if let Some(m) = m {
        let max_m = match cal {
            Calendar::French => 13,
            Calendar::Hebrew => 13,
            _ => 12,
        };
        if m == 0 || m > max_m {
            return None;
        }
        if let Some(d) = d {
            if d == 0 || d > days_in_month(cal, y, m) {
                return None;
            }
        }
    }
    Some(())
}

fn tokenize(s: &str) -> Vec<String> {
    s.replace(',', " ").split_whitespace().map(|t| t.to_string()).collect()
}

impl GenDate {
    pub fn exact(p: Part) -> GenDate {
        GenDate { calendar: Calendar::Gregorian, qualifier: Qualifier::Exact, a: Some(p), b: None, phrase: None }
    }
    pub fn phrase(text: &str) -> GenDate {
        GenDate { calendar: Calendar::Gregorian, qualifier: Qualifier::Exact, a: None, b: None, phrase: Some(text.to_string()) }
    }

    /// Parse GEDCOM-style and common human date strings. Falls back to a phrase on failure only via [`GenDate::parse_lenient`].
    pub fn parse(input: &str) -> Result<GenDate, DateError> {
        let s = input.trim();
        if s.is_empty() {
            return Err(DateError::Empty);
        }
        if s.starts_with('(') && s.ends_with(')') {
            return Ok(GenDate::phrase(&s[1..s.len() - 1]));
        }
        let mut toks = tokenize(s);
        let mut cal = Calendar::Gregorian;
        // calendar escape may appear anywhere before the date body, possibly after a keyword.
        let mut take_cal = |toks: &mut Vec<String>| {
            let mut i = 0;
            while i < toks.len() {
                if toks[i].starts_with("@#") {
                    // escape may contain a space ("@#DFRENCH R@")
                    let mut esc = toks[i].clone();
                    let mut n = 1;
                    while !esc.ends_with('@') && i + n < toks.len() {
                        esc.push(' ');
                        esc.push_str(&toks[i + n]);
                        n += 1;
                    }
                    if let Some(c) = Calendar::from_escape(&esc) {
                        cal = c;
                        toks.drain(i..i + n);
                        continue;
                    }
                }
                i += 1;
            }
        };
        take_cal(&mut toks);
        if toks.is_empty() {
            return Err(DateError::Empty);
        }
        let kw = toks[0].to_ascii_uppercase();
        let kw = kw.trim_end_matches('.').to_string();
        let simple = |q: Qualifier, rest: &[String]| -> Result<GenDate, DateError> {
            let p = parse_part(cal, rest)?;
            Ok(GenDate { calendar: cal, qualifier: q, a: Some(p), b: None, phrase: None })
        };
        match kw.as_str() {
            "ABT" | "ABOUT" | "CIRCA" | "C" | "CA" | "YAKLASIK" | "YAKLAŞIK" => simple(Qualifier::About, &toks[1..]),
            "EST" | "ESTIMATED" => simple(Qualifier::Estimated, &toks[1..]),
            "CAL" | "CALCULATED" => simple(Qualifier::Calculated, &toks[1..]),
            "BEF" | "BEFORE" | "ONCE" | "ÖNCE" => simple(Qualifier::Before, &toks[1..]),
            "AFT" | "AFTER" | "SONRA" => simple(Qualifier::After, &toks[1..]),
            "BET" | "BETWEEN" => {
                let rest = &toks[1..];
                let pos = rest
                    .iter()
                    .position(|t| t.eq_ignore_ascii_case("AND"))
                    .ok_or_else(|| DateError::Invalid(s.to_string()))?;
                let a = parse_part(cal, &rest[..pos])?;
                let b = parse_part(cal, &rest[pos + 1..])?;
                Ok(GenDate { calendar: cal, qualifier: Qualifier::Between, a: Some(a), b: Some(b), phrase: None })
            }
            "FROM" => {
                let rest = &toks[1..];
                if let Some(pos) = rest.iter().position(|t| t.eq_ignore_ascii_case("TO")) {
                    let a = parse_part(cal, &rest[..pos])?;
                    let b = parse_part(cal, &rest[pos + 1..])?;
                    Ok(GenDate { calendar: cal, qualifier: Qualifier::FromTo, a: Some(a), b: Some(b), phrase: None })
                } else {
                    simple(Qualifier::From, rest)
                }
            }
            "TO" => simple(Qualifier::To, &toks[1..]),
            _ => simple(Qualifier::Exact, &toks),
        }
    }

    /// Parse, and on failure keep the original text as a phrase (never loses data on import).
    pub fn parse_lenient(input: &str) -> Option<GenDate> {
        let s = input.trim();
        if s.is_empty() {
            return None;
        }
        Some(Self::parse(s).unwrap_or_else(|_| GenDate::phrase(s)))
    }

    /// Earliest and latest JDN covered, if the date is not a pure phrase.
    pub fn range(&self) -> Option<(i64, i64)> {
        let a = self.a?.span(self.calendar);
        Some(match self.qualifier {
            Qualifier::Between | Qualifier::FromTo => {
                let b = self.b?.span(self.calendar);
                (a.0, b.1)
            }
            Qualifier::Before | Qualifier::To => (a.0 - 1, a.0 - 1).min((a.0, a.0)),
            Qualifier::After | Qualifier::From => (a.1 + 1, a.1 + 1).max((a.1, a.1)),
            _ => a,
        })
    }

    /// Single integer used for ordering and indexed range queries (JDN).
    pub fn sort_key(&self) -> Option<i64> {
        let a = self.a?.span(self.calendar);
        Some(match self.qualifier {
            Qualifier::Before | Qualifier::To => a.0 - 1,
            Qualifier::After => a.1 + 1,
            _ => a.0,
        })
    }

    /// Gregorian year used for statistics/age maths (midpoint-agnostic: start year).
    pub fn gregorian_year(&self) -> Option<i32> {
        self.sort_key().map(|k| jdn_to_gregorian(k).0)
    }

    pub fn is_exact_day(&self) -> bool {
        self.qualifier == Qualifier::Exact && matches!(self.a, Some(Part { month: Some(_), day: Some(_), .. }))
    }

    // ---------- formatting ----------

    fn fmt_part_gedcom(&self, p: &Part) -> String {
        let table = month_table(self.calendar);
        let mut out = String::new();
        if let Some(d) = p.day {
            out.push_str(&d.to_string());
            out.push(' ');
        }
        if let Some(m) = p.month {
            out.push_str(&table[(m - 1) as usize].to_ascii_uppercase());
            out.push(' ');
        }
        out.push_str(&abs_year(p));
        if p.year < 0 {
            out.push_str(" BC");
        }
        out
    }

    pub fn to_gedcom(&self) -> String {
        if self.a.is_none() {
            return match &self.phrase {
                Some(p) => format!("({})", p),
                None => String::new(),
            };
        }
        let a = self.a.as_ref().unwrap();
        let body = |q: &str, parts: Vec<String>| -> String {
            let mut s = String::new();
            if let Some(e) = self.calendar.gedcom_escape() {
                s.push_str(e);
                s.push(' ');
            }
            if !q.is_empty() {
                s.push_str(q);
                s.push(' ');
            }
            s.push_str(&parts.join(" "));
            s
        };
        let fa = self.fmt_part_gedcom(a);
        match self.qualifier {
            Qualifier::Exact => body("", vec![fa]),
            Qualifier::About => body("ABT", vec![fa]),
            Qualifier::Estimated => body("EST", vec![fa]),
            Qualifier::Calculated => body("CAL", vec![fa]),
            Qualifier::Before => body("BEF", vec![fa]),
            Qualifier::After => body("AFT", vec![fa]),
            Qualifier::From => body("FROM", vec![fa]),
            Qualifier::To => body("TO", vec![fa]),
            Qualifier::Between => {
                let fb = self.b.as_ref().map(|b| self.fmt_part_gedcom(b)).unwrap_or_default();
                body("BET", vec![fa, "AND".into(), fb])
            }
            Qualifier::FromTo => {
                let fb = self.b.as_ref().map(|b| self.fmt_part_gedcom(b)).unwrap_or_default();
                body("FROM", vec![fa, "TO".into(), fb])
            }
        }
    }

    fn fmt_part_locale(&self, p: &Part, loc: Locale) -> String {
        let mut out = String::new();
        if let Some(d) = p.day {
            out.push_str(&d.to_string());
            out.push(' ');
        }
        if let Some(m) = p.month {
            let name = match (self.calendar, loc) {
                (Calendar::Gregorian | Calendar::Julian, Locale::Tr) => GREG_TR_LONG[(m - 1) as usize],
                (Calendar::Gregorian | Calendar::Julian, Locale::En) => GREG_EN[(m - 1) as usize],
                _ => month_table(self.calendar)[(m - 1) as usize],
            };
            out.push_str(name);
            out.push(' ');
        }
        out.push_str(&abs_year(p));
        if p.year < 0 {
            out.push_str(if loc == Locale::Tr { " MÖ" } else { " BC" });
        }
        out
    }

    /// Human display ("abt 1900", "3 Mar 1850", "yak. 1900").
    pub fn format(&self, loc: Locale) -> String {
        let Some(a) = &self.a else {
            return self.phrase.clone().unwrap_or_default();
        };
        let fa = self.fmt_part_locale(a, loc);
        let fb = self.b.as_ref().map(|b| self.fmt_part_locale(b, loc)).unwrap_or_default();
        let cal = match (self.calendar, loc) {
            (Calendar::Gregorian, _) => "",
            (Calendar::Julian, _) => " (Julian)",
            (Calendar::Hebrew, _) => " (Hebrew)",
            (Calendar::French, _) => " (French Republican)",
            (Calendar::Hijri, _) => " (Hijri)",
        };
        let s = match (self.qualifier, loc) {
            (Qualifier::Exact, _) => fa,
            (Qualifier::About, Locale::En) => format!("abt {}", fa),
            (Qualifier::About, Locale::Tr) => format!("yak. {}", fa),
            (Qualifier::Estimated, Locale::En) => format!("est {}", fa),
            (Qualifier::Estimated, Locale::Tr) => format!("tah. {}", fa),
            (Qualifier::Calculated, Locale::En) => format!("calc {}", fa),
            (Qualifier::Calculated, Locale::Tr) => format!("hes. {}", fa),
            (Qualifier::Before, Locale::En) => format!("bef {}", fa),
            (Qualifier::Before, Locale::Tr) => format!("{} öncesi", fa),
            (Qualifier::After, Locale::En) => format!("aft {}", fa),
            (Qualifier::After, Locale::Tr) => format!("{} sonrası", fa),
            (Qualifier::Between, Locale::En) => format!("bet {} and {}", fa, fb),
            (Qualifier::Between, Locale::Tr) => format!("{} ile {} arası", fa, fb),
            (Qualifier::From, Locale::En) => format!("from {}", fa),
            (Qualifier::From, Locale::Tr) => format!("{} başlayarak", fa),
            (Qualifier::To, Locale::En) => format!("to {}", fa),
            (Qualifier::To, Locale::Tr) => format!("{} dek", fa),
            (Qualifier::FromTo, Locale::En) => format!("{} – {}", fa, fb),
            (Qualifier::FromTo, Locale::Tr) => format!("{} – {}", fa, fb),
        };
        format!("{}{}", s, cal)
    }
}

fn abs_year(p: &Part) -> String {
    let y = p.year.abs();
    match p.dual_year {
        Some(alt) => format!("{}/{:02}", y, alt.rem_euclid(100)),
        None => y.to_string(),
    }
}

// ---------- age ----------

/// Whole years between two dates (earliest-day basis); `None` if either lacks a day-resolution year.
pub fn age_years(birth: &GenDate, at: &GenDate) -> Option<i32> {
    let (by, bm, bd) = ymd_greg(birth)?;
    let (ay, am, ad) = ymd_greg(at)?;
    let mut age = ay - by;
    if (am, ad) < (bm, bd) {
        age -= 1;
    }
    // no year zero
    if by < 0 && ay > 0 {
        age -= 1;
    }
    Some(age)
}

fn ymd_greg(d: &GenDate) -> Option<(i32, u8, u8)> {
    let a = d.a?;
    let cal = d.calendar;
    let (lo, _) = a.span(cal);
    let g = jdn_to_gregorian(lo);
    // Missing month/day should compare as Jan 1 so that year-only ages are year differences.
    Some(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jdn_known_values() {
        assert_eq!(gregorian_to_jdn(2000, 1, 1), 2451545);
        assert_eq!(julian_to_jdn(1582, 10, 5), gregorian_to_jdn(1582, 10, 15));
        assert_eq!(jdn_to_gregorian(2451545), (2000, 1, 1));
        assert_eq!(jdn_to_julian(gregorian_to_jdn(1582, 10, 15)), (1582, 10, 5));
    }

    #[test]
    fn bc_has_no_year_zero() {
        // 1 Jan 1 AD directly follows 31 Dec 1 BC
        assert_eq!(gregorian_to_jdn(1, 1, 1) - 1, gregorian_to_jdn(-1, 12, 31));
        assert_eq!(jdn_to_gregorian(gregorian_to_jdn(-44, 3, 15)), (-44, 3, 15));
    }

    #[test]
    fn hijri_french_hebrew_anchors() {
        // Tabular civil epoch: 1 Muharram 1 AH = 16 Jul 622 (Julian)
        assert_eq!(jdn_to_julian(hijri_to_jdn(1, 1, 1)), (622, 7, 16));
        // Tabular and observed calendars may differ by a day; stay within one.
        let d = jdn_to_gregorian(hijri_to_jdn(1446, 1, 1));
        assert!(d == (2024, 7, 7) || d == (2024, 7, 8));
        // 1 Vendémiaire An II = 22 Sep 1793; 18 Brumaire An VIII = 9 Nov 1799
        assert_eq!(jdn_to_gregorian(french_to_jdn(2, 1, 1)), (1793, 9, 22));
        assert_eq!(jdn_to_gregorian(french_to_jdn(8, 2, 18)), (1799, 11, 9));
        // 1 Tishri 5785 = 3 Oct 2024; 15 Nisan 5784 = 23 Apr 2024
        assert_eq!(jdn_to_gregorian(hebrew_to_jdn(5785, 7, 1)), (2024, 10, 3));
        assert_eq!(jdn_to_gregorian(hebrew_to_jdn(5784, 1, 15)), (2024, 4, 23));
    }

    #[test]
    fn parse_forms() {
        let d = GenDate::parse("3 Mar 1850").unwrap();
        assert_eq!(d.a, Some(Part::ymd(1850, 3, 3)));
        assert_eq!(GenDate::parse("abt 1900").unwrap().qualifier, Qualifier::About);
        let b = GenDate::parse("bet 1880 and 1890").unwrap();
        assert_eq!(b.qualifier, Qualifier::Between);
        assert_eq!(b.b, Some(Part::year_only(1890)));
        assert_eq!(GenDate::parse("March 3, 1850").unwrap().a, Some(Part::ymd(1850, 3, 3)));
        assert_eq!(GenDate::parse("3 Mart 1850").unwrap().a, Some(Part::ymd(1850, 3, 3)));
        assert_eq!(GenDate::parse("1850-03-03").unwrap().a, Some(Part::ymd(1850, 3, 3)));
        assert_eq!(GenDate::parse("3.3.1850").unwrap().a, Some(Part::ymd(1850, 3, 3)));
        assert_eq!(GenDate::parse("MAR 1850").unwrap().a.unwrap().day, None);
        assert_eq!(GenDate::parse("44 BC").unwrap().a.unwrap().year, -44);
        let j = GenDate::parse("@#DJULIAN@ 5 OCT 1582").unwrap();
        assert_eq!(j.calendar, Calendar::Julian);
        let dual = GenDate::parse("12 FEB 1750/51").unwrap();
        assert_eq!(dual.a.unwrap().dual_year, Some(1751));
        let fr = GenDate::parse("@#DFRENCH R@ 18 BRUM 8").unwrap();
        assert_eq!(fr.calendar, Calendar::French);
        assert_eq!(fr.a, Some(Part::ymd(8, 2, 18)));
        assert!(GenDate::parse("30 FEB 1850").is_err());
        assert!(GenDate::parse("").is_err());
        assert_eq!(GenDate::parse("(in the reign of Victoria)").unwrap().phrase.unwrap(), "in the reign of Victoria");
    }

    #[test]
    fn lenient_keeps_text() {
        let d = GenDate::parse_lenient("sometime in spring").unwrap();
        assert_eq!(d.phrase.as_deref(), Some("sometime in spring"));
        assert_eq!(d.sort_key(), None);
    }

    #[test]
    fn gedcom_roundtrip() {
        for s in [
            "3 MAR 1850",
            "ABT 1900",
            "BET 1880 AND 1890",
            "FROM 1900 TO 1910",
            "BEF 12 JUN 1700",
            "AFT 1700",
            "EST MAR 1700",
            "CAL 1700",
            "@#DJULIAN@ 5 OCT 1582",
            "@#DFRENCH R@ 18 BRUM 8",
            "@#DHEBREW@ 1 TSH 5785",
            "44 BC",
            "12 FEB 1750/51",
            "(unknown era)",
        ] {
            let d = GenDate::parse(s).unwrap();
            assert_eq!(d.to_gedcom(), s, "roundtrip {s}");
        }
    }

    #[test]
    fn display_locales() {
        let d = GenDate::parse("ABT 3 MAR 1850").unwrap();
        assert_eq!(d.format(Locale::En), "abt 3 Mar 1850");
        assert_eq!(d.format(Locale::Tr), "yak. 3 Mart 1850");
    }

    #[test]
    fn ordering_by_sort_key() {
        let k = |s: &str| GenDate::parse(s).unwrap().sort_key().unwrap();
        assert!(k("BEF 1850") < k("1850"));
        assert!(k("1850") < k("JUN 1850"));
        assert!(k("AFT 1850") > k("31 DEC 1850"));
        assert!(k("@#DJULIAN@ 4 OCT 1582") < k("15 OCT 1582"));
        assert_eq!(k("@#DJULIAN@ 5 OCT 1582"), k("15 OCT 1582"));
    }

    #[test]
    fn ages() {
        let b = GenDate::parse("3 MAR 1850").unwrap();
        assert_eq!(age_years(&b, &GenDate::parse("2 MAR 1900").unwrap()), Some(49));
        assert_eq!(age_years(&b, &GenDate::parse("3 MAR 1900").unwrap()), Some(50));
        assert_eq!(age_years(&GenDate::parse("1850").unwrap(), &GenDate::parse("1900").unwrap()), Some(50));
    }
}
