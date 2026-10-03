use kintree_core::date::*;
use proptest::prelude::*;

proptest! {
    #[test]
    fn gregorian_jdn_roundtrip(y in -4000i32..5000, m in 1u8..=12, d in 1u8..=28) {
        prop_assume!(y != 0);
        prop_assert_eq!(jdn_to_gregorian(gregorian_to_jdn(y, m, d)), (y, m, d));
    }

    #[test]
    fn julian_jdn_roundtrip(y in -4000i32..5000, m in 1u8..=12, d in 1u8..=28) {
        prop_assume!(y != 0);
        prop_assert_eq!(jdn_to_julian(julian_to_jdn(y, m, d)), (y, m, d));
    }

    #[test]
    fn consecutive_days_increment_jdn(y in 1i32..3000, m in 1u8..=12) {
        let n = days_in_month(Calendar::Gregorian, y, m);
        for d in 1..n {
            prop_assert_eq!(gregorian_to_jdn(y, m, d) + 1, gregorian_to_jdn(y, m, d + 1));
        }
    }

    #[test]
    fn exact_gedcom_roundtrip(y in 1i32..3000, m in 1u8..=12, d in 1u8..=28, q in 0usize..6) {
        let prefix = ["", "ABT ", "EST ", "CAL ", "BEF ", "AFT "][q];
        let months = ["JAN","FEB","MAR","APR","MAY","JUN","JUL","AUG","SEP","OCT","NOV","DEC"];
        let s = format!("{}{} {} {}", prefix, d, months[(m-1) as usize], y);
        let parsed = GenDate::parse(&s).unwrap();
        prop_assert_eq!(parsed.to_gedcom(), s);
    }

    #[test]
    fn between_range_is_ordered(y1 in 1i32..2000, span in 0i32..200) {
        let s = format!("BET {} AND {}", y1, y1 + span);
        let (lo, hi) = GenDate::parse(&s).unwrap().range().unwrap();
        prop_assert!(lo <= hi);
    }

    #[test]
    fn parser_never_panics(s in ".{0,40}") {
        let _ = GenDate::parse(&s);
        let _ = GenDate::parse_lenient(&s);
    }
}

#[test]
fn hebrew_year_lengths_are_valid() {
    // Every Hebrew year has 353-355 or 383-385 days; consecutive months cover the year exactly.
    for y in 5000..6000 {
        let start = to_jdn(Calendar::Hebrew, y, 7, 1);
        let next = to_jdn(Calendar::Hebrew, y + 1, 7, 1);
        let len = next - start;
        assert!(matches!(len, 353..=355 | 383..=385), "year {y} len {len}");
    }
}
