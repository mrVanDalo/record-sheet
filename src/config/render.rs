//! Rendering configuration for the record-sheet PDF.

/// Page geometry in points (origin bottom-left).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageConfig {
    /// Page width.
    pub width: f32,
    /// Page height.
    pub height: f32,
    /// Outer page margin.
    pub margin: f32,
}

/// Geometry of the week-row calendar cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeekCellConfig {
    /// Cell width; the calendar grid is seven cells across.
    pub width: f32,
    /// Nominal cell height; the last row stretches so the grid runs from
    /// margin to margin.
    pub height: f32,
    /// Font size of day numbers and the weekday header.
    pub font_size: f32,
}

/// Geometry of the rotated month-name boxes between the calendar and the
/// writing lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonthCellConfig {
    /// Box width.
    pub width: f32,
    /// Month name font size.
    pub font_size: f32,
}

/// Rendering configuration. All lengths are in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderConfig {
    /// Page geometry: media box and outer margin.
    pub page: PageConfig,
    /// Week-row calendar cell geometry: grid cell size and day-number font.
    pub week_cells: WeekCellConfig,
    /// Month-name box geometry: box width (also the calendar-to-writing
    /// gap) and label font size.
    pub month_cells: MonthCellConfig,
}

impl RenderConfig {
    /// Calendar area width: seven week cells across.
    pub(crate) fn calendar_width(&self) -> f32 {
        self.week_cells.width * 7.0
    }

    /// Number of week rows that fill one page (header row + this many week
    /// rows between the page margins). Sheets built for rendering should
    /// use this count.
    pub fn page_fill_weeks(&self) -> usize {
        ((self.page.height - 2.0 * self.page.margin) / self.week_cells.height - 1.0) as usize
    }

    /// Calendar table line width: scales with the cell size, 0.7 at the
    /// A4 default (34 pt cells).
    pub(crate) fn calendar_line_width(&self) -> f32 {
        0.7 * self.week_cells.height / 34.0
    }

    /// Writing line width: scales with the cell size, 0.5 at the A4
    /// default (34 pt cells).
    pub(crate) fn writing_line_width(&self) -> f32 {
        0.5 * self.week_cells.height / 34.0
    }
}

impl Default for RenderConfig {
    /// A4 portrait.
    fn default() -> Self {
        Self {
            page: PageConfig { width: 595.0, height: 842.0, margin: 40.0 },
            week_cells: WeekCellConfig { width: 24.0, height: 24.0, font_size: 10.0 },
            month_cells: MonthCellConfig { width: 20.0, font_size: 9.0 },
        }
    }
}
