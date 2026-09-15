//! PDF rendering of a [`Sheet`] onto a single A4 page, landscape or portrait,
//! with the calendar grid on the left and ruled writing lines on the right.

use crate::sheet::Sheet;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

// ---------------------------------------------------------------------------
// Geometry (A4 portrait, points; origin bottom-left).
// ---------------------------------------------------------------------------

/// A4 width in points.
const PAGE_WIDTH: f32 = 595.0;
/// A4 height in points.
const PAGE_HEIGHT: f32 = 842.0;
/// Outer page margin.
const MARGIN: f32 = 40.0;

/// Calendar area width (left column).
const CALENDAR_WIDTH: f32 = 170.0;
/// Gap between calendar and writing area.
const GAP: f32 = 20.0;
/// Calendar cell height; the last row stretches to fill the remaining page
/// height so the calendar runs to the end of the page.
const CELL_HEIGHT: f32 = 34.0;

/// Number of week rows that fill one page (header row + this many week
/// rows between the page margins). Sheets built for rendering should use
/// this count.
pub const PAGE_FILL_WEEKS: usize = ((PAGE_HEIGHT - 2.0 * MARGIN) / CELL_HEIGHT - 1.0) as usize;
/// Calendar table line width.
const CALENDAR_LINE_WIDTH: f32 = 0.7;
/// Writing line width.
const WRITING_LINE_WIDTH: f32 = 0.5;

/// Inset from the cell's left edge where day numbers are drawn.
const DAY_NUMBER_INSET_X: f32 = 4.0;
/// Baseline lift of day numbers within a cell.
const DAY_NUMBER_BASELINE_LIFT: f32 = 6.0;
/// Day number font size.
const DAY_NUMBER_FONT_SIZE: f32 = 10.0;
/// Header font size.
const HEADER_FONT_SIZE: f32 = 10.0;
/// Month name font size.
const MONTH_NAME_FONT_SIZE: f32 = 9.0;

/// Render `sheet` to PDF bytes.
pub fn render_pdf(sheet: &Sheet) -> Vec<u8> {
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
    page.media_box(Rect::new(0.0, 0.0, PAGE_WIDTH, PAGE_HEIGHT));
    page.parent(page_tree_id);
    page.contents(content_id);
    page.resources().fonts().pair(font_name, font_id);
    page.finish();

    pdf.type1_font(font_id).base_font(Name(b"Helvetica"));

    let content = draw(sheet, font_name);
    pdf.stream(content_id, &content);

    pdf.finish()
}

fn draw(sheet: &Sheet, font_name: Name) -> Vec<u8> {
    let mut content = Content::new();

    let weeks = sheet.weeks.len();
    debug_assert_eq!(weeks, PAGE_FILL_WEEKS, "sheet must fill the page exactly");
    let top = PAGE_HEIGHT - MARGIN;
    // Stretched row height: header + week rows exactly fill the page.
    let row_height = (top - MARGIN) / (weeks as f32 + 1.0);
    // Grid top starts below the header row.
    let grid_top = top - row_height;
    // Calendar grid horizontal extents.
    let calendar_left = MARGIN;
    let calendar_right = calendar_left + CALENDAR_WIDTH;
    let column_width = CALENDAR_WIDTH / 7.0;

    // Writing area horizontal extents.
    let writing_left = calendar_right + GAP;
    let writing_right = PAGE_WIDTH - MARGIN;

    // ------------------------------------------------------------------
    // Header row: weekday abbreviations.
    // ------------------------------------------------------------------
    let weekday_abbreviations = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "So"];
    for (column, abbreviation) in weekday_abbreviations.iter().enumerate() {
        let x_position = calendar_left + column as f32 * column_width + column_width / 2.0 - 8.0;
        content.begin_text();
        content.set_font(font_name, HEADER_FONT_SIZE);
        content.next_line(x_position, top - row_height + DAY_NUMBER_BASELINE_LIFT);
        content.show(Str(abbreviation.as_bytes()));
        content.end_text();
    }

    // ------------------------------------------------------------------
    // Calendar grid: horizontal lines.
    // ------------------------------------------------------------------
    content.set_line_width(CALENDAR_LINE_WIDTH);
    for row in 0..=weeks + 1 {
        let y_position = top - row as f32 * row_height;
        content.move_to(calendar_left, y_position);
        content.line_to(calendar_right, y_position);
    }
    // Vertical lines.
    for column in 0..=7 {
        let x_position = calendar_left + column as f32 * column_width;
        content.move_to(x_position, MARGIN);
        content.line_to(x_position, top);
    }
    content.stroke();

    // ------------------------------------------------------------------
    // Day numbers and month names.
    // ------------------------------------------------------------------
    for (row, week) in sheet.weeks.iter().enumerate() {
        for (column, day) in week.days.iter().enumerate() {
            let Some(date) = day else { continue };
            let x_position = calendar_left + column as f32 * column_width + DAY_NUMBER_INSET_X;
            let y_position =
                grid_top - row as f32 * row_height - row_height + DAY_NUMBER_BASELINE_LIFT;
            content.begin_text();
            content.set_font(font_name, DAY_NUMBER_FONT_SIZE);
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
        let box_right = calendar_right + GAP;
        // Rotated month name, centered in the box. Rotated glyph "up" is
        // +x, so the horizontal center of the ink is offset from the
        // baseline by (ascent - descent) / 2; for Helvetica that is
        // (0.718 - 0.207) / 2 = 0.256 em below the baseline.
        let box_center_x = (box_left + box_right) / 2.0;
        let baseline_x = box_center_x - MONTH_NAME_FONT_SIZE * 0.256;
        let extent_center_y = (box_top + box_bottom) / 2.0;
        // Vertical text extent of the name (Helvetica ~0.5 em per char).
        let text_height = label.name.len() as f32 * MONTH_NAME_FONT_SIZE * 0.5;
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
        content.set_font(font_name, MONTH_NAME_FONT_SIZE);
        // Rotate 90° clockwise: text advances in -y, glyph "up" is +x.
        content.set_text_matrix([0.0, -1.0, 1.0, 0.0, baseline_x, baseline_y]);
        content.show(Str(label.name.as_bytes()));
        content.end_text();
    }
    content.stroke();

    // ------------------------------------------------------------------
    // Writing lines, full remaining page height, aligned with calendar rows.
    content.set_line_width(WRITING_LINE_WIDTH);
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
        let sheet = Sheet::new(date(2026, 9, 15), PAGE_FILL_WEEKS);
        let pdf = render_pdf(&sheet);
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1000);
    }

    #[test]
    fn month_names_present_when_box_fits() {
        // The default sheet's months span multiple weeks; their names fit.
        let sheet = Sheet::new(date(2026, 9, 15), PAGE_FILL_WEEKS);
        let pdf = render_pdf(&sheet);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(text.contains("October"));
        assert!(text.contains("January"));
    }

    #[test]
    fn month_without_space_is_dropped() {
        // Start so that the sheet's last month is a single week: its name
        // (February, ~36pt) does not fit the one-week box (~35pt), so the
        // month gets no box and no label.
        let sheet = Sheet::new(date(2026, 9, 15), PAGE_FILL_WEEKS);
        assert_eq!(
            sheet.labels.last().map(|label| label.name),
            Some("February"),
            "precondition: February is the last month on this sheet"
        );
        let pdf = render_pdf(&sheet);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(!text.contains("February"));
    }
}
