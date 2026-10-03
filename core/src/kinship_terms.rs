//! Language-specific names for computed relationships (EN, TR, DE, ES, FR, RU, AR).
//!
//! Direct-line, sibling, aunt/uncle, nephew/niece, cousin, spouse, step and the common in-law roles are covered for every
//! language; rarer compounds fall back to a templated "spouse's X" / "X's spouse" phrase.

use crate::relationship::{Blood, Kind, LinkKind, Relation, Sex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Tr,
    De,
    Es,
    Fr,
    Ru,
    Ar,
}

impl Lang {
    pub fn from_code(c: &str) -> Lang {
        match c {
            "tr" => Lang::Tr,
            "de" => Lang::De,
            "es" => Lang::Es,
            "fr" => Lang::Fr,
            "ru" => Lang::Ru,
            "ar" => Lang::Ar,
            _ => Lang::En,
        }
    }
}

fn pick(s: Sex, m: &str, f: &str, n: &str) -> String {
    match s {
        Sex::Male => m,
        Sex::Female => f,
        Sex::Unknown => n,
    }
    .to_string()
}

fn ordinal_en(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{}{}", n, suffix)
}

pub fn describe(rel: &Relation, lang: Lang) -> String {
    let (a, b) = (rel.a_sex, rel.b_sex);
    match &rel.kind {
        Kind::Same => match lang {
            Lang::En => "self",
            Lang::Tr => "kendisi",
            Lang::De => "dieselbe Person",
            Lang::Es => "la misma persona",
            Lang::Fr => "la même personne",
            Lang::Ru => "тот же человек",
            Lang::Ar => "نفس الشخص",
        }
        .to_string(),
        Kind::Unrelated => match lang {
            Lang::En => "not related",
            Lang::Tr => "akrabalık yok",
            Lang::De => "nicht verwandt",
            Lang::Es => "sin parentesco",
            Lang::Fr => "aucun lien de parenté",
            Lang::Ru => "не родственники",
            Lang::Ar => "غير مرتبطين",
        }
        .to_string(),
        Kind::Spouse => match lang {
            Lang::En => pick(b, "husband", "wife", "spouse"),
            Lang::Tr => pick(b, "koca", "karı", "eş"),
            Lang::De => pick(b, "Ehemann", "Ehefrau", "Ehepartner"),
            Lang::Es => pick(b, "esposo", "esposa", "cónyuge"),
            Lang::Fr => pick(b, "époux", "épouse", "conjoint"),
            Lang::Ru => pick(b, "муж", "жена", "супруг"),
            Lang::Ar => pick(b, "زوج", "زوجة", "قرين"),
        },
        Kind::StepChild => match lang {
            Lang::En => pick(b, "stepson", "stepdaughter", "stepchild"),
            Lang::Tr => pick(b, "üvey oğul", "üvey kız", "üvey evlat"),
            Lang::De => pick(b, "Stiefsohn", "Stieftochter", "Stiefkind"),
            Lang::Es => pick(b, "hijastro", "hijastra", "hijastro/a"),
            Lang::Fr => pick(b, "beau-fils", "belle-fille", "enfant du conjoint"),
            Lang::Ru => pick(b, "пасынок", "падчерица", "приёмыш супруга"),
            Lang::Ar => pick(b, "ربيب", "ربيبة", "ربيب"),
        },
        Kind::StepParent => match lang {
            Lang::En => pick(b, "stepfather", "stepmother", "stepparent"),
            Lang::Tr => pick(b, "üvey baba", "üvey anne", "üvey ebeveyn"),
            Lang::De => pick(b, "Stiefvater", "Stiefmutter", "Stiefelternteil"),
            Lang::Es => pick(b, "padrastro", "madrastra", "padrastro/madrastra"),
            Lang::Fr => pick(b, "beau-père", "belle-mère", "beau-parent"),
            Lang::Ru => pick(b, "отчим", "мачеха", "отчим/мачеха"),
            Lang::Ar => pick(b, "زوج الأم", "زوجة الأب", "زوج الأم/زوجة الأب"),
        },
        Kind::Blood(bl) => blood_name(bl, a, b, lang),
        Kind::SpouseRelative { blood, .. } => spouse_relative(blood, a, b, lang),
        Kind::RelativeSpouse { blood, .. } => relative_spouse(blood, a, b, lang),
        Kind::SpouseRelativeSpouse { blood, .. } => {
            if (blood.up, blood.down) == (1, 1) {
                match lang {
                    Lang::Tr => pick(b, "bacanak", "elti", "elti/bacanak"),
                    Lang::En => pick(b, "brother-in-law", "sister-in-law", "sibling-in-law"),
                    Lang::De => pick(b, "Schwager", "Schwägerin", "angeheiratete Verwandte"),
                    Lang::Es => pick(b, "concuñado", "concuñada", "concuñado/a"),
                    Lang::Fr => pick(b, "beau-frère", "belle-sœur", "beau-frère/belle-sœur"),
                    Lang::Ru => pick(b, "свояк", "золовка", "свояк/золовка"),
                    Lang::Ar => pick(b, "نسيب", "نسيبة", "نسيب"),
                }
            } else {
                generic_spouse_of_relative_of_spouse(lang)
            }
        }
    }
}

