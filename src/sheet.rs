//! Calendar-grid model for the record sheet: a Monday-first week grid
//! starting at a given date, running for a fixed number of weeks ("to the
//! end of the page"), with month-name label placement.
use jiff::{
    civil::{Date, Weekday},
    ToSpan,
};

/// Number of days in a week row.
pub const WEEK_LEN: usize = 7;

/// One row of the calendar grid: a week, laid out Monday-first.
///
/// `None` marks cells before the sheet's start date (the grid "starts at the
/// date given to the CLI"). Days after the start always continue across
/// month boundaries until the week count is exhausted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Week {
    /// Exactly 7 entries, Monday (index 0) ..= Sunday (index 6).
    pub days: [Option<Date>; WEEK_LEN],
}

impl Week {
    fn of(date: Date) -> Week {
        let offset = date.weekday().to_monday_zero_offset() as i16;
        let monday = date - offset.days();
        let mut days = [None; WEEK_LEN];
        for (i, slot) in days.iter_mut().enumerate() {
            *slot = Some(monday + (i as i16).days());
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
    pub col: usize,
    /// Full month name, e.g. "September".
    pub name: &'static str,
    /// Index into `Sheet::weeks` of the week holding the month's last
    /// visible day.
    pub end_row: usize,
    /// Column of the month's last visible day within its week.
    pub end_col: usize,
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
        let mut weeks: Vec<Week> = Vec::with_capacity(week_count);
        let mut cursor = start;
        for _ in 0..week_count {
            let mut week = Week::of(cursor);
            // Blank out days before the start date in the first week.
            for slot in week.days.iter_mut() {
                if let Some(d) = *slot {
                    if d < start {
                        *slot = None;
                    }
                }
            }
            weeks.push(week);
            cursor += 7.days();
        }

        // One label per month whose 1st appears in the grid; each carries
        let mut labels = Vec::new();
        for (row, week) in weeks.iter().enumerate() {
            for (col, day) in week.days.iter().enumerate() {
                let Some(date) = day else { continue };
                if date.day() != 1 {
                    continue;
                }
                // Find the last visible day of the same month.
                let mut end_row = row;
                let mut end_col = col;
                for (r2, w2) in weeks.iter().enumerate().skip(row) {
                    for (c2, d2) in w2.days.iter().enumerate() {
                        if let Some(d) = d2 {
                            if (d.year(), d.month()) == (date.year(), date.month()) {
                                end_row = r2;
                                end_col = c2;
                            }
                        }
                    }
                }
                labels.push(MonthLabel {
                    row,
                    col,
                    name: month_name(date.month()),
                    end_row,
                    end_col,
                });
            }
        }

        Sheet {
            start,
            weeks,
            labels,
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
pub fn weekday_col(date: Date) -> usize {
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
                col: 3,
                name: "October",
                end_row: 5,
                end_col: 6,
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
                    col: 6,
                    name: "November",
                    end_row: 5,
                    end_col: 0,
                },
                MonthLabel {
                    row: 5,
                    col: 1,
                    name: "December",
                    end_row: 5,
                    end_col: 6,
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
                col: 1,
                name: "September",
                end_row: 0,
                end_col: 6,
            }]
        );
    }

    #[test]
    fn monday_of_midweek_date() {
        assert_eq!(monday_of(date(2026, 9, 15)), date(2026, 9, 14));
        assert_eq!(monday_of(date(2026, 9, 14)), date(2026, 9, 14));
        assert_eq!(monday_of(date(2026, 9, 20)), date(2026, 9, 14));
    }
}
