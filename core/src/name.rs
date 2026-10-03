//! Name model, display formatting, culture-aware surname inheritance, search normalization and phonetics.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum NameType {
    #[default]
    Birth,
    Married,
    Aka,
    Religious,
    Immigrant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PersonName {
    pub kind: NameType,
    pub prefix: String,
    pub given: String,
    pub nickname: String,
    pub surname_prefix: String,
    pub surname: String,
    pub suffix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameFormat {
    /// "Given Surname"
    GivenFirst,
    /// "Surname, Given"
    SurnameFirst,
    /// "SURNAME, Given" (capitalised surname)
    SurnameUpperFirst,
    /// "G. Surname"
    Initials,
}

impl PersonName {
    pub fn new(given: &str, surname: &str) -> Self {
        PersonName {
            given: given.into(),
            surname: surname.into(),
            ..Default::default()
        }
    }

    fn full_surname(&self) -> String {
        join(&[&self.surname_prefix, &self.surname])
    }

    pub fn display(&self, fmt: NameFormat) -> String {
        let sn = self.full_surname();
        match fmt {
            NameFormat::GivenFirst => join(&[&self.prefix, &self.given, &sn, &self.suffix]),
            NameFormat::SurnameFirst | NameFormat::SurnameUpperFirst => {
                let sn = if fmt == NameFormat::SurnameUpperFirst {
                    upper_tr(&sn)
                } else {
                    sn
                };
                let rest = join(&[&self.prefix, &self.given, &self.suffix]);
                match (sn.is_empty(), rest.is_empty()) {
                    (true, _) => rest,
                    (_, true) => sn,
                    _ => format!("{}, {}", sn, rest),
                }
            }
            NameFormat::Initials => {
                let init: Vec<String> = self
                    .given
                    .split_whitespace()
                    .filter_map(|g| g.chars().next())
                    .map(|c| format!("{}.", c.to_uppercase()))
                    .collect();
                join(&[&init.join(" "), &sn])
            }
        }
    }

    /// GEDCOM NAME value: `Given /Surname/ Suffix`.
    pub fn to_gedcom(&self) -> String {
        let given = join(&[&self.prefix, &self.given]);
        let mut s = given;
        s.push_str(&format!(" /{}/", self.full_surname()));
        if !self.suffix.is_empty() {
            s.push(' ');
            s.push_str(&self.suffix);
        }
        s.trim().to_string()
    }

    /// Parse a GEDCOM NAME value (`Given /Surname/ Suffix`). A bare string becomes the given name.
    pub fn from_gedcom(v: &str) -> PersonName {
        let mut n = PersonName::default();
        match (v.find('/'), v.rfind('/')) {
            (Some(a), Some(b)) if b > a => {
                n.given = v[..a].trim().to_string();
                n.surname = v[a + 1..b].trim().to_string();
                n.suffix = v[b + 1..].trim().to_string();
            }
            _ => n.given = v.trim().to_string(),
        }
        n
    }
}

fn join(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|p| !p.trim().is_empty())
        .map(|p| p.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Turkish-aware uppercase (i→İ, ı→I).
pub fn upper_tr(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            'i' => vec!['İ'],
            'ı' => vec!['I'],
            c => c.to_uppercase().collect(),
        })
        .collect()
}