fn generic_spouse_of_relative_of_spouse(lang: Lang) -> String {
    match lang {
        Lang::En => "spouse of a relative of the spouse",
        Lang::Tr => "eşin akrabasının eşi",
        Lang::De => "Ehepartner eines Verwandten des Ehepartners",
        Lang::Es => "cónyuge de un pariente del cónyuge",
        Lang::Fr => "conjoint d'un parent du conjoint",
        Lang::Ru => "супруг(а) родственника супруга(и)",
        Lang::Ar => "زوج قريب الزوج",
    }
    .to_string()
}

fn neutral_blood(bl: &Blood, lang: Lang) -> String {
    blood_name(bl, Sex::Unknown, Sex::Unknown, lang)
}

fn spouse_relative(bl: &Blood, a: Sex, b: Sex, lang: Lang) -> String {
    match ((bl.up, bl.down), lang) {
        ((1, 0), Lang::En) => pick(b, "father-in-law", "mother-in-law", "parent-in-law"),
        ((1, 0), Lang::Tr) => pick(b, "kayınpeder", "kayınvalide", "kayın"),
        ((1, 0), Lang::De) => pick(
            b,
            "Schwiegervater",
            "Schwiegermutter",
            "Schwiegerelternteil",
        ),
        ((1, 0), Lang::Es) => pick(b, "suegro", "suegra", "suegro/a"),
        ((1, 0), Lang::Fr) => pick(b, "beau-père", "belle-mère", "beau-parent"),
        ((1, 0), Lang::Ru) => match (a, b) {
            (Sex::Male, Sex::Male) => "тесть".into(),
            (Sex::Male, Sex::Female) => "тёща".into(),
            (Sex::Female, Sex::Male) => "свёкор".into(),
            (Sex::Female, Sex::Female) => "свекровь".into(),
            _ => "родитель супруга(и)".into(),
        },
        ((1, 0), Lang::Ar) => pick(b, "حمو", "حماة", "حمو"),
        ((1, 1), Lang::En) => pick(b, "brother-in-law", "sister-in-law", "sibling-in-law"),
        ((1, 1), Lang::Tr) => match (a, b) {
            (_, Sex::Male) => "kayınbirader".into(),
            (Sex::Male, Sex::Female) => "baldız".into(),
            (Sex::Female, Sex::Female) => "görümce".into(),
            _ => "kayın".into(),
        },
        ((1, 1), Lang::De) => pick(b, "Schwager", "Schwägerin", "Schwager/Schwägerin"),
        ((1, 1), Lang::Es) => pick(b, "cuñado", "cuñada", "cuñado/a"),
        ((1, 1), Lang::Fr) => pick(b, "beau-frère", "belle-sœur", "beau-frère/belle-sœur"),
        ((1, 1), Lang::Ru) => match (a, b) {
            (Sex::Male, Sex::Male) => "шурин".into(),
            (Sex::Male, Sex::Female) => "свояченица".into(),
            (Sex::Female, Sex::Male) => "деверь".into(),
            (Sex::Female, Sex::Female) => "золовка".into(),
            _ => "брат/сестра супруга(и)".into(),
        },
        ((1, 1), Lang::Ar) => pick(b, "سلف", "سلفة", "سلف"),
        _ => {
            let x = neutral_blood(bl, lang);
            match lang {
                Lang::En => format!("spouse's {}", x),
                Lang::Tr => format!("eşin {}", x),
                Lang::De => format!("{} des Ehepartners", x),
                Lang::Es => format!("{} del cónyuge", x),
                Lang::Fr => format!("{} du conjoint", x),
                Lang::Ru => format!("{} супруга(и)", x),
                Lang::Ar => format!("{} الزوج", x),
            }
        }
    }
}

