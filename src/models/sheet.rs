//! Calendar-grid model for the record sheet: a Monday-first row grid
//! starting at a given date, running for a fixed number of weeks ("to the
//! end of the page"), with row groups labelling month extents.
use jiff::{
    civil::{Date, Weekday},
    ToSpan,
};

/// Number of days in a week row.
pub const WEEK_LENGTH: usize = 7;

/// One row of the calendar grid: a week, laid out Monday-first.
///
/// `None` marks cells before the sheet's start date (the grid "starts at the
/// date given to the CLI"). Days after the start always continue across
/// month boundaries until the week count is exhausted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Week {
    /// Exactly 7 entries, Monday (index 0) ..= Sunday (index 6).
    pub days: [Option<Date>; WEEK_LENGTH],
}

impl Week {
    fn of(date: Date) -> Week {
        let offset = date.weekday().to_monday_zero_offset() as i16;
        let monday = date - offset.days();
        let mut days = [None; WEEK_LENGTH];
        for (day_offset, slot) in days.iter_mut().enumerate() {
            *slot = Some(monday + (day_offset as i16).days());
        }
        Week { days }
    }
}

/// A position in the calendar grid: row index into `Sheet::rows`, column
/// within that row (0 = Monday).
///
/// The row index may be negative: a virtual position before the grid,
/// used when a month starts before the first grid week. Renderers must
/// handle such rows (e.g. by clamping to the grid's top edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub row: isize,
    pub column: usize,
}

/// How a group of rows is labelled/boxed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowGroupKind {
    /// A calendar month, 0 = January (index into `Sheet::year.months`).
    Month(i8),
}

/// A group spanning a range of cells across rows, anchored at a start and
/// end `Position` (both inclusive). A group may start before the grid at a
/// virtual (negative-row) position when its month begins before the first
/// grid week; `end` is always inside the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowGroup {
    pub kind: RowGroupKind,
    pub start: Position,
    pub end: Position,
}

/// One row of the calendar grid. Currently a week of day cells or the
/// weekday-abbreviation header; more kinds may follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// A week of day cells, Monday-first.
    Week(Week),
    /// Column headings spanning one grid row.
    WeekHeader(WeekHeader),
}

/// The language of all generated text: weekday headings and month
/// names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    German,
    Spanish,
}

impl Language {
    /// The weekday abbreviations for this language, Monday (index 0)
    /// ..= Sunday (index 6).
    pub fn weekday_labels(&self) -> [&'static str; WEEK_LENGTH] {
        match self {
            Language::English => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "So"],
            Language::German => ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
            Language::Spanish => ["Lu", "Ma", "Mi", "Ju", "Vi", "Sa", "Do"],
        }
    }
}

/// Weekday column headings for the grid's first row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeekHeader {
    /// One abbreviation per weekday column, Monday (index 0) ..= Sunday
    /// (index 6).
    pub labels: [&'static str; WEEK_LENGTH],
}

impl WeekHeader {
    /// The weekday headings for `language`.
    pub fn for_language(language: Language) -> WeekHeader {
        WeekHeader {
            labels: language.weekday_labels(),
        }
    }
}

impl Default for WeekHeader {
    fn default() -> Self {
        WeekHeader::for_language(Language::default())
    }
}

/// The full sheet: `rows` grid rows starting at the week containing
/// `start`, with days before `start` blanked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// The first printed date; days before it are blank.
    pub start: Date,
    /// Rows of the grid, top to bottom: row 0 is the [`WeekHeader`],
    /// followed by one [`Row::Week`] per week, oldest first.
    pub rows: Vec<Row>,
    /// One group per month whose 1st appears in the grid, plus the month
    /// of `start` when its 1st lies before the grid. The rendered
    /// name is looked up in `year` by the number of cells the month spans
    /// (see [`Month`]), rotated in the margin beside the grid, so every
    /// 1st gets a group regardless of its weekday column.
    pub groups: Vec<RowGroup>,
    /// Month-name variants per calendar month; the rendered name of a
    /// group depends on the cells it spans.
    pub year: Year,
}

impl Sheet {
    /// Build a sheet of `week_count` week rows starting at the week that
    /// contains `start`, with weekday headings and month names in
    /// `language`.
    pub fn new(start: Date, week_count: usize, language: Language) -> Sheet {
        let mut sheet = Sheet {
            start,
            rows: Vec::with_capacity(week_count + 1),
            groups: Vec::new(),
            year: Year::for_language(language),
        };
        sheet
            .rows
            .push(Row::WeekHeader(WeekHeader::for_language(language)));
        sheet.append_weeks(week_count);
        sheet
    }