/// Turkish-aware title case for capitalisation normalisation ("AHMET yılmaz" → "Ahmet Yılmaz").
pub fn title_case_tr(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut cs = w.chars();
            let first = cs
                .next()
                .map(|c| upper_tr(&c.to_string()))
                .unwrap_or_default();
            let rest: String = cs
                .flat_map(|c| match c {
                    'I' => vec!['ı'],
                    'İ' => vec!['i'],
                    c => c.to_lowercase().collect(),
                })
                .collect();
            format!("{}{}", first, rest)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Fold a string for search: lowercase, strip diacritics (incl. ı/İ ş ğ ç ö ü), collapse whitespace.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let f = match c {
            'ı' | 'İ' | 'I' | 'i' | 'î' | 'ï' | 'í' | 'ì' | 'Î' | 'Ï' | 'Í' | 'Ì' => 'i',
            'ş' | 'Ş' | 'ś' | 'š' | 'Š' => 's',
            'ğ' | 'Ğ' => 'g',
            'ç' | 'Ç' | 'ć' | 'č' | 'Č' => 'c',
            'ö' | 'Ö' | 'ó' | 'ò' | 'ô' | 'õ' | 'ø' | 'Ø' | 'Ó' | 'Ò' | 'Ô' | 'Õ' => {
                'o'
            }
            'ü' | 'Ü' | 'ú' | 'ù' | 'û' | 'Ú' | 'Ù' | 'Û' => 'u',
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => {
                'a'
            }
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'ñ' | 'Ñ' => 'n',
            'ý' | 'ÿ' | 'Ý' => 'y',
            'ž' | 'Ž' | 'ź' | 'ż' => 'z',
            'ß' => {
                out.push_str("ss");
                continue;
            }
            'æ' | 'Æ' => {
                out.push_str("ae");
                continue;
            }
            c => c.to_lowercase().next().unwrap_or(c),
        };
        out.push(f);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------- phonetics ----------

/// American Soundex (4 chars).
pub fn soundex(s: &str) -> String {
    let letters: Vec<char> = fold(s)
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    let Some(&first) = letters.first() else {
        return String::new();
    };
    let code = |c: char| match c {
        'b' | 'f' | 'p' | 'v' => '1',
        'c' | 'g' | 'j' | 'k' | 'q' | 's' | 'x' | 'z' => '2',
        'd' | 't' => '3',
        'l' => '4',
        'm' | 'n' => '5',
        'r' => '6',
        _ => '0',
    };
    let mut out = String::new();
    out.push(first.to_ascii_uppercase());
    let mut prev = code(first);
    for &c in &letters[1..] {
        let k = code(c);
        if k != '0' && k != prev {
            out.push(k);
            if out.len() == 4 {
                break;
            }
        }
        // h and w do not separate equal codes
        if c != 'h' && c != 'w' {
            prev = k;
        }
    }
    while out.len() < 4 {
        out.push('0');
    }
    out
}

/// Kölner Phonetik (Cologne phonetics), returns a digit string.
pub fn cologne(s: &str) -> String {
    let t: Vec<char> = fold(s)
        .replace("ae", "a")
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    let mut codes: Vec<char> = Vec::new();
    for i in 0..t.len() {
        let c = t[i];
        let prev = if i > 0 { Some(t[i - 1]) } else { None };
        let next = t.get(i + 1).copied();
        let code = match c {
            'a' | 'e' | 'i' | 'j' | 'o' | 'u' | 'y' => '0',
            'h' => continue,
            'b' => '1',
            'p' => {
                if next == Some('h') {
                    '3'
                } else {
                    '1'
                }
            }
            'd' | 't' => {
                if matches!(next, Some('c' | 's' | 'z')) {
                    '8'
                } else {
                    '2'
                }
            }
            'f' | 'v' | 'w' => '3',
            'g' | 'k' | 'q' => '4',
            'c' => {
                let onset = i == 0;
                if onset {
                    if matches!(
                        next,
                        Some('a' | 'h' | 'k' | 'l' | 'o' | 'q' | 'r' | 'u' | 'x')
                    ) {
                        '4'
                    } else {
                        '8'
                    }
                } else if matches!(prev, Some('s' | 'z')) {
                    '8'
                } else if matches!(next, Some('a' | 'h' | 'k' | 'o' | 'q' | 'u' | 'x')) {
                    '4'
                } else {
                    '8'
                }
            }
            'x' => {
                if matches!(prev, Some('c' | 'k' | 'q')) {
                    '8'
                } else {
                    codes.push('4');
                    '8'
                }
            }
            'l' => '5',
            'm' | 'n' => '6',
            'r' => '7',
            's' | 'z' => '8',
            _ => continue,
        };
        codes.push(code);
    }
    let mut out = String::new();
    let mut last = None;
    for c in codes {
        if Some(c) != last {
            out.push(c);
        }
        last = Some(c);
    }
    // drop zeros except leading
    let mut res = String::new();
    for (i, c) in out.chars().enumerate() {
        if c != '0' || i == 0 {
            res.push(c);
        }
    }
    res
}

/// Levenshtein distance on folded strings, for fuzzy name matching.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = fold(a).chars().collect();
    let b: Vec<char> = fold(b).chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// 0.0‥1.0 similarity from folded edit distance.
pub fn similarity(a: &str, b: &str) -> f64 {
    let (fa, fb) = (fold(a), fold(b));
    let max = fa.chars().count().max(fb.chars().count());
    if max == 0 {
        return 1.0;
    }
    1.0 - edit_distance(&fa, &fb) as f64 / max as f64
}

// ---------- culture-aware surname inheritance ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamingCulture {
    /// Child takes father's surname.
    Patrilineal,
    /// Child takes father's given name + -son/-dóttir (Icelandic).
    Patronymic,
    /// Spanish double surname: father's first + mother's first.
    SpanishDouble,
    /// Turkish soyadı (patrilineal; married women may keep birth name as second).
    Turkish,
    /// Slavic gendered endings (-ski/-ska, -ov/-ova, -ić stays).
    SlavicGendered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sex {
    Male,
    Female,
    Other,
    Unknown,
}

/// Suggest a child's surname for the given culture. `father`/`mother` are the parents' names (either may be absent).
pub fn suggest_child_surname(
    culture: NamingCulture,
    father: Option<&PersonName>,
    mother: Option<&PersonName>,
    child_sex: Sex,
) -> String {
    match culture {
        NamingCulture::Patrilineal | NamingCulture::Turkish => father
            .map(|f| f.surname.clone())
            .or_else(|| mother.map(|m| m.surname.clone()))
            .unwrap_or_default(),
        NamingCulture::Patronymic => match father {
            Some(f) if !f.given.is_empty() => {
                let base = f.given.split_whitespace().next().unwrap_or("");
                let genitive = if base.ends_with('i') || base.ends_with('a') {
                    base.to_string()
                } else {
                    format!("{}s", base)
                };
                match child_sex {
                    Sex::Female => format!("{}dóttir", genitive),
                    _ => format!("{}son", genitive),
                }
            }
            _ => String::new(),
        },
        NamingCulture::SpanishDouble => {
            let first = |n: Option<&PersonName>| {
                n.map(|n| {
                    n.surname
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_string()
                })
                .unwrap_or_default()
            };
            join(&[&first(father), &first(mother)])
        }
        NamingCulture::SlavicGendered => {
            let base = father.map(|f| f.surname.clone()).unwrap_or_default();
            slavic_inflect(&base, child_sex)
        }
    }
}

/// Convert a Slavic surname to its masculine/feminine form (-ski↔-ska, -cki↔-cka, -ov↔-ova, -in↔-ina, -ev↔-eva).
pub fn slavic_inflect(surname: &str, sex: Sex) -> String {
    let female = sex == Sex::Female;
    let pairs = [
        ("ski", "ska"),
        ("cki", "cka"),
        ("dzki", "dzka"),
        ("ov", "ova"),
        ("ev", "eva"),
        ("in", "ina"),
    ];
    for (m, f) in pairs {
        if female {
            if let Some(stem) = surname.strip_suffix(m) {
                return format!("{}{}", stem, f);
            }
        } else if let Some(stem) = surname.strip_suffix(f) {
            return format!("{}{}", stem, m);
        }
    }
    surname.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gedcom_name_roundtrip() {
        let n = PersonName::from_gedcom("John Quincy /Adams/ Jr");
        assert_eq!(
            (n.given.as_str(), n.surname.as_str(), n.suffix.as_str()),
            ("John Quincy", "Adams", "Jr")
        );
        assert_eq!(n.to_gedcom(), "John Quincy /Adams/ Jr");
        assert_eq!(PersonName::from_gedcom("Madonna").given, "Madonna");
        assert_eq!(PersonName::from_gedcom("/Smith/").surname, "Smith");
    }

    #[test]
    fn display_formats() {
        let n = PersonName::new("Ayşe Nur", "Yılmaz");
        assert_eq!(n.display(NameFormat::GivenFirst), "Ayşe Nur Yılmaz");
        assert_eq!(n.display(NameFormat::SurnameFirst), "Yılmaz, Ayşe Nur");
        assert_eq!(n.display(NameFormat::SurnameUpperFirst), "YILMAZ, Ayşe Nur");
        assert_eq!(n.display(NameFormat::Initials), "A. N. Yılmaz");
    }

    #[test]
    fn turkish_case() {
        assert_eq!(upper_tr("ışık iğne"), "IŞIK İĞNE");
        assert_eq!(title_case_tr("İSMAİL ılgaz"), "İsmail Ilgaz");
        assert_eq!(fold("ŞÖYLE ĞÜÇ ıİ"), "soyle guc ii");
        assert_eq!(fold("Müller"), fold("Muller"));
    }

    #[test]
    fn soundex_values() {
        assert_eq!(soundex("Robert"), "R163");
        assert_eq!(soundex("Rupert"), "R163");
        assert_eq!(soundex("Ashcraft"), "A261");
        assert_eq!(soundex("Tymczak"), "T522");
        assert_eq!(soundex("Pfister"), "P236");
        assert_eq!(soundex(""), "");
    }

    #[test]
    fn cologne_values() {
        assert_eq!(cologne("Müller-Lüdenscheidt"), "65752682");
        assert_eq!(cologne("Meier"), cologne("Mayer"));
        assert_eq!(cologne("Wikipedia"), "3412");
    }

    #[test]
    fn fuzzy() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert!(similarity("Yilmaz", "Yılmaz") > 0.99);
        assert!(similarity("Smith", "Smyth") > 0.7);
    }

    #[test]
    fn surname_rules() {
        let f = PersonName::new("Jón", "Jónsson");
        let m = PersonName::new("Anna", "García López");
        assert_eq!(
            suggest_child_surname(NamingCulture::Patrilineal, Some(&f), Some(&m), Sex::Male),
            "Jónsson"
        );
        assert_eq!(
            suggest_child_surname(
                NamingCulture::SpanishDouble,
                Some(&PersonName::new("Luis", "Pérez Ruiz")),
                Some(&m),
                Sex::Male
            ),
            "Pérez García"
        );
        assert_eq!(
            suggest_child_surname(NamingCulture::Patronymic, Some(&f), None, Sex::Female),
            "Jónsdóttir"
        );
        assert_eq!(
            suggest_child_surname(
                NamingCulture::Patronymic,
                Some(&PersonName::new("Gunnar", "")),
                None,
                Sex::Male
            ),
            "Gunnarsson"
        );
        assert_eq!(slavic_inflect("Kowalski", Sex::Female), "Kowalska");
        assert_eq!(slavic_inflect("Ivanova", Sex::Male), "Ivanov");
        assert_eq!(slavic_inflect("Horvat", Sex::Female), "Horvat");
    }
}