fn relative_spouse(bl: &Blood, _a: Sex, b: Sex, lang: Lang) -> String {
    match ((bl.up, bl.down), lang) {
        ((0, 1), Lang::En) => pick(b, "son-in-law", "daughter-in-law", "child-in-law"),
        ((0, 1), Lang::Tr) => pick(b, "damat", "gelin", "damat/gelin"),
        ((0, 1), Lang::De) => pick(b, "Schwiegersohn", "Schwiegertochter", "Schwiegerkind"),
        ((0, 1), Lang::Es) => pick(b, "yerno", "nuera", "yerno/nuera"),
        ((0, 1), Lang::Fr) => pick(b, "gendre", "belle-fille", "gendre/belle-fille"),
        ((0, 1), Lang::Ru) => pick(b, "зять", "невестка", "зять/невестка"),
        ((0, 1), Lang::Ar) => pick(b, "صهر", "كنة", "صهر"),
        ((1, 1), Lang::En) => pick(b, "brother-in-law", "sister-in-law", "sibling-in-law"),
        ((1, 1), Lang::Tr) => pick(b, "enişte", "yenge", "enişte/yenge"),
        ((1, 1), Lang::De) => pick(b, "Schwager", "Schwägerin", "Schwager/Schwägerin"),
        ((1, 1), Lang::Es) => pick(b, "cuñado", "cuñada", "cuñado/a"),
        ((1, 1), Lang::Fr) => pick(b, "beau-frère", "belle-sœur", "beau-frère/belle-sœur"),
        ((1, 1), Lang::Ru) => pick(b, "зять", "невестка", "зять/невестка"),
        ((1, 1), Lang::Ar) => pick(b, "زوج الأخت", "زوجة الأخ", "قرين الأخ/الأخت"),
        ((2, 1), Lang::En) => pick(
            b,
            "uncle (by marriage)",
            "aunt (by marriage)",
            "aunt/uncle (by marriage)",
        ),
        ((2, 1), Lang::Tr) => pick(b, "enişte", "yenge", "enişte/yenge"),
        ((2, 1), Lang::De) => pick(
            b,
            "Onkel (angeheiratet)",
            "Tante (angeheiratet)",
            "angeheiratete Tante/Onkel",
        ),
        ((2, 1), Lang::Es) => pick(b, "tío político", "tía política", "tío/a político/a"),
        ((2, 1), Lang::Fr) => pick(
            b,
            "oncle par alliance",
            "tante par alliance",
            "oncle/tante par alliance",
        ),
        ((2, 1), Lang::Ru) => pick(b, "муж тёти", "жена дяди", "супруг(а) дяди/тёти"),
        ((2, 1), Lang::Ar) => pick(b, "زوج العمة/الخالة", "زوجة العم/الخال", "زوج العم/العمة"),
        _ => {
            let x = neutral_blood(bl, lang);
            match lang {
                Lang::En => format!("{}'s spouse", x),
                Lang::Tr => format!("{} eşi", x),
                Lang::De => format!("Ehepartner von {}", x),
                Lang::Es => format!("cónyuge de {}", x),
                Lang::Fr => format!("conjoint de {}", x),
                Lang::Ru => format!("супруг(а): {}", x),
                Lang::Ar => format!("زوج {}", x),
            }
        }
    }
}

fn adoption_suffix(bl: &Blood, lang: Lang, direct: bool) -> Option<(&'static str, bool)> {
    // (word, is_prefix). Direct parent/child relations take a prefix; others a suffix.
    let _ = direct;
    match (bl.link, lang) {
        (LinkKind::Adopted, Lang::En) => Some(("adoptive", true)),
        (LinkKind::Foster, Lang::En) => Some(("foster", true)),
        (LinkKind::Adopted, Lang::Tr) => Some(("evlat edinilmiş", true)),
        (LinkKind::Foster, Lang::Tr) => Some(("koruyucu", true)),
        (LinkKind::Adopted, Lang::De) => Some(("(Adoptiv)", false)),
        (LinkKind::Foster, Lang::De) => Some(("(Pflege)", false)),
        (LinkKind::Adopted, Lang::Es) => Some(("adoptivo/a", false)),
        (LinkKind::Foster, Lang::Es) => Some(("de acogida", false)),
        (LinkKind::Adopted, Lang::Fr) => Some(("adoptif", false)),
        (LinkKind::Foster, Lang::Fr) => Some(("d'accueil", false)),
        (LinkKind::Adopted, Lang::Ru) => Some(("приёмный", true)),
        (LinkKind::Foster, Lang::Ru) => Some(("опекаемый", true)),
        (LinkKind::Adopted, Lang::Ar) => Some(("بالتبني", false)),
        (LinkKind::Foster, Lang::Ar) => Some(("بالرعاية", false)),
        _ => None,
    }
}

