//! Calendar-grid model for the record sheet: a Monday-first week grid
//! starting at a given date, running for a fixed number of weeks ("to the
//! end of the page"), with month-name label placement.
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

/// A month's extent on the sheet: anchored at the day cell holding the 1st,
/// running to the last visible day of that month.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthLabel {
    /// Index into `Sheet::weeks` of the week holding the 1st.
    pub row: usize,
    /// Column of the 1st within that week (0 = Monday).
    pub column: usize,
    /// Which calendar month this labels, 0 = January (index into
    /// `Sheet::year.months`).
    pub month: i8,
    /// Index into `Sheet::weeks` of the week holding the month's last
    /// visible day.
    pub end_row: usize,
    /// Column of the month's last visible day within its week.
    pub end_column: usize,
}

/// The full sheet: `weeks` week rows starting at the week containing
/// `start`, with days before `start` blanked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// The first printed date; days before it are blank.
    pub start: Date,
    /// Week rows, Monday-first, oldest first.
    pub weeks: Vec<Week>,
    /// One label per month whose 1st appears in the grid. The rendered
    /// name is looked up in `year` by the number of cells the month spans
    /// (see [`Month`]), rotated in the margin beside the grid, so every
    /// 1st gets a label regardless of its weekday column.
    pub labels: Vec<MonthLabel>,
    /// Month-name variants per calendar month; the rendered name of a
    /// label depends on the cells it spans.
    pub year: Year,
}

impl Sheet {
    /// Build a sheet of `week_count` week rows starting at the week that
    /// contains `start`.
    pub fn new(start: Date, week_count: usize) -> Sheet {
        let mut sheet = Sheet {
            start,
            weeks: Vec::with_capacity(week_count),
            labels: Vec::new(),
            year: Year::default(),
        };
        sheet.append_weeks(week_count);
        sheet
    }

    /// Append `count` more week rows to the end of the sheet.
    pub fn append_weeks(&mut self, count: usize) {
        for _ in 0..count {
            self.append_week();
        }
    }

    /// Append one more week row, continuing from the last row (or the
    /// week containing `start` for an empty sheet). Days before `start`
    /// stay blank; month labels are kept in sync.
    pub fn append_week(&mut self) {
        let row = self.weeks.len();
        let monday = monday_of(self.start) + ((row * WEEK_LENGTH) as i16).days();
        let mut week = Week::of(monday);
        // Blank out days before the start date in the first week.
        for slot in week.days.iter_mut() {
            if let Some(date) = *slot {
                if date < self.start {
                    *slot = None;
                }
            }
        }
        self.weeks.push(week);
        self.rebuild_labels();
    }