    /// Append `count` more week rows to the end of the sheet.
    pub fn append_weeks(&mut self, count: usize) {
        for _ in 0..count {
            self.append_week();
        }
    }
    /// Append one more week row, continuing from the last row. Days
    /// before `start` stay blank; month groups are kept in sync.
    pub fn append_week(&mut self) {
        let row = self.rows.len();
        let monday = monday_of(self.start) + (((row - 1) * WEEK_LENGTH) as i16).days();
        let mut week = Week::of(monday);
        // Blank out days before the start date in the first week.
        for slot in week.days.iter_mut() {
            if let Some(date) = *slot {
                if date < self.start {
                    *slot = None;
                }
            }
        }
        self.rows.push(Row::Week(week));
        self.rebuild_groups();
    }
    /// Last visible grid position whose date is in `date`'s month, if any.
    fn last_visible_in_month(&self, date: Date) -> Option<Position> {
        let mut end = None;
        for (row, row_kind) in self.rows.iter().enumerate() {
            let Row::Week(week) = row_kind else { continue };
            for (column, day) in week.days.iter().enumerate() {
                if let Some(candidate) = day {
                    if (candidate.year(), candidate.month()) == (date.year(), date.month()) {
                        end = Some(Position {
                            row: row as isize,
                            column,
                        });
                    }
                }
            }
        }
        end
    }

    /// Recompute `groups` from the grid: one group per month whose 1st
    /// appears in the grid, spanning to the month's last visible day.
    fn rebuild_groups(&mut self) {
        self.groups.clear();
        // The month of `start` when its 1st is not visible in the grid
        // (all days before `start` are blanked): anchor the group at the
        // virtual position where the 1st would sit. Row arithmetic extends
        // the week-row sequence backwards; row 0 is the header band, so a
        // 1st in the week directly before the grid lands on row 0 and
        // earlier weeks go negative.
        let first = self.start.first_of_month();
        if first < self.start {
            let weeks_back = (monday_of(self.start) - monday_of(first)).get_days() as isize / 7;
            if let Some(end) = self.last_visible_in_month(first) {
                self.groups.push(RowGroup {
                    kind: RowGroupKind::Month(first.month() - 1),
                    start: Position {
                        row: 1 - weeks_back,
                        column: weekday_column(first),
                    },
                    end,
                });
            }
        }
        for (row, row_kind) in self.rows.iter().enumerate() {
            let Row::Week(week) = row_kind else { continue };
            for (column, day) in week.days.iter().enumerate() {
                let Some(date) = day else { continue };
                if date.day() != 1 {
                    continue;
                }
                self.groups.push(RowGroup {
                    kind: RowGroupKind::Month(date.month() - 1),
                    start: Position {
                        row: row as isize,
                        column,
                    },
                    // Always `Some`: the 1st itself is visible.
                    end: self
                        .last_visible_in_month(*date)
                        .expect("a visible 1st has a visible month end"),
                });
            }
        }
    }
}

/// Name variants for one month, keyed by available cell count.
///
/// Entries are `(limit, name)` pairs, sorted ascending by `limit`: `name`
/// applies when the month spans more than `limit` cells, and the largest
/// applicable `limit` wins. With no applicable entry the month gets no
/// text (a single cell shows nothing for the English defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Month {
    names: &'static [(u8, &'static str)],
}

impl Month {
    /// Build a month from `(limit, name)` entries sorted ascending by
    /// `limit`.
    pub const fn new(names: &'static [(u8, &'static str)]) -> Month {
        Month { names }
    }

    /// The name to render for a label box `cells` row-units tall, if
    /// any. `cells` is continuous (half rows possible when a month
    /// starts or ends mid-cell); an entry applies when `cells` is
    /// strictly greater than its limit.
    pub fn name(&self, cells: f32) -> Option<&'static str> {
        self.names
            .iter()
            .rev()
            .find(|(limit, _)| cells > f32::from(*limit))
            .map(|(_, name)| *name)
    }
}

/// The twelve months of a year.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Year {
    /// January (index 0) through December (index 11).
    pub months: Vec<Month>,
}