pub fn blood_name(bl: &Blood, a: Sex, b: Sex, lang: Lang) -> String {
    let base = match lang {
        Lang::En => en(bl, b),
        Lang::Tr => tr(bl, a, b),
        Lang::De => de(bl, b),
        Lang::Es => es(bl, b),
        Lang::Fr => fr(bl, b),
        Lang::Ru => ru(bl, b),
        Lang::Ar => ar(bl, b),
    };
    match adoption_suffix(bl, lang, bl.up + bl.down <= 1) {
        Some((w, true)) => format!("{} {}", w, base),
        Some((w, false)) => format!("{} {}", base, w),
        None => base,
    }
}

// ---------------- English ----------------

fn greats_en(n: usize) -> String {
    "great-".repeat(n)
}

fn en(bl: &Blood, b: Sex) -> String {
    let (u, d) = (bl.up, bl.down);
    let half = if bl.half() { "half-" } else { "" };
    match (u, d) {
        (0, 1) => pick(b, "son", "daughter", "child"),
        (0, n) => format!(
            "{}{}",
            greats_en(n - 2),
            pick(b, "grandson", "granddaughter", "grandchild")
        ),
        (1, 0) => pick(b, "father", "mother", "parent"),
        (n, 0) => format!(
            "{}{}",
            greats_en(n - 2),
            pick(b, "grandfather", "grandmother", "grandparent")
        ),
        (1, 1) => format!("{}{}", half, pick(b, "brother", "sister", "sibling")),
        (1, n) => format!(
            "{}{}{}",
            half,
            greats_en(n - 2),
            pick(b, "nephew", "niece", "nibling")
        ),
        (n, 1) => format!(
            "{}{}{}",
            half,
            greats_en(n - 2),
            pick(b, "uncle", "aunt", "aunt/uncle")
        ),
        (u, d) => {
            let degree = u.min(d) - 1;
            let removed = u.abs_diff(d);
            let mut s = format!("{}{} cousin", half, ordinal_en(degree));
            match removed {
                0 => {}
                1 => s.push_str(" once removed"),
                2 => s.push_str(" twice removed"),
                n => s.push_str(&format!(" {} times removed", n)),
            }
            s
        }
    }
}

// ---------------- Turkish ----------------

