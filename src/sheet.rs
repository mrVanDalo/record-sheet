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
    /// Full month name, e.g. "September".
    pub name: &'static str,
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
    /// One label per month whose 1st appears in the grid. The rendered name
    /// is rotated in the margin beside the grid, so every 1st gets a label
    /// regardless of its weekday column.
    pub labels: Vec<MonthLabel>,
}

impl Sheet {
    /// Build a sheet of `week_count` week rows starting at the week that
    /// contains `start`.
    pub fn new(start: Date, week_count: usize) -> Sheet {
        let mut sheet = Sheet {
            start,
            weeks: Vec::with_capacity(week_count),
            labels: Vec::new(),
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
                    name: month_name(date.month()),
                    end_row,
                    end_column,
                });
            }
        }
    }
}

/// English month names.
pub fn month_name(month: i8) -> &'static str {
    const NAMES: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    NAMES[(month - 1) as usize]
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
                name: "October",
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
                    name: "November",
                    end_row: 5,
                    end_column: 0,
                },
                MonthLabel {
                    row: 5,
                    column: 1,
                    name: "December",
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
                name: "September",
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
                name: "October",
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
            .find(|label| label.name == "October")
            .unwrap();
        assert!(label.end_row > before);
        assert_eq!(label.end_row, sheet.weeks.len() - 1);
    }

    #[test]
    fn monday_of_midweek_date() {
        assert_eq!(monday_of(date(2026, 9, 15)), date(2026, 9, 14));
        assert_eq!(monday_of(date(2026, 9, 14)), date(2026, 9, 14));
        assert_eq!(monday_of(date(2026, 9, 20)), date(2026, 9, 14));
    }
}
