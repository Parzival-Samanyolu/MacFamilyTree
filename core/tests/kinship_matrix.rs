use kintree_core::kinship_terms::*;
use kintree_core::relationship::*;

const LANGS: [(&str, Lang); 7] = [
    ("en", Lang::En),
    ("tr", Lang::Tr),
    ("de", Lang::De),
    ("es", Lang::Es),
    ("fr", Lang::Fr),
    ("ru", Lang::Ru),
    ("ar", Lang::Ar),
];
const SEXES: [Sex; 3] = [Sex::Male, Sex::Female, Sex::Unknown];

fn blood(
    up: usize,
    down: usize,
    side: Sex,
    anc_sex: Sex,
    b_below: Sex,
    link: LinkKind,
    couple: bool,
) -> Blood {
    let ancestors = if couple {
        vec!["x".to_string(), "y".to_string()]
    } else {
        vec!["x".to_string()]
    };
    Blood {
        up,
        down,
        ancestors,
        side,
        link,
        anc_sex,
        b_below,
    }
}
fn rel(kind: Kind, a: Sex, b: Sex) -> Relation {
    Relation {
        kind,
        a_sex: a,
        b_sex: b,
    }
}

#[test]
fn every_language_names_every_blood_relation_without_gaps() {
    let mut n = 0;
    for (code, lang) in LANGS {
        assert_eq!(Lang::from_code(code), lang);
        for up in 0..=6 {
            for down in 0..=6 {
                if up == 0 && down == 0 {
                    continue;
                }
                for side in SEXES {
                    for b in SEXES {
                        for link in [
                            LinkKind::Biological,
                            LinkKind::Adopted,
                            LinkKind::Foster,
                            LinkKind::Step,
                        ] {
                            for couple in [true, false] {
                                let r = rel(
                                    Kind::Blood(blood(up, down, side, side, b, link, couple)),
                                    Sex::Male,
                                    b,
                                );
                                let s = describe(&r, lang);
                                assert!(!s.trim().is_empty(), "{code} {up}/{down}");
                                assert!(
                                    !s.contains('{') && !s.contains("(en)"),
                                    "{code} {up}/{down}: {s}"
                                );
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(n > 10_000);
}

#[test]
fn compound_and_special_relations_are_described_in_all_languages() {
    let b = |u, d| {
        blood(
            u,
            d,
            Sex::Female,
            Sex::Male,
            Sex::Male,
            LinkKind::Biological,
            true,
        )
    };
    for (code, lang) in LANGS {
        let kinds = vec![
            Kind::Same,
            Kind::Unrelated,
            Kind::Spouse,
            Kind::StepChild,
            Kind::StepParent,
            Kind::SpouseRelative {
                via: "s".into(),
                blood: b(1, 0),
            },
            Kind::SpouseRelative {
                via: "s".into(),
                blood: b(1, 1),
            },
            Kind::SpouseRelative {
                via: "s".into(),
                blood: b(0, 1),
            },
            Kind::SpouseRelative {
                via: "s".into(),
                blood: b(2, 2),
            },
            Kind::RelativeSpouse {
                via: "r".into(),
                blood: b(0, 1),
            },
            Kind::RelativeSpouse {
                via: "r".into(),
                blood: b(1, 1),
            },
            Kind::RelativeSpouse {
                via: "r".into(),
                blood: b(2, 1),
            },
            Kind::SpouseRelativeSpouse {
                spouse: "s".into(),
                via: "r".into(),
                blood: b(1, 1),
            },
            Kind::SpouseRelativeSpouse {
                spouse: "s".into(),
                via: "r".into(),
                blood: b(3, 3),
            },
        ];
        for k in kinds {
            for (a, bs) in [
                (Sex::Male, Sex::Female),
                (Sex::Female, Sex::Male),
                (Sex::Unknown, Sex::Unknown),
            ] {
                let s = describe(&rel(k.clone(), a, bs), lang);
                assert!(
                    !s.trim().is_empty() && !s.contains('{'),
                    "{code} {k:?}: {s}"
                );
            }
        }
        assert!(!blood_name(&b(2, 3), Sex::Male, Sex::Female, lang).is_empty());
    }
    let all = describe_all(
        &[
            rel(Kind::Spouse, Sex::Male, Sex::Female),
            rel(Kind::Blood(b(1, 0)), Sex::Male, Sex::Male),
        ],
        Lang::En,
    );
    assert_eq!(all.len(), 2);
}

type Spec = (usize, usize, Sex, Sex, Sex, Sex);

fn one(up: usize, down: usize, side: Sex, anc: Sex, below: Sex, b: Sex, lang: Lang) -> String {
    describe(
        &rel(
            Kind::Blood(blood(
                up,
                down,
                side,
                anc,
                below,
                LinkKind::Biological,
                true,
            )),
            Sex::Male,
            b,
        ),
        lang,
    )
}

#[test]
fn well_known_terms_are_correct_per_language() {
    use Sex::*;
    // (up, down, side, anc_sex, b_below, b) → expected [en, tr, de, es, fr, ru, ar]
    let cases: &[(Spec, [&str; 7])] = &[
        (
            (1, 0, Male, Male, Male, Male),
            ["father", "baba", "Vater", "padre", "père", "отец", "أب"],
        ),
        (
            (1, 0, Male, Female, Female, Female),
            ["mother", "anne", "Mutter", "madre", "mère", "мать", "أم"],
        ),
        (
            (0, 1, Male, Male, Male, Male),
            ["son", "oğul", "Sohn", "hijo", "fils", "сын", "ابن"],
        ),
        (
            (0, 1, Male, Female, Female, Female),
            [
                "daughter", "kız", "Tochter", "hija", "fille", "дочь", "ابنة",
            ],
        ),
        (
            (1, 1, Male, Male, Male, Male),
            [
                "brother",
                "erkek kardeş",
                "Bruder",
                "hermano",
                "frère",
                "брат",
                "أخ",
            ],
        ),
        (
            (1, 1, Male, Male, Female, Female),
            [
                "sister",
                "kız kardeş",
                "Schwester",
                "hermana",
                "sœur",
                "сестра",
                "أخت",
            ],
        ),
        (
            (2, 0, Male, Male, Male, Male),
            [
                "grandfather",
                "dede",
                "Großvater",
                "abuelo",
                "grand-père",
                "дед",
                "جد",
            ],
        ),
        (
            (0, 2, Male, Female, Female, Female),
            [
                "granddaughter",
                "torun",
                "Enkelin",
                "nieta",
                "petite-fille",
                "внучка",
                "حفيدة",
            ],
        ),
        (
            (2, 1, Male, Male, Male, Male),
            ["uncle", "amca", "Onkel", "tío", "oncle", "дядя", "عم"],
        ),
        (
            (2, 1, Female, Female, Female, Female),
            ["aunt", "teyze", "Tante", "tía", "tante", "тётя", "خالة"],
        ),
        (
            (1, 2, Male, Male, Male, Male),
            [
                "nephew",
                "yeğen",
                "Neffe",
                "sobrino",
                "neveu",
                "племянник",
                "ابن أخ",
            ],
        ),
        (
            (3, 0, Female, Female, Female, Female),
            [
                "great-grandmother",
                "büyük nine",
                "Urgroßmutter",
                "bisabuela",
                "arrière-grand-mère",
                "прабабушка",
                "جدة (الجيل 3)",
            ],
        ),
        (
            (0, 4, Male, Male, Male, Male),
            [
                "great-great-grandson",
                "4. kuşak torun",
                "UrUrEnkel",
                "tataranieto",
                "arrière-arrière-petit-fils",
                "праправнук",
                "حفيد (الجيل 4)",
            ],
        ),
    ];
    for ((u, d, side, anc, below, b), expect) in cases {
        for (i, (code, lang)) in LANGS.iter().enumerate() {
            assert_eq!(
                one(*u, *d, *side, *anc, *below, *b, *lang),
                expect[i],
                "{code} {u}/{d}"
            );
        }
    }
}

#[test]
fn removed_cousins_use_correct_plural_agreement() {
    let removed = |n: usize, lang| {
        one(
            2 + n,
            2,
            Sex::Male,
            Sex::Male,
            Sex::Female,
            Sex::Female,
            lang,
        )
    };
    assert!(removed(1, Lang::Ru).ends_with("(разница в 1 поколение)"));
    assert!(removed(2, Lang::Ru).ends_with("(разница в 2 поколения)"));
    assert!(removed(5, Lang::Ru).ends_with("(разница в 5 поколений)"));
    assert!(removed(11, Lang::Ru).ends_with("(разница в 11 поколений)"));
    assert!(removed(1, Lang::De).ends_with("1 Generation versetzt"));
    assert!(removed(2, Lang::De).ends_with("2 Generationen versetzt"));
    assert!(removed(1, Lang::Es).contains("1 generación de"));
    assert!(removed(2, Lang::Es).contains("2 generaciones de"));
    assert!(removed(2, Lang::Fr).contains("2 générations d'écart"));
    assert!(!one(
        4,
        4,
        Sex::Male,
        Sex::Male,
        Sex::Female,
        Sex::Female,
        Lang::Ar
    )
    .contains("جيل فرق"));
}