fn tr(bl: &Blood, _a: Sex, b: Sex) -> String {
    let (u, d) = (bl.up, bl.down);
    let side_male = bl.side == Sex::Male;
    let by_marriage_prefix = if bl.half() { "üvey " } else { "" };
    match (u, d) {
        (0, 1) => pick(b, "oğul", "kız", "evlat"),
        (0, 2) => "torun".into(),
        (0, 3) => "torunun çocuğu".into(),
        (0, n) => format!("{}. kuşak torun", n),
        (1, 0) => pick(b, "baba", "anne", "ebeveyn"),
        (2, 0) => match b {
            Sex::Male => "dede".into(),
            Sex::Female => if side_male { "babaanne" } else { "anneanne" }.into(),
            Sex::Unknown => "büyükebeveyn".into(),
        },
        (3, 0) => pick(b, "büyük dede", "büyük nine", "büyük ata"),
        (n, 0) => format!("{}. kuşak büyük ata", n - 1),
        (1, 1) => {
            let half = if bl.half() {
                match bl.anc_sex {
                    Sex::Male => "baba bir ",
                    Sex::Female => "ana bir ",
                    Sex::Unknown => "üvey ",
                }
            } else {
                ""
            };
            format!(
                "{}{}",
                half,
                pick(b, "erkek kardeş", "kız kardeş", "kardeş")
            )
        }
        (1, 2) => format!("{}yeğen", by_marriage_prefix),
        (1, 3) => "yeğenin çocuğu".into(),
        (1, n) => format!("{}. kuşak yeğen", n),
        (2, 1) => match (side_male, b) {
            (true, Sex::Male) => "amca".into(),
            (true, Sex::Female) => "hala".into(),
            (false, Sex::Male) => "dayı".into(),
            (false, Sex::Female) => "teyze".into(),
            (true, _) => "amca/hala".into(),
            (false, _) => "dayı/teyze".into(),
        },
        (n, 1) => format!(
            "{}{}",
            "büyük ".repeat(n - 2),
            match (side_male, b) {
                (true, Sex::Male) => "amca",
                (true, Sex::Female) => "hala",
                (false, Sex::Male) => "dayı",
                (false, Sex::Female) => "teyze",
                (true, _) => "amca/hala",
                (false, _) => "dayı/teyze",
            }
        ),
        (2, 2) => {
            // The relative on the connecting side: amca/hala (father's siblings) or dayı/teyze (mother's siblings).
            let uncle = match (side_male, bl.b_below) {
                (true, Sex::Male) => "amca",
                (true, Sex::Female) => "hala",
                (false, Sex::Male) => "dayı",
                (false, Sex::Female) => "teyze",
                _ => "",
            };
            if uncle.is_empty() {
                "kuzen".into()
            } else {
                format!("{}{}", uncle, pick(b, " oğlu", " kızı", " çocuğu"))
            }
        }
        (u, d) => {
            let degree = u.min(d) - 1;
            let removed = u.abs_diff(d);
            if removed == 0 {
                format!("{}. dereceden kuzen", degree)
            } else {
                format!("{}. dereceden kuzen ({} kuşak farklı)", degree, removed)
            }
        }
    }
}

// ---------------- German ----------------

fn de(bl: &Blood, b: Sex) -> String {
    let (u, d) = (bl.up, bl.down);
    let ur = |n: usize| "Ur".repeat(n.saturating_sub(2));
    match (u, d) {
        (0, 1) => pick(b, "Sohn", "Tochter", "Kind"),
        (0, 2) => pick(b, "Enkel", "Enkelin", "Enkelkind"),
        (0, n) => format!("{}{}", ur(n), pick(b, "Enkel", "Enkelin", "Enkelkind"))
            .replacen("UrEnkel", "UrEnkel", 1),
        (1, 0) => pick(b, "Vater", "Mutter", "Elternteil"),
        (2, 0) => pick(b, "Großvater", "Großmutter", "Großelternteil"),
        (n, 0) => format!(
            "{}{}",
            ur(n),
            pick(b, "großvater", "großmutter", "großelternteil")
        ),
        (1, 1) => {
            if bl.half() {
                pick(b, "Halbbruder", "Halbschwester", "Halbgeschwister")
            } else {
                pick(b, "Bruder", "Schwester", "Geschwister")
            }
        }
        (1, 2) => pick(b, "Neffe", "Nichte", "Neffe/Nichte"),
        (1, n) => format!("{}{}", ur(n), pick(b, "neffe", "nichte", "neffe/nichte")),
        (2, 1) => pick(b, "Onkel", "Tante", "Onkel/Tante"),
        (n, 1) => format!("{}{}", ur(n), pick(b, "onkel", "tante", "onkel/tante")),
        (u, d) => {
            let degree = u.min(d) - 1;
            let removed = u.abs_diff(d);
            let word = pick(b, "Cousin", "Cousine", "Cousin/Cousine");
            let mut s = if degree == 1 {
                word
            } else {
                format!("{} {}. Grades", word, degree)
            };
            if removed > 0 {
                s.push_str(&format!(", {} Generation(en) versetzt", removed));
            }
            s
        }
    }
}

// ---------------- Spanish ----------------