impl Year {
    /// The month names for `language`: no text for a single cell, the
    /// short name from two cells, the full name from three.
    pub fn for_language(language: Language) -> Year {
        let names: [&[(u8, &str)]; 12] = match language {
            Language::English => [
                &[(1, "Jan"), (2, "January")],
                &[(1, "Feb"), (2, "February")],
                &[(1, "Mar"), (2, "March")],
                &[(1, "Apr"), (2, "April")],
                &[(1, "May")],
                &[(1, "Jun"), (2, "June")],
                &[(1, "Jul"), (2, "July")],
                &[(1, "Aug"), (2, "August")],
                &[(1, "Sep"), (2, "September")],
                &[(1, "Oct"), (2, "October")],
                &[(1, "Nov"), (2, "November")],
                &[(1, "Dec"), (2, "December")],
            ],
            Language::German => [
                &[(1, "Jan"), (2, "Januar")],
                &[(1, "Feb"), (2, "Februar")],
                &[(1, "Mär"), (2, "März")],
                &[(1, "Apr"), (2, "April")],
                &[(1, "Mai")],
                &[(1, "Jun"), (2, "Juni")],
                &[(1, "Jul"), (2, "Juli")],
                &[(1, "Aug"), (2, "August")],
                &[(1, "Sep"), (2, "September")],
                &[(1, "Okt"), (2, "Oktober")],
                &[(1, "Nov"), (2, "November")],
                &[(1, "Dez"), (2, "Dezember")],
            ],
            Language::Spanish => [
                &[(1, "Ene"), (2, "Enero")],
                &[(1, "Feb"), (2, "Febrero")],
                &[(1, "Mar"), (2, "Marzo")],
                &[(1, "Abr"), (2, "Abril")],
                &[(1, "May")],
                &[(1, "Jun"), (2, "Junio")],
                &[(1, "Jul"), (2, "Julio")],
                &[(1, "Ago"), (2, "Agosto")],
                &[(1, "Sep"), (2, "Septiembre")],
                &[(1, "Oct"), (2, "Octubre")],
                &[(1, "Nov"), (2, "Noviembre")],
                &[(1, "Dic"), (2, "Diciembre")],
            ],
        };
        Year {
            months: names.iter().map(|names| Month::new(names)).collect(),
        }
    }
}

impl Default for Year {
    fn default() -> Self {
        Year::for_language(Language::default())
    }
}

/// Weekday column (0 = Monday) of a date, for tests and layout.
pub fn weekday_column(date: Date) -> usize {
    date.weekday().to_monday_zero_offset() as usize
}

/// The Monday of the week containing `date`.
pub fn monday_of(date: Date) -> Date {
    let offset = date.weekday().to_monday_zero_offset() as i16;
    date - offset.days()
}

