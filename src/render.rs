//! PDF rendering of a [`Sheet`] onto a single A4 page, landscape or portrait,
//! with the calendar grid on the left and ruled writing lines on the right.

use crate::sheet::Sheet;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

// ---------------------------------------------------------------------------
// Geometry (A4 portrait, points; origin bottom-left).
// ---------------------------------------------------------------------------

/// A4 width in points.
const PAGE_W: f32 = 595.0;
/// A4 height in points.
const PAGE_H: f32 = 842.0;
/// Outer page margin.
const MARGIN: f32 = 40.0;

/// Calendar area width (left column).
const CAL_W: f32 = 170.0;
/// Gap between calendar and writing area.
const GAP: f32 = 20.0;
/// Calendar cell height; the last row stretches to fill the remaining page
/// height so the calendar runs to the end of the page.
const CELL_H: f32 = 34.0;

/// Number of week rows that fill one page (header row + this many week
/// rows between the page margins). Sheets built for rendering should use
/// this count.
pub const PAGE_FILL_WEEKS: usize = ((PAGE_H - 2.0 * MARGIN) / CELL_H - 1.0) as usize;
/// Calendar table line width.
const CAL_LINE: f32 = 0.7;
/// Writing line width.
const WRITE_LINE: f32 = 0.5;

/// Inset from the cell's left edge where day numbers are drawn.
const DAY_NUM_X: f32 = 4.0;
/// Baseline lift of day numbers within a cell.
const DAY_NUM_Y: f32 = 6.0;
/// Day number font size.
const DAY_SIZE: f32 = 10.0;
/// Header font size.
const HDR_SIZE: f32 = 10.0;
/// Month name font size.
const MONTH_SIZE: f32 = 9.0;

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
    page.media_box(Rect::new(0.0, 0.0, PAGE_W, PAGE_H));
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
    let mut c = Content::new();

    let weeks = sheet.weeks.len();
    debug_assert_eq!(weeks, PAGE_FILL_WEEKS, "sheet must fill the page exactly");
    let top = PAGE_H - MARGIN;
    // Stretched row height: header + week rows exactly fill the page.
    let row_h = (top - MARGIN) / (weeks as f32 + 1.0);
    // Grid top starts below the header row.
    let grid_top = top - row_h;
    // Calendar grid x extents.
    let cal_x0 = MARGIN;
    let cal_x1 = cal_x0 + CAL_W;
    let col_w = CAL_W / 7.0;

    // Writing area x extents.
    let wr_x0 = cal_x1 + GAP;
    let wr_x1 = PAGE_W - MARGIN;

    // ------------------------------------------------------------------
    // Header row: weekday abbreviations.
    // ------------------------------------------------------------------
    let hdr_names = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "So"];
    for (i, name) in hdr_names.iter().enumerate() {
        let x = cal_x0 + i as f32 * col_w + col_w / 2.0 - 8.0;
        c.begin_text();
        c.set_font(font_name, HDR_SIZE);
        c.next_line(x, top - row_h + DAY_NUM_Y);
        c.show(Str(name.as_bytes()));
        c.end_text();
    }

    // ------------------------------------------------------------------
    // Calendar grid: horizontal lines.
    // ------------------------------------------------------------------
    c.set_line_width(CAL_LINE);
    for row in 0..=weeks + 1 {
        let y = top - row as f32 * row_h;
        c.move_to(cal_x0, y);
        c.line_to(cal_x1, y);
    }
    // Vertical lines.
    for col in 0..=7 {
        let x = cal_x0 + col as f32 * col_w;
        c.move_to(x, MARGIN);
        c.line_to(x, top);
    }
    c.stroke();

    // ------------------------------------------------------------------
    // Day numbers and month names.
    // ------------------------------------------------------------------
    for (row, week) in sheet.weeks.iter().enumerate() {
        for (col, day) in week.days.iter().enumerate() {
            let Some(date) = day else { continue };
            let x = cal_x0 + col as f32 * col_w + DAY_NUM_X;
            let y = grid_top - row as f32 * row_h - row_h + DAY_NUM_Y;
            c.begin_text();
            c.set_font(font_name, DAY_SIZE);
            c.next_line(x, y);
            c.show(Str(date.day().to_string().as_bytes()));
            c.end_text();
        }
    }
    for label in &sheet.labels {
        // The month box spans vertically from the row holding the 1st to
        // the row holding the month's last visible day. If the 1st is a
        // Monday (or the sheet starts mid-week with no earlier day in the
        // same row), the box top aligns with the week line; otherwise it
        // starts mid-cell. Symmetrically for the bottom edge.
        let week = &sheet.weeks[label.row];
        let start_on_line = label.col == 0 || week.days[..label.col].iter().all(|d| d.is_none());
        let end_week = &sheet.weeks[label.end_row];
        let end_on_line = label.end_col == 6
            || end_week.days[label.end_col + 1..]
                .iter()
                .all(|d| d.is_none());

        let row_top = grid_top - label.row as f32 * row_h;
        let top_y = if start_on_line {
            row_top
        } else {
            row_top - row_h / 2.0
        };
        let end_row_top = grid_top - label.end_row as f32 * row_h;
        let end_row_bottom = end_row_top - row_h;
        let bottom_y = if end_on_line {
            end_row_bottom
        } else {
            end_row_top - row_h / 2.0
        };

        // Box: from just right of the grid into the gap.
        let box_x0 = cal_x1;
        let box_x1 = cal_x1 + GAP;
        // Rotated month name, centered in the box. Rotated glyph "up" is
        // +x, so the horizontal center of the ink is offset from the
        // baseline by (ascent - descent) / 2; for Helvetica that is
        // (0.718 - 0.207) / 2 = 0.256 em below the baseline.
        let box_mid_x = (box_x0 + box_x1) / 2.0;
        let baseline_x = box_mid_x - MONTH_SIZE * 0.256;
        let extent_mid_y = (top_y + bottom_y) / 2.0;
        // Vertical text extent of the name (Helvetica ~0.5 em per char).
        let text_h = label.name.len() as f32 * MONTH_SIZE * 0.5;
        let box_h = top_y - bottom_y;
        // Symmetric to the first month: if the box has no space for the
        // name, the last month gets no box at all.
        if text_h > box_h {
            continue;
        }

        c.rect(box_x0, bottom_y, box_x1 - box_x0, top_y - bottom_y);
        let baseline_y = extent_mid_y + text_h / 2.0;
        c.begin_text();
        c.set_font(font_name, MONTH_SIZE);
        // Rotate 90° clockwise: text advances in -y, glyph "up" is +x.
        c.set_text_matrix([0.0, -1.0, 1.0, 0.0, baseline_x, baseline_y]);
        c.show(Str(label.name.as_bytes()));
        c.end_text();
    }
    c.stroke();

    // ------------------------------------------------------------------
    // Writing lines, full remaining page height, aligned with calendar rows.
    c.set_line_width(WRITE_LINE);
    for row in 1..=weeks + 1 {
        let y = top - row as f32 * row_h;
        c.move_to(wr_x0, y);
        c.line_to(wr_x1, y);
    }
    c.stroke();

    c.finish().to_vec()
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
            sheet.labels.last().map(|l| l.name),
            Some("February"),
            "precondition: February is the last month on this sheet"
        );
        let pdf = render_pdf(&sheet);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(!text.contains("February"));
    }
}