fn es(bl: &Blood, b: Sex) -> String {
    let (u, d) = (bl.up, bl.down);
    let half = bl.half();
    let bis = |n: usize| match n {
        2 => "",
        3 => "bis",
        4 => "tatara",
        n => {
            if n > 4 {
                "tatara"
            } else {
                ""
            }
        }
    };
    match (u, d) {
        (0, 1) => pick(b, "hijo", "hija", "hijo/a"),
        (0, 2) => pick(b, "nieto", "nieta", "nieto/a"),
        (0, n) => format!("{}{}", bis(n), pick(b, "nieto", "nieta", "nieto/a")),
        (1, 0) => pick(b, "padre", "madre", "progenitor"),
        (2, 0) => pick(b, "abuelo", "abuela", "abuelo/a"),
        (n, 0) => format!("{}{}", bis(n), pick(b, "abuelo", "abuela", "abuelo/a")),
        (1, 1) => format!(
            "{}{}",
            if half { "medio " } else { "" },
            pick(b, "hermano", "hermana", "hermano/a")
        )
        .replace("medio hermana", "media hermana"),
        (1, 2) => pick(b, "sobrino", "sobrina", "sobrino/a"),
        (1, n) => format!("{}{}", bis(n), pick(b, "sobrino", "sobrina", "sobrino/a")),
        (2, 1) => pick(b, "tío", "tía", "tío/a"),
        (n, 1) => format!(
            "{} {}",
            pick(b, "tío", "tía", "tío/a"),
            if n == 3 { "abuelo" } else { "bisabuelo" }
        )
        .replace("tía abuelo", "tía abuela"),
        (u, d) => {
            let degree = u.min(d) - 1;
            let removed = u.abs_diff(d);
            let base = pick(b, "primo", "prima", "primo/a");
            let mut s = match degree {
                1 => base,
                2 => format!("{} segundo", base).replace("prima segundo", "prima segunda"),
                n => format!("{} de {}º grado", base, n),
            };
            if removed > 0 {
                s.push_str(&format!(" ({} generación/es de diferencia)", removed));
            }
            s
        }
    }
}

// ---------------- French ----------------

fn fr(bl: &Blood, b: Sex) -> String {
    let (u, d) = (bl.up, bl.down);
    let arriere = |n: usize| "arrière-".repeat(n.saturating_sub(2));
    match (u, d) {
        (0, 1) => pick(b, "fils", "fille", "enfant"),
        (0, 2) => pick(b, "petit-fils", "petite-fille", "petit-enfant"),
        (0, n) => format!(
            "{}{}",
            arriere(n),
            pick(b, "petit-fils", "petite-fille", "petit-enfant")
        ),
        (1, 0) => pick(b, "père", "mère", "parent"),
        (2, 0) => pick(b, "grand-père", "grand-mère", "grand-parent"),
        (n, 0) => format!(
            "{}{}",
            arriere(n),
            pick(b, "grand-père", "grand-mère", "grand-parent")
        ),
        (1, 1) => format!(
            "{}{}",
            if bl.half() { "demi-" } else { "" },
            pick(b, "frère", "sœur", "frère/sœur")
        ),
        (1, 2) => pick(b, "neveu", "nièce", "neveu/nièce"),
        (1, n) => format!(
            "{}{}",
            pick(b, "petit-neveu", "petite-nièce", "petit-neveu/nièce"),
            if n > 3 { " (éloigné)" } else { "" }
        ),
        (2, 1) => pick(b, "oncle", "tante", "oncle/tante"),
        (n, 1) => format!(
            "{}{}",
            pick(b, "grand-oncle", "grand-tante", "grand-oncle/tante"),
            if n > 3 { " (éloigné)" } else { "" }
        ),
        (u, d) => {
            let degree = u.min(d) - 1;
            let removed = u.abs_diff(d);
            let base = pick(b, "cousin", "cousine", "cousin/e");
            let mut s = match degree {
                1 => format!(
                    "{} germain{}",
                    base,
                    if b == Sex::Female { "e" } else { "" }
                )
                .replace("cousine germaine", "cousine germaine"),
                n => format!("{} au {}e degré", base, n + 1),
            };
            if removed > 0 {
                s.push_str(&format!(
                    " (issu de germain, {} génération(s) d'écart)",
                    removed
                ));
            }
            s
        }
    }
}

// ---------------- Russian ----------------

