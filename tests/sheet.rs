//! Snapshot tests for the `Sheet` model: construction and incremental
//! `append_week`/`append_weeks` growth.
//!
//! Snapshots capture the full `Sheet` (row grid plus row groups).
//! After intentional model changes, review with `cargo insta review`
//! (or `cargo insta accept`).

use insta::assert_debug_snapshot;
use jiff::civil::date;
use record_sheet::Sheet;

/// 1: a sheet for a given date contains only the week of that date.
#[test]
fn sheet_for_single_week() {
    assert_debug_snapshot!(Sheet::new(date(2026, 9, 15), 1));
}

/// 2: a sheet for a given date, then append one week.
#[test]
fn sheet_with_one_appended_week() {
    let mut sheet = Sheet::new(date(2026, 9, 15), 1);
    sheet.append_week();
    assert_debug_snapshot!(sheet);
}

/// 3: a sheet for a given date, then append three weeks.
#[test]
fn sheet_with_three_appended_weeks() {
    let mut sheet = Sheet::new(date(2026, 9, 15), 1);
    sheet.append_weeks(3);
    assert_debug_snapshot!(sheet);
}

/// 4: a sheet for a given date spanning ten weeks.
#[test]
fn sheet_for_ten_weeks() {
    assert_debug_snapshot!(Sheet::new(date(2026, 9, 15), 10));
}

/// 5: a sheet for a given date spanning three weeks, then append two weeks.
#[test]
fn sheet_for_three_weeks_with_two_appended() {
    let mut sheet = Sheet::new(date(2026, 9, 15), 3);
    sheet.append_weeks(2);
    assert_debug_snapshot!(sheet);
}
