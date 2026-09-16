//! PDF rendering of a [`Sheet`] onto a single page (A4 with the default
//! [`RenderConfig`]), landscape or portrait, with the calendar grid on the
//! left and ruled writing lines on the right.

use crate::config::render::RenderConfig;
use crate::models::sheet::{Row, RowGroupKind, Sheet};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

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

    pdf.type1_font(font_id)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));

    let content = draw(sheet, config, font_name);
    pdf.stream(content_id, &content);

    pdf.finish()
}

/// Encode `text` as single-byte WinAnsi (CP1252) codes for the standard
/// Helvetica font. Characters in the Latin-1 range (where CP1252 and
/// Latin-1 agree, and where every generated label lives) map to their
/// code point; anything else falls back to `?`.
fn winansi_bytes(text: &str) -> Vec<u8> {
    text.chars()
        .map(|character| {
            if (character as u32) < 0x100 {
                character as u8
            } else {
                b'?'
            }
        })
        .collect()
}

fn draw(sheet: &Sheet, config: &RenderConfig, font_name: Name) -> Vec<u8> {
    let mut content = Content::new();

    let row_count = sheet.rows.len();
    debug_assert_eq!(
        row_count - 1,
        config.page_fill_weeks(),
        "sheet must fill the page exactly"
    );
    let title = config.title.as_ref();
    let title_height = title.map_or(0.0, |t| t.height);
    let top = config.page.height - config.page.margin - title_height;
    // Stretched row height: header + week rows exactly fill the page.
    let row_height = (top - config.page.margin) / row_count as f32;
    // Row 0 is the header; `row_top(i)` is the top edge of row `i`.
    // Negative indexes are virtual rows before the grid (a month group
    // anchored before the first week).
    let row_top = |index: isize| top - index as f32 * row_height;
    // Calendar grid horizontal extents.
    let calendar_left = config.page.margin;
    let calendar_right = calendar_left + config.calendar_width();
    let column_width = config.week_cells.width;

    // Writing area horizontal extents.
    let writing_left = calendar_right + config.month_cells.width;
    let writing_right = config.page.width - config.page.margin;

    // ------------------------------------------------------------------
    // Optional title band.
    // ------------------------------------------------------------------
    if let Some(t) = title {
        let band_top = config.page.height - config.page.margin;
        // Cap top of the title sits exactly on the grid's top edge: the
        // same distance from the paper top as the grid gets without a
        // title (one page margin). Baseline = cap top minus the 0.718 em
        // Helvetica cap/ascent height.
        let content_width = config.page.width - 2.0 * config.page.margin;
        let est_text_width = t.text.len() as f32 * t.font_size * 0.5;
        let x = config.page.margin + (content_width - est_text_width) / 2.0;
        let baseline_y = band_top - t.font_size * 0.718;
        content.begin_text();
        content.set_font(font_name, t.font_size);
        content.next_line(x, baseline_y);
        content.show(Str(&winansi_bytes(&t.text)));
        content.end_text();
    }

    // ------------------------------------------------------------------
    // Calendar grid: horizontal lines.
    // ------------------------------------------------------------------
    content.set_line_width(config.calendar_line_width());
    for row in 0..=row_count {
        let y_position = row_top(row as isize);
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
    for (row_index, row) in sheet.rows.iter().enumerate() {
        let row_bottom = row_top(row_index as isize) - row_height;
        match row {
            Row::WeekHeader(header) => {
                for (column, label) in header.labels.iter().enumerate() {
                    let x_position =
                        calendar_left + column as f32 * column_width + column_width / 2.0
                            - config.week_cells.font_size * 0.8;
                    content.begin_text();
                    content.set_font(font_name, config.week_cells.font_size);
                    content.next_line(x_position, row_bottom + config.week_cells.font_size * 0.6);
                    content.show(Str(&winansi_bytes(label)));
                    content.end_text();
                }
            }
            Row::Week(week) => {
                for (column, day) in week.days.iter().enumerate() {
                    let Some(date) = day else { continue };
                    let x_position = calendar_left
                        + column as f32 * column_width
                        + config.week_cells.font_size * 0.4;
                    let y_position = row_bottom + config.week_cells.font_size * 0.6;
                    content.begin_text();
                    content.next_line(x_position, y_position);
                    content.show(Str(&winansi_bytes(&date.day().to_string())));
                    content.end_text();
                }
            }
        }
    }
    for label in &sheet.groups {
        // The month box spans vertically from the row holding the 1st to
        // the row holding the month's last visible day. If the 1st is a
        // Monday (or the sheet starts mid-week with no earlier day in the
        // same row), the box top aligns with the week line; otherwise it
        // starts mid-cell. Symmetrically for the bottom edge. A negative
        // start row is virtual (before the grid): nothing precedes it, so
        // the box starts on a line; row 0 is the header band, treated the
        // same.
        let start_on_line = match usize::try_from(label.start.row) {
            Ok(row) => match &sheet.rows[row] {
                Row::Week(week) => {
                    label.start.column == 0
                        || week.days[..label.start.column]
                            .iter()
                            .all(|day| day.is_none())
                }
                Row::WeekHeader(_) => true,
            },
            Err(_) => true,
        };
        let Row::Week(end_week) = &sheet.rows[label.end.row as usize] else {
            continue;
        };
        let end_on_line = label.end.column == 6
            || end_week.days[label.end.column + 1..]
                .iter()
                .all(|day| day.is_none());

        let box_row_top = row_top(label.start.row);
        let box_top = if start_on_line {
            box_row_top
        } else {
            box_row_top - row_height / 2.0
        };
        // Virtual groups may start above the grid; clamp to the grid's
        // top edge so box and rotated label stay on the page.
        let box_top = box_top.min(top);
        let end_row_top = row_top(label.end.row);
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
        // The label box height in row units (continuous, half rows
        // possible) decides which name variant applies: the box spans
        // from the 1st's position to the month's last visible day, so a
        // month clipped at the grid edge or starting mid-cell gets a box
        // smaller than its day-cell count would suggest.
        let box_rows = (box_top - box_bottom) / row_height;
        // The biggest applicable variant wins; none apply -> no text,
        // but the box is still drawn (matches the empty-cell months).
        let RowGroupKind::Month(month) = label.kind;
        let name = sheet.year.months[month as usize].name(box_rows);
        let text_height = name.map_or(0.0, |name| {
            name.len() as f32 * config.month_cells.font_size * 0.5
        });
        let box_height = box_top - box_bottom;
        // Symmetric to the first month: if the box has no space for any
        // variant of the name, the last month gets no box at all.
        if name
            .is_some_and(|name| name.len() as f32 * config.month_cells.font_size * 0.5 > box_height)
        {
            continue;
        }

        content.rect(
            box_left,
            box_bottom,
            box_right - box_left,
            box_top - box_bottom,
        );
        let baseline_y = extent_center_y + text_height / 2.0;
        if let Some(name) = name {
            content.begin_text();
            content.set_font(font_name, config.month_cells.font_size);
            // Rotate 90° clockwise: text advances in -y, glyph "up" is +x.
            content.set_text_matrix([0.0, -1.0, 1.0, 0.0, baseline_x, baseline_y]);
            content.show(Str(&winansi_bytes(name)));
            content.end_text();
        }
    }
    content.stroke();

    // ------------------------------------------------------------------
    // Writing lines, full remaining page height, aligned with calendar rows.
    content.set_line_width(config.writing_line_width());
    for row in 1..=row_count {
        let y_position = row_top(row as isize);
        content.move_to(writing_left, y_position);
        content.line_to(writing_right, y_position);
    }
    content.stroke();

    content.finish().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::render::{PageConfig, TitleConfig};
    use crate::models::sheet::Language;
    use jiff::civil::date;

    #[test]
    fn renders_nonempty_pdf() {
        let config = RenderConfig::default();
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
        let pdf = render_pdf(&sheet, &config);
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1000);
    }

    #[test]
    fn month_names_present_when_box_fits() {
        let config = RenderConfig::default();
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
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
        let sheet = Sheet::new(
            date(2026, 7, 19),
            config.page_fill_weeks(),
            Language::English,
        );
        assert_eq!(
            sheet.groups.last().map(|group| group.kind),
            Some(RowGroupKind::Month(1)),
            "precondition: February is the last month on this sheet"
        );
        assert_eq!(
            sheet
                .groups
                .last()
                .map(|group| group.end.row - group.start.row),
            Some(0),
            "precondition: February spans a single week row"
        );
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(!text.contains("February"));
    }
    #[test]
    fn month_short_name_when_box_is_small() {
        // The user's report: a page-filling sheet from Tuesday
        // 2026-09-15 ends Sunday 2027-04-11, clipping April to a label
        // box of 1.5 row units (1st mid-cell Thursday, last visible day
        // Sunday Apr 11 on the grid edge). The 2-cell variant applies
        // (1.5 > 1), the full name does not (1.5 > 2 is false).
        let config = RenderConfig::default();
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
        let group = sheet
            .groups
            .last()
            .expect("precondition: the sheet ends in April");
        let RowGroupKind::Month(3) = group.kind else {
            panic!("precondition: the sheet ends in April");
        };
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(text.contains("Apr"));
        assert!(!text.contains("April"));
    }

    #[test]
    fn first_month_box_clamped_to_grid_top() {
        // The virtual first month (September, 1st before the grid) gets
        // a box clamped to the grid's top edge; its label must render.
        let config = RenderConfig::default();
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(text.contains("September"));
    }

    #[test]
    fn renders_with_custom_page_config() {
        // US Letter instead of A4: the media box must reflect the config.
        let config = RenderConfig {
            page: PageConfig {
                width: 612.0,
                height: 792.0,
                margin: 36.0,
            },
            ..RenderConfig::default()
        };
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(text.contains("612 792"));
    }

    #[test]
    fn german_month_names_use_winansi_encoding() {
        // "März" must appear with the WinAnsi (0xE4) encoding of "ä",
        // not raw UTF-8 bytes, and the font must declare WinAnsiEncoding.
        let config = RenderConfig::default();
        let sheet = Sheet::new(
            date(2026, 2, 15),
            config.page_fill_weeks(),
            Language::German,
        );
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(text.contains("WinAnsiEncoding"));
        // "März" renders with the WinAnsi code 0xE4 for "ä" (hex string
        // `4DE4727A` in the content stream), never raw UTF-8 `C3A4`.
        assert!(text.contains("<4DE4727A>"));
        assert!(!pdf.windows(4).any(|window| window == b"M\xc3\xa4"));
    }

    #[test]
    fn title_drawn_when_set() {
        let config = RenderConfig {
            title: Some(TitleConfig {
                text: "My Calendar".to_string(),
                height: 36.0,
                font_size: 16.0,
            }),
            ..RenderConfig::default()
        };
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        // The content stream is stored uncompressed; "My Calendar" appears
        // verbatim, like the month-name tests.
        assert!(text.contains("My Calendar"));
    }

    #[test]
    fn title_absent_when_none() {
        let config = RenderConfig::default();
        assert!(config.title.is_none());
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
        let pdf = render_pdf(&sheet, &config);
        let text = String::from_utf8_lossy(&pdf).into_owned();
        // No title text should appear.
        assert!(!text.contains("My Calendar"));
    }
}