fn ru(bl: &Blood, b: Sex) -> String {
    let (u, d) = (bl.up, bl.down);
    let pra = |n: usize| "пра".repeat(n.saturating_sub(2));
    match (u, d) {
        (0, 1) => pick(b, "сын", "дочь", "ребёнок"),
        (0, 2) => pick(b, "внук", "внучка", "внук/внучка"),
        (0, n) => format!("{}{}", pra(n), pick(b, "внук", "внучка", "внук/внучка")),
        (1, 0) => pick(b, "отец", "мать", "родитель"),
        (2, 0) => pick(b, "дед", "бабушка", "дед/бабушка"),
        (n, 0) => format!("{}{}", pra(n), pick(b, "дед", "бабушка", "дед/бабушка")),
        (1, 1) => {
            if bl.half() {
                let kind = if bl.anc_sex == Sex::Female {
                    "единоутробный"
                } else {
                    "единокровный"
                };
                let kind_f = if bl.anc_sex == Sex::Female {
                    "единоутробная"
                } else {
                    "единокровная"
                };
                match b {
                    Sex::Female => format!("{} сестра", kind_f),
                    _ => format!("{} брат", kind),
                }
            } else {
                pick(b, "брат", "сестра", "брат/сестра")
            }
        }
        (1, 2) => pick(b, "племянник", "племянница", "племянник/племянница"),
        (1, n) => format!(
            "{}{}",
            "внучатый ".repeat(n - 2),
            pick(b, "племянник", "племянница", "племянник/племянница")
        ),
        (2, 1) => pick(b, "дядя", "тётя", "дядя/тётя"),
        (n, 1) => format!(
            "{}{}",
            "двоюродный ".repeat(n - 2),
            pick(b, "дядя", "тётя", "дядя/тётя")
        ),
        (u, d) => {
            let degree = u.min(d) - 1;
            let prefix = match degree {
                1 => "двоюродный",
                2 => "троюродный",
                3 => "четвероюродный",
                _ => "дальний",
            };
            let prefix_f = match degree {
                1 => "двоюродная",
                2 => "троюродная",
                3 => "четвероюродная",
                _ => "дальняя",
            };
            let word = match b {
                Sex::Female => format!("{} сестра", prefix_f),
                _ => format!("{} брат", prefix),
            };
            if u != d {
                format!("{} (разница в {} поколений)", word, u.abs_diff(d))
            } else {
                word
            }
        }
    }
}

// ---------------- Arabic ----------------

fn ar(bl: &Blood, b: Sex) -> String {
    let (u, d) = (bl.up, bl.down);
    let side_male = bl.side == Sex::Male;
    match (u, d) {
        (0, 1) => pick(b, "ابن", "ابنة", "ابن/ابنة"),
        (0, 2) => pick(b, "حفيد", "حفيدة", "حفيد"),
        (0, n) => format!("حفيد (الجيل {})", n),
        (1, 0) => pick(b, "أب", "أم", "والد"),
        (2, 0) => pick(b, "جد", "جدة", "جد"),
        (n, 0) => format!("جد (الجيل {})", n),
        (1, 1) => {
            if bl.half() {
                let w = pick(b, "أخ", "أخت", "أخ/أخت");
                if bl.anc_sex == Sex::Female {
                    format!("{} لأم", w)
                } else {
                    format!("{} لأب", w)
                }
            } else {
                pick(b, "أخ", "أخت", "أخ/أخت")
            }
        }
        (1, 2) => {
            let by = if bl.b_below == Sex::Female {
                "أخت"
            } else {
                "أخ"
            };
            format!("{} {}", pick(b, "ابن", "ابنة", "ابن"), by)
        }
        (1, n) => format!("{} (الجيل {})", pick(b, "ابن أخ", "ابنة أخ", "ابن أخ"), n),
        (2, 1) => match (side_male, b) {
            (true, Sex::Male) => "عم".into(),
            (true, Sex::Female) => "عمة".into(),
            (false, Sex::Male) => "خال".into(),
            (false, Sex::Female) => "خالة".into(),
            (true, _) => "عم/عمة".into(),
            (false, _) => "خال/خالة".into(),
        },
        (n, 1) => format!("{} (الجيل {})", if side_male { "عم" } else { "خال" }, n),
        (2, 2) => {
            let relative = match (side_male, bl.b_below) {
                (true, Sex::Female) => "عمة",
                (true, _) => "عم",
                (false, Sex::Female) => "خالة",
                (false, _) => "خال",
            };
            format!("{} {}", pick(b, "ابن", "ابنة", "ابن"), relative)
        }
        (u, d) => format!(
            "قريب من الدرجة {} ({} جيل فرق)",
            u.min(d) - 1,
            u.abs_diff(d)
        ),
    }
}

/// Describe every route between two people, closest first, in the given language.
pub fn describe_all(rels: &[Relation], lang: Lang) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in rels {
        let s = describe(r, lang);
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}