    /// Recompute `labels` from the grid: one label per month whose 1st
    /// appears in the grid, spanning to the month's last visible day.
    fn rebuild_labels(&mut self) {
        self.labels.clear();
        for (row, week) in self.weeks.iter().enumerate() {
            for (column, day) in week.days.iter().enumerate() {
                let Some(date) = day else { continue };
                if date.day() != 1 {
                    continue;
                }
                // Find the last visible day of the same month.
                let mut end_row = row;
                let mut end_column = column;
                for (candidate_row, candidate_week) in self.weeks.iter().enumerate().skip(row) {
                    for (candidate_column, candidate_day) in candidate_week.days.iter().enumerate()
                    {
                        if let Some(candidate_date) = candidate_day {
                            if (candidate_date.year(), candidate_date.month())
                                == (date.year(), date.month())
                            {
                                end_row = candidate_row;
                                end_column = candidate_column;
                            }
                        }
                    }
                }
                self.labels.push(MonthLabel {
                    row,
                    column,
                    month: date.month() - 1,
                    end_row,
                    end_column,
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
    /// English month names: no text for a single cell, the short name
    /// from two cells, the full name from three.
    pub fn english() -> Year {
        const ENGLISH: [&[(u8, &str)]; 12] = [
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
        ];
        Year {
            months: ENGLISH.iter().map(|names| Month::new(names)).collect(),
        }
    }
}

impl Default for Year {
    fn default() -> Self {
        Year::english()
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

    #[test]
    fn weeks_start_at_given_date_midweek() {
        // 2026-09-15 is a Tuesday.
        let sheet = Sheet::new(date(2026, 9, 15), 1);
        assert_eq!(sheet.weeks.len(), 1);
        let week = &sheet.weeks[0].days;
        // Monday 2026-09-14 is before the start -> blank.
        assert_eq!(week[0], None);
        assert_eq!(week[1], Some(date(2026, 9, 15)));
        assert_eq!(week[6], Some(date(2026, 9, 20)));
    }

    #[test]
    fn weeks_run_across_month_boundary_to_fill_page() {
        // 2026-09-15 + 6 weeks crosses into October.
        let sheet = Sheet::new(date(2026, 9, 15), 6);
        assert_eq!(sheet.weeks.len(), 6);
        assert_eq!(sheet.weeks[5].days[6], Some(date(2026, 10, 25)));
    }

    #[test]
    fn label_on_first_of_month() {
        // 2026-10-01 is a Thursday.
        let sheet = Sheet::new(date(2026, 9, 15), 6);
        assert_eq!(
            sheet.labels,
            vec![MonthLabel {
                row: 2,
                column: 3,
                month: 9,
                end_row: 5,
                end_column: 6,
            }]
        );
    }
    #[test]
    fn every_first_of_month_gets_a_label_regardless_of_weekday() {
        // 2026-11-01 is a Sunday; the rotated margin label has no width
        // constraint, so November is labelled too.
        let sheet = Sheet::new(date(2026, 10, 30), 6);
        assert_eq!(
            sheet.labels,
            vec![
                MonthLabel {
                    row: 0,
                    column: 6,
                    month: 10,
                    end_row: 5,
                    end_column: 0,
                },
                MonthLabel {
                    row: 5,
                    column: 1,
                    month: 11,
                    end_row: 5,
                    end_column: 6,
                },
            ]
        );
    }
    #[test]
    fn start_on_first_of_month_gets_label() {
        // 2026-09-01 is a Tuesday.
        let sheet = Sheet::new(date(2026, 9, 1), 1);
        assert_eq!(
            sheet.labels,
            vec![MonthLabel {
                row: 0,
                column: 1,
                month: 8,
                end_row: 0,
                end_column: 6,
            }]
        );
    }
    #[test]
    fn append_week_extends_grid() {
        // 2026-09-15 is a Tuesday; 3 appended weeks continue the grid.
        let mut sheet = Sheet::new(date(2026, 9, 15), 1);
        sheet.append_week();
        sheet.append_week();
        sheet.append_week();
        assert_eq!(sheet.weeks.len(), 4);
        assert_eq!(sheet.weeks[3].days[0], Some(date(2026, 10, 5)));
        assert_eq!(sheet.weeks[3].days[6], Some(date(2026, 10, 11)));
        // Row 0 is unchanged: Monday still blanked before the start.
        assert_eq!(sheet.weeks[0].days[0], None);
        assert_eq!(sheet.weeks[0].days[1], Some(date(2026, 9, 15)));
    }

    #[test]
    fn append_weeks_matches_sheet_new() {
        let mut appended = Sheet::new(date(2026, 9, 15), 1);
        appended.append_weeks(5);
        assert_eq!(appended, Sheet::new(date(2026, 9, 15), 6));
    }

    #[test]
    fn append_week_updates_labels() {
        // 2026-09-15 is a Tuesday; the initial sheet has no label (no
        // month-1st in 2026-09-15..=09-20). Appending two weeks crosses
        // into October, whose 1st (Thursday) must gain a label that ends
        // at the grid's edge and extends as the grid grows.
        let mut sheet = Sheet::new(date(2026, 9, 15), 1);
        assert!(sheet.labels.is_empty());
        sheet.append_week();
        assert!(sheet.labels.is_empty()); // 09-21..=09-27, still no 1st.
        sheet.append_week();
        // Grid now covers 2026-09-15 ..= 2026-10-04.
        assert_eq!(
            sheet.labels,
            vec![MonthLabel {
                row: 2,
                column: 3,
                month: 9,
                end_row: 2,
                end_column: 6,
            }]
        );
    }

    #[test]
    fn append_weeks_keeps_month_label_end_open() {
        // 2026-10-01 mid-sheet: appending weeks must move October's
        // end_row forward.
        let mut sheet = Sheet::new(date(2026, 9, 1), 2);
        let before = sheet.labels[0].end_row;
        sheet.append_weeks(3);
        let label = sheet
            .labels
            .iter()
            .find(|label| label.month == 9)
            .unwrap();
        assert!(label.end_row > before);
        assert_eq!(label.end_row, sheet.weeks.len() - 1);
    }

    #[test]
    fn monday_of_midweek_date() {
        assert_eq!(monday_of(date(2026, 9, 15)), date(2026, 9, 14));
        assert_eq!(monday_of(date(2026, 9, 14)), date(2026, 9, 14));
    }

    #[test]
    fn month_name_by_cells() {
        let january = Year::english().months[0];
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
        let sheet = Sheet::new(date(2026, 9, 15), 6);
        assert_eq!(sheet.year.months[9].name(4.0), Some("October"));
        let mut small = Sheet::new(date(2026, 9, 29), 1);
        small.append_week();
        // October spans 2026-10-01..=10-04 -> 4 cells.
        assert_eq!(small.year.months[9].name(4.0), Some("October"));
    }
}
