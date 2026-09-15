//! PDF rendering of a [`Sheet`] onto a single page (A4 with the default
//! [`RenderConfig`]), landscape or portrait, with the calendar grid on the
//! left and ruled writing lines on the right.

use crate::sheet::Sheet;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

// ---------------------------------------------------------------------------
// Configuration.
// ---------------------------------------------------------------------------

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
    fn calendar_width(&self) -> f32 {
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
    fn calendar_line_width(&self) -> f32 {
        0.7 * self.week_cells.height / 34.0
    }

    /// Writing line width: scales with the cell size, 0.5 at the A4
    /// default (34 pt cells).
    fn writing_line_width(&self) -> f32 {
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

/// Render `sheet` to PDF bytes using `config`.
pub fn render_pdf(sheet: &Sheet, config: &RenderConfig) -> Vec<u8> {
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let font_id = Ref::new(4);
    let content_id = Ref::new(5);
    let font_name = Name(b"F1");

    let mut pdf = Pdf::new();
    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id).kids([page_id]).count(1);

    let mut page = pdf.page(page_id);
    page.media_box(Rect::new(0.0, 0.0, config.page.width, config.page.height));
    page.parent(page_tree_id);
    page.contents(content_id);
    page.resources().fonts().pair(font_name, font_id);
    page.finish();

    pdf.type1_font(font_id).base_font(Name(b"Helvetica"));

    let content = draw(sheet, config, font_name);
    pdf.stream(content_id, &content);

    pdf.finish()
}

fn draw(sheet: &Sheet, config: &RenderConfig, font_name: Name) -> Vec<u8> {
    let mut content = Content::new();

    let weeks = sheet.weeks.len();
    debug_assert_eq!(weeks, config.page_fill_weeks(), "sheet must fill the page exactly");
    let top = config.page.height - config.page.margin;
    // Stretched row height: header + week rows exactly fill the page.
    let row_height = (top - config.page.margin) / (weeks as f32 + 1.0);
    // Grid top starts below the header row.
    let grid_top = top - row_height;
    // Calendar grid horizontal extents.
    let calendar_left = config.page.margin;
    let calendar_right = calendar_left + config.calendar_width();
    let column_width = config.week_cells.width;

    // Writing area horizontal extents.
    let writing_left = calendar_right + config.month_cells.width;
    let writing_right = config.page.width - config.page.margin;

    // ------------------------------------------------------------------
    // Header row: weekday abbreviations.
    // ------------------------------------------------------------------
    let weekday_abbreviations = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "So"];
    for (column, abbreviation) in weekday_abbreviations.iter().enumerate() {
        let x_position = calendar_left + column as f32 * column_width + column_width / 2.0
            - config.week_cells.font_size * 0.8;
        content.begin_text();
        content.set_font(font_name, config.week_cells.font_size);
        content.next_line(x_position, top - row_height + config.week_cells.font_size * 0.6);
        content.show(Str(abbreviation.as_bytes()));
        content.end_text();
    }

    // ------------------------------------------------------------------
    // Calendar grid: horizontal lines.
    // ------------------------------------------------------------------
    content.set_line_width(config.calendar_line_width());
    for row in 0..=weeks + 1 {
        let y_position = top - row as f32 * row_height;
        content.move_to(calendar_left, y_position);
        content.line_to(calendar_right, y_position);
    }
    // Vertical lines.
    for column in 0..=7 {
        let x_position = calendar_left + column as f32 * column_width;
        content.move_to(x_position, config.page.margin);
        content.line_to(x_position, top);
    }
    content.stroke();

    // ------------------------------------------------------------------
    // Day numbers and month names.
    // ------------------------------------------------------------------
    for (row, week) in sheet.weeks.iter().enumerate() {
        for (column, day) in week.days.iter().enumerate() {
            let Some(date) = day else { continue };
            let x_position = calendar_left + column as f32 * column_width
                + config.week_cells.font_size * 0.4;
            let y_position =
                grid_top - row as f32 * row_height - row_height + config.week_cells.font_size * 0.6;
            content.begin_text();
            content.set_font(font_name, config.week_cells.font_size);
            content.next_line(x_position, y_position);
            content.show(Str(date.day().to_string().as_bytes()));
            content.end_text();
        }
    }
    for label in &sheet.labels {
        // The month box spans vertically from the row holding the 1st to
        // the row holding the month's last visible day. If the 1st is a
        // Monday (or the sheet starts mid-week with no earlier day in the
        // same row), the box top aligns with the week line; otherwise it
        // starts mid-cell. Symmetrically for the bottom edge.
        let week = &sheet.weeks[label.row];
        let start_on_line =
            label.column == 0 || week.days[..label.column].iter().all(|day| day.is_none());
        let end_week = &sheet.weeks[label.end_row];
        let end_on_line = label.end_column == 6
            || end_week.days[label.end_column + 1..]
                .iter()
                .all(|day| day.is_none());

        let row_top = grid_top - label.row as f32 * row_height;
        let box_top = if start_on_line {
            row_top
        } else {
            row_top - row_height / 2.0
        };
        let end_row_top = grid_top - label.end_row as f32 * row_height;
        let end_row_bottom = end_row_top - row_height;
        let box_bottom = if end_on_line {
            end_row_bottom
        } else {
            end_row_top - row_height / 2.0
        };

        // Box: from just right of the grid into the gap.
        let box_left = calendar_right;
        let box_right = calendar_right + config.month_cells.width;
        // Rotated month name, centered in the box. Rotated glyph "up" is
        // +x, so the horizontal center of the ink is offset from the
        // baseline by (ascent - descent) / 2; for Helvetica that is
        // (0.718 - 0.207) / 2 = 0.256 em below the baseline.
        let box_center_x = (box_left + box_right) / 2.0;
        let baseline_x = box_center_x - config.month_cells.font_size * 0.256;
        let extent_center_y = (box_top + box_bottom) / 2.0;
        // Vertical text extent of the name (Helvetica ~0.5 em per char).
        let text_height = label.name.len() as f32 * config.month_cells.font_size * 0.5;
        let box_height = box_top - box_bottom;
        // Symmetric to the first month: if the box has no space for the
        // name, the last month gets no box at all.
        if text_height > box_height {
            continue;
        }

        content.rect(
            box_left,
            box_bottom,
            box_right - box_left,
            box_top - box_bottom,
        );
        let baseline_y = extent_center_y + text_height / 2.0;
        content.begin_text();
        content.set_font(font_name, config.month_cells.font_size);
        // Rotate 90° clockwise: text advances in -y, glyph "up" is +x.
        content.set_text_matrix([0.0, -1.0, 1.0, 0.0, baseline_x, baseline_y]);
        content.show(Str(label.name.as_bytes()));
        content.end_text();
    }
    content.stroke();

    // ------------------------------------------------------------------
    // Writing lines, full remaining page height, aligned with calendar rows.
    content.set_line_width(config.writing_line_width());
    for row in 1..=weeks + 1 {
        let y_position = top - row as f32 * row_height;
        content.move_to(writing_left, y_position);
        content.line_to(writing_right, y_position);
    }
    content.stroke();

    content.finish().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn renders_nonempty_pdf() {
        let config = RenderConfig::default();
        let sheet = Sheet::new(date(2026, 9, 15), config.page_fill_weeks());
        let pdf = render_pdf(&sheet, &config);
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1000);
    }

    #[test]
    fn month_names_present_when_box_fits() {
        let config = RenderConfig::default();
        let sheet = Sheet::new(date(2026, 9, 15), config.page_fill_weeks());
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(text.contains("October"));
        assert!(text.contains("January"));
    }

    #[test]
    fn month_without_space_is_dropped() {
        // Start so that the sheet's last month is a single week row
        // (2027-02-01 is a Monday): its name (February, ~36pt) does not
        // fit the one-week box (~24.6pt), so the month gets no box and
        // no label.
        let config = RenderConfig::default();
        let sheet = Sheet::new(date(2026, 7, 19), config.page_fill_weeks());
        assert_eq!(
            sheet.labels.last().map(|label| label.name),
            Some("February"),
            "precondition: February is the last month on this sheet"
        );
        assert_eq!(
            sheet.labels.last().map(|label| label.end_row - label.row),
            Some(0),
            "precondition: February spans a single week row"
        );
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(!text.contains("February"));
    }

    #[test]
    fn renders_with_custom_page_config() {
        // US Letter instead of A4: the media box must reflect the config.
        let config = RenderConfig {
            page: PageConfig { width: 612.0, height: 792.0, margin: 36.0 },
            ..RenderConfig::default()
        };
        let sheet = Sheet::new(date(2026, 9, 15), config.page_fill_weeks());
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(text.contains("612 792"));
    }
}
