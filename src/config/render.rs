//! Rendering configuration for the record-sheet PDF.

use crate::renders::logo::LogoImage;

/// QR code ink edge length in points (drawn square, dark modules only).
pub(crate) const QR_SIZE: f32 = 20.0;

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

/// Geometry and font size of the optional title band above the calendar.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleConfig {
    /// The title text, drawn in the band. May be empty when only a logo is
    /// set. The band exists only when text is non-empty: it then shortens
    /// the grid by [`TitleConfig::height`]. A logo with empty text creates
    /// no band — it overlays the unchanged page in the top-right corner,
    /// its ink flush with the top margin and the writing lines.
    pub text: String,
    /// Optional logo image drawn in the top-right: on the title text's line
    /// with text, pinned to the page corner without.
    pub logo: Option<LogoImage>,
    /// Height of the title band; the grid shrinks by this amount when a
    /// title is set.
    pub height: f32,
    /// Title font size.
    pub font_size: f32,
}

impl Default for TitleConfig {
    /// Default title band: no text, 36 pt tall, 16 pt font.
    fn default() -> Self {
        Self {
            text: String::new(),
            logo: None,
            height: 36.0,
            font_size: 16.0,
        }
    }
}

/// Rendering configuration. All lengths are in points.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderConfig {
    /// Page geometry: media box and outer margin.
    pub page: PageConfig,
    /// Week-row calendar cell geometry: grid cell size and day-number font.
    pub week_cells: WeekCellConfig,
    /// Month-name box geometry: box width (also the calendar-to-writing
    /// gap) and label font size.
    pub month_cells: MonthCellConfig,
    /// Optional title band above the calendar grid.
    pub title: Option<TitleConfig>,
    /// Text encoded as a small QR code in the bottom-right corner of the
    /// page. The bottom writing line is shortened so it never touches the
    /// QR; the calendar grid keeps its full height. `None` draws nothing.
    pub qr_code: Option<String>,
}

impl RenderConfig {
    /// Calendar area width: seven week cells across.
    pub(crate) fn calendar_width(&self) -> f32 {
        self.week_cells.width * 7.0
    }

    /// Number of week rows that fill one page below the optional title band
    /// (header row + this many week rows between the margins). A logo
    /// without title text does not create a band: it overlays the grid, so
    /// the row count matches a title-less page. A QR code does not change
    /// the row count either: it sits in the bottom-right corner and the
    /// last writing line is shortened to clear it.
    pub fn page_fill_weeks(&self) -> usize {
        let title_height = self
            .title
            .as_ref()
            .filter(|t| !t.text.is_empty())
            .map_or(0.0, |t| t.height);
        ((self.page.height - 2.0 * self.page.margin - title_height) / self.week_cells.height - 1.0)
            as usize
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
            page: PageConfig {
                width: 595.0,
                height: 842.0,
                margin: 40.0,
            },
            week_cells: WeekCellConfig {
                width: 24.0,
                height: 24.0,
                font_size: 10.0,
            },
            month_cells: MonthCellConfig {
                width: 20.0,
                font_size: 9.0,
            },
            title: None,
            qr_code: None,
        }
    }
}