/// Is `weekday` a weekend column (Saturday/Sunday)?
pub fn is_weekend(weekday: Weekday) -> bool {
    matches!(weekday, Weekday::Saturday | Weekday::Sunday)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn week(sheet: &Sheet, row: usize) -> &Week {
        let Row::Week(week) = &sheet.rows[row] else {
            panic!("row {row} is not a week");
        };
        week
    }

    #[test]
    fn weeks_start_at_given_date_midweek() {
        // 2026-09-15 is a Tuesday.
        let sheet = Sheet::new(date(2026, 9, 15), 1, Language::English);
        assert_eq!(sheet.rows.len(), 2);
        let week = &week(&sheet, 1).days;
        // Monday 2026-09-14 is before the start -> blank.
        assert_eq!(week[0], None);
        assert_eq!(week[1], Some(date(2026, 9, 15)));
        assert_eq!(week[6], Some(date(2026, 9, 20)));
    }

    #[test]
    fn weeks_run_across_month_boundary_to_fill_page() {
        // 2026-09-15 + 6 weeks crosses into October.
        let sheet = Sheet::new(date(2026, 9, 15), 6, Language::English);
        assert_eq!(sheet.rows.len(), 7);
        assert_eq!(week(&sheet, 6).days[6], Some(date(2026, 10, 25)));
    }

    #[test]
    fn label_on_first_of_month() {
        // 2026-10-01 is a Thursday.
        let sheet = Sheet::new(date(2026, 9, 15), 6, Language::English);
        assert_eq!(
            sheet.groups,
            vec![
                RowGroup {
                    kind: RowGroupKind::Month(8),
                    start: Position { row: -1, column: 1 },
                    end: Position { row: 3, column: 2 },
                },
                RowGroup {
                    kind: RowGroupKind::Month(9),
                    start: Position { row: 3, column: 3 },
                    end: Position { row: 6, column: 6 },
                },
            ]
        );
    }
    #[test]
    fn every_first_of_month_gets_a_label_regardless_of_weekday() {
        // 2026-11-01 is a Sunday; the rotated margin label has no width
        // constraint, so November is labelled too.
        let sheet = Sheet::new(date(2026, 10, 30), 6, Language::English);
        assert_eq!(
            sheet.groups,
            vec![
                RowGroup {
                    kind: RowGroupKind::Month(9),
                    start: Position { row: -3, column: 3 },
                    end: Position { row: 1, column: 5 },
                },
                RowGroup {
                    kind: RowGroupKind::Month(10),
                    start: Position { row: 1, column: 6 },
                    end: Position { row: 6, column: 0 },
                },
                RowGroup {
                    kind: RowGroupKind::Month(11),
                    start: Position { row: 6, column: 1 },
                    end: Position { row: 6, column: 6 },
                },
            ]
        );
    }
    #[test]
    fn start_on_first_of_month_gets_label() {
        // 2026-09-01 is a Tuesday.
        let sheet = Sheet::new(date(2026, 9, 1), 1, Language::English);
        assert_eq!(
            sheet.groups,
            vec![RowGroup {
                kind: RowGroupKind::Month(8),
                start: Position { row: 1, column: 1 },
                end: Position { row: 1, column: 6 },
            }]
        );
    }
    #[test]
    fn append_week_extends_grid() {
        // 2026-09-15 is a Tuesday; 3 appended weeks continue the grid.
        let mut sheet = Sheet::new(date(2026, 9, 15), 1, Language::English);
        sheet.append_week();
        sheet.append_week();
        sheet.append_week();
        assert_eq!(sheet.rows.len(), 5);
        assert_eq!(week(&sheet, 4).days[0], Some(date(2026, 10, 5)));
        assert_eq!(week(&sheet, 4).days[6], Some(date(2026, 10, 11)));
        // Row 1 is unchanged: Monday still blanked before the start.
        assert_eq!(week(&sheet, 1).days[0], None);
        assert_eq!(week(&sheet, 1).days[1], Some(date(2026, 9, 15)));
    }

    #[test]
    fn append_weeks_matches_sheet_new() {
        let mut appended = Sheet::new(date(2026, 9, 15), 1, Language::English);
        appended.append_weeks(5);
        assert_eq!(
            appended,
            Sheet::new(date(2026, 9, 15), 6, Language::English)
        );
    }
    #[test]
    fn append_week_updates_labels() {
        // 2026-09-15 is a Tuesday. The September virtual group exists
        // from the start (anchored before the grid) and its end grows
        // with the grid; appending weeks crosses into October, whose
        // 1st (Thursday) gains a label ending at the grid's edge.
        let mut sheet = Sheet::new(date(2026, 9, 15), 1, Language::English);
        // Only September's virtual group; it ends at the grid edge
        // (2026-09-20, Sunday of row 1).
        assert_eq!(
            sheet.groups,
            vec![RowGroup {
                kind: RowGroupKind::Month(8),
                start: Position { row: -1, column: 1 },
                end: Position { row: 1, column: 6 },
            }]
        );
        sheet.append_week();
        // 09-21..=09-27 appended: September's end moves to Sep 27.
        assert_eq!(
            sheet.groups,
            vec![RowGroup {
                kind: RowGroupKind::Month(8),
                start: Position { row: -1, column: 1 },
                end: Position { row: 2, column: 6 },
            }]
        );
        sheet.append_week();
        // Grid now covers 2026-09-15 ..= 2026-10-04: October appears.
        assert_eq!(
            sheet.groups,
            vec![
                RowGroup {
                    kind: RowGroupKind::Month(8),
                    start: Position { row: -1, column: 1 },
                    end: Position { row: 3, column: 2 },
                },
                RowGroup {
                    kind: RowGroupKind::Month(9),
                    start: Position { row: 3, column: 3 },
                    end: Position { row: 3, column: 6 },
                },
            ]
        );
    }

    #[test]
    fn append_weeks_keeps_month_label_end_open() {
        // 2026-10-01 mid-sheet: appending weeks must move October's
        // end_row forward.
        let mut sheet = Sheet::new(date(2026, 9, 1), 2, Language::English);
        let before = sheet.groups[0].end.row;
        sheet.append_weeks(3);
        let group = sheet
            .groups
            .iter()
            .find(|group| matches!(group.kind, RowGroupKind::Month(9)))
            .unwrap();
        assert!(group.end.row > before);
        assert_eq!(group.end.row, sheet.rows.len() as isize - 1);
    }

    #[test]
    fn weekday_header_comes_from_model() {
        let sheet = Sheet::new(date(2026, 9, 15), 1, Language::English);
        assert_eq!(
            sheet.rows[0],
            Row::WeekHeader(WeekHeader::for_language(Language::English)),
            "row 0 is the weekday-abbreviation header"
        );
    }

    #[test]
    fn weekday_header_follows_language() {
        assert_eq!(
            WeekHeader::for_language(Language::German).labels,
            ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"]
        );
        assert_eq!(
            WeekHeader::for_language(Language::Spanish).labels,
            ["Lu", "Ma", "Mi", "Ju", "Vi", "Sa", "Do"]
        );
    }

    #[test]
    fn month_names_follow_language() {
        let german = Year::for_language(Language::German);
        assert_eq!(german.months[2].name(2.0), Some("Mär"));
        assert_eq!(german.months[2].name(3.0), Some("März"));
        assert_eq!(german.months[9].name(3.0), Some("Oktober"));
        let spanish = Year::for_language(Language::Spanish);
        assert_eq!(spanish.months[0].name(2.0), Some("Ene"));
        assert_eq!(spanish.months[0].name(3.0), Some("Enero"));
        assert_eq!(spanish.months[4].name(2.0), Some("May"));
    }

    #[test]
    fn sheet_carries_language() {
        let sheet = Sheet::new(date(2026, 9, 15), 1, Language::German);
        assert_eq!(
            sheet.rows[0],
            Row::WeekHeader(WeekHeader::for_language(Language::German))
        );
        assert_eq!(sheet.year.months[11].name(3.0), Some("Dezember"));
    }

    #[test]
    fn monday_of_midweek_date() {
        assert_eq!(monday_of(date(2026, 9, 15)), date(2026, 9, 14));
        assert_eq!(monday_of(date(2026, 9, 14)), date(2026, 9, 14));
    }

    #[test]
    fn month_name_by_cells() {
        let january = Year::for_language(Language::English).months[0];
        assert_eq!(january.name(1.0), None);
        assert_eq!(january.name(2.0), Some("Jan"));
        assert_eq!(january.name(3.0), Some("January"));
        assert_eq!(january.name(4.0), Some("January"));
    }

    #[test]
    fn month_name_biggest_entry_wins() {
        // The user's example: entries at 1, 3, 4; the biggest applicable
        // limit wins, and exactly `limit` cells get nothing.
        let december = Month::new(&[(1, "Dec"), (3, "Decem"), (4, "Decembre")]);
        assert_eq!(december.name(1.0), None);
        assert_eq!(december.name(2.0), Some("Dec"));
        assert_eq!(december.name(3.0), Some("Dec"));
        assert_eq!(december.name(4.0), Some("Decem"));
        assert_eq!(december.name(5.0), Some("Decembre"));
    }

    #[test]
    fn month_single_entry() {
        // The user's May example: `May = { 1 = "May" }` — a single cell
        // stays empty, from two cells the name shows.
        let may = Month::new(&[(1, "May")]);
        assert_eq!(may.name(1.0), None);
        assert_eq!(may.name(2.0), Some("May"));
    }

    #[test]
    fn month_name_grows_with_appended_weeks() {
        // October spans 4 cells in a 6-week sheet: full name. In a 1-week
        // sheet crossing into October (4 cells, end at grid edge) it
        // still gets its full name; a shorter variant applies only when
        // the span is small.
        let sheet = Sheet::new(date(2026, 9, 15), 6, Language::English);
        assert_eq!(sheet.year.months[9].name(4.0), Some("October"));
        let mut small = Sheet::new(date(2026, 9, 29), 1, Language::English);
        small.append_week();
        // October spans 2026-10-01..=10-04 -> 4 cells.
        assert_eq!(small.year.months[9].name(4.0), Some("October"));
    }

    #[test]
    fn virtual_group_anchors_blank_first_week_cell() {
        // 2026-09-03 is a Thursday; the 1st (Sep 1, Tuesday) sits in a
        // blanked cell of the first week row: a non-negative virtual
        // anchor on that blank cell.
        let sheet = Sheet::new(date(2026, 9, 3), 1, Language::English);
        assert_eq!(
            sheet.groups,
            vec![RowGroup {
                kind: RowGroupKind::Month(8),
                start: Position { row: 1, column: 1 },
                end: Position { row: 1, column: 6 },
            }]
        );
    }

    #[test]
    fn virtual_group_absent_without_week_rows() {
        // No week rows -> no visible days -> no virtual group.
        let sheet = Sheet::new(date(2026, 9, 15), 0, Language::English);
        assert!(sheet.groups.is_empty());
    }
}
