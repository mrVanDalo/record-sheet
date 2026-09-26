//! PDF rendering of a [`Sheet`] onto a single page (A4 with the default
//! [`RenderConfig`]), landscape or portrait, with the calendar grid on the
//! left and ruled writing lines on the right.

use crate::config::render::{QR_SIZE, RenderConfig};
use crate::models::sheet::{Row, RowGroupKind, Sheet};
use crate::RecordError;
use pdf_writer::{Content, Finish, Filter, Name, Pdf, Rect, Ref, Str};

/// Render `sheet` to PDF bytes using `config`.
///
/// Fails with [`RecordError::InvalidQrCode`] when `config.qr_code` cannot
/// be encoded as a QR code (e.g. the text is too long).
pub fn render_pdf(sheet: &Sheet, config: &RenderConfig) -> Result<Vec<u8>, RecordError> {
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let font_id = Ref::new(4);
    let content_id = Ref::new(5);
    let logo_id = Ref::new(6);
    let logo_mask_id = Ref::new(7);
    let font_name = Name(b"F1");
    let logo_name = Name(b"Im1");

    let mut pdf = Pdf::new();
    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id).kids([page_id]).count(1);

    let mut page = pdf.page(page_id);
    page.media_box(Rect::new(0.0, 0.0, config.page.width, config.page.height));
    page.parent(page_tree_id);
    page.contents(content_id);
    // One `/Resources` dictionary per page: calling `page.resources()` twice
    // would write two `/Resources` entries.
    let mut resources = page.resources();
    resources.fonts().pair(font_name, font_id);
    if config
        .title
        .as_ref()
        .is_some_and(|title| title.logo.is_some())
    {
        resources.x_objects().pair(logo_name, logo_id);
    }
    drop(resources);
    page.finish();

    pdf.type1_font(font_id)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));

    if let Some(logo) = config.title.as_ref().and_then(|title| title.logo.as_ref()) {
        // The soft mask must be written before the image that references it:
        // each XObject writer holds a mutable borrow of `pdf`.
        if let Some(alpha) = &logo.alpha {
            let samples = miniz_oxide::deflate::compress_to_vec_zlib(alpha, 6);
            let mut mask = pdf.image_xobject(logo_mask_id, &samples);
            mask.filter(Filter::FlateDecode);
            mask.width(logo.width as i32);
            mask.height(logo.height as i32);
            mask.bits_per_component(8);
            mask.color_space().device_gray();
        }
        let samples = miniz_oxide::deflate::compress_to_vec_zlib(&logo.rgb, 6);
        let mut image = pdf.image_xobject(logo_id, &samples);
        image.filter(Filter::FlateDecode);
        image.width(logo.width as i32);
        image.height(logo.height as i32);
        image.bits_per_component(8);
        image.color_space().device_rgb();
        if logo.alpha.is_some() {
            image.s_mask(logo_mask_id);
        }
    }

    let qr = config
        .qr_code
        .as_deref()
        .map(crate::renders::qr::encode_qr)
        .transpose()?;
    let content = draw(sheet, config, qr.as_ref(), font_name, logo_name);
    pdf.stream(content_id, &content);
    Ok(pdf.finish())
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

fn draw(
    sheet: &Sheet,
    config: &RenderConfig,
    qr: Option<&crate::renders::qr::QrMatrix>,
    font_name: Name,
    logo_name: Name,
) -> Vec<u8> {
    let mut content = Content::new();

    let row_count = sheet.rows.len();
    debug_assert_eq!(
        row_count - 1,
        config.page_fill_weeks(),
        "sheet must fill the page exactly"
    );
    let title = config.title.as_ref();
    // The band exists only when title text is present; a logo without text
    // overlays the unchanged page in the top-right corner.
    let title_height = title.filter(|t| !t.text.is_empty()).map_or(0.0, |t| t.height);
    // Ink rect of the overlay logo (logo without title text): writing lines
    // intersecting this rect must stop short of it. `None` in every other
    // mode — with title text the band keeps the grid clear of the logo.
    let overlay = title
        .filter(|t| t.text.is_empty())
        .and_then(|t| t.logo.as_ref())
        .map(|logo| {
            const LOGO_SIZE: f32 = 42.0;
            let ink_w_px = (logo.trim_right - logo.trim_left) as f32;
            let ink_h_px = (logo.trim_bottom - logo.trim_top) as f32;
            let scale = LOGO_SIZE / ink_w_px.max(ink_h_px);
            let ink_w = ink_w_px * scale;
            let ink_h = ink_h_px * scale;
            let band_top = config.page.height - config.page.margin;
            // Same corner geometry as the draw below: ink top flush with
            // the paper margin, right edge flush with the writing lines.
            let ink_right = config.page.width - config.page.margin;
            let ink_bottom = band_top - ink_h;
            (ink_right - ink_w, ink_bottom, ink_right, band_top)
        });
    let top = config.page.height - config.page.margin - title_height;
    // Stretched row height: header + week rows exactly fill margin..top.
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
        if !t.text.is_empty() {
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
        // Logo: trimmed of its transparent border, scaled uniformly so the
        // longest visible edge is 42 pt, vertically centered on the title
        // text's cap-height center (or pinned to the band's top-right
        // corner without text), with the visible ink's right edge flush
        // against the writing lines. Drawn after the title text, isolated
        // in its own graphics state.
        if let Some(logo) = &t.logo {
            // Aspect-preserving uniform scale: the LONGER ink edge becomes
            // 42 pt. Fixed size independent of font size and band height;
            // the ink may overflow the band into the top margin.
            const LOGO_SIZE: f32 = 42.0;
            let ink_w_px = (logo.trim_right - logo.trim_left) as f32;
            let ink_h_px = (logo.trim_bottom - logo.trim_top) as f32;
            let scale = LOGO_SIZE / ink_w_px.max(ink_h_px);
            let ink_w = ink_w_px * scale;
            let ink_h = ink_h_px * scale;
            // Without title text the band shrinks to nothing
            // (`title_height == 0`): the ink overlays the unchanged page,
            // its top flush with the paper margin — the same corner the
            // band occupies with text, but the grid keeps its full height.
            let text_center = if t.text.is_empty() {
                band_top - ink_h / 2.0
            } else {
                // Cap-height text visual center (half of the 0.718 em).
                let baseline_y = band_top - t.font_size * 0.718;
                baseline_y + t.font_size * 0.359
            };
            // Ink rect centered on the text (or pinned to the corner),
            // ink right edge flush with the writing lines (`writing_right`).
            let ink_bottom = text_center - ink_h / 2.0;
            let ink_left = writing_right - ink_w;
            // Back to full-image placement: the image rect extends past the
            // ink rect by the transparent border, in the same uniform scale.
            let img_x = ink_left - logo.trim_left as f32 * scale;
            let img_y = ink_bottom - (logo.height - logo.trim_bottom) as f32 * scale;
            let img_w = logo.width as f32 * scale;
            let img_h = logo.height as f32 * scale;
            content.save_state();
            content.transform([img_w, 0.0, 0.0, img_h, img_x, img_y]);
            content.x_object(logo_name);
            content.restore_state();
        }
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
    // Lines crossing the overlay logo's ink rect stop short of its left
    // edge (plus a 4 pt gap) so they never intersect the logo. With a QR
    // code, the bottom line (y = page margin) stops short of the QR's ink
    // rect (plus a 4 pt gap); all other lines keep their full length.
    content.set_line_width(config.writing_line_width());
    let qr_left = if qr.is_some() {
        Some(writing_right - QR_SIZE)
    } else {
        None
    };
    for row in 1..=row_count {
        let y_position = row_top(row as isize);
        let mut end = match overlay {
            Some((overlay_left, overlay_bottom, _, overlay_top))
                if y_position > overlay_bottom && y_position < overlay_top =>
            {
                overlay_left - 4.0
            }
            _ => writing_right,
        };
        if qr_left.is_some() && approx_eq_pt(y_position, config.page.margin) {
            end = end.min(qr_left.unwrap() - 4.0);
        }
        content.move_to(writing_left, y_position);
        content.line_to(end, y_position);
    }
    content.stroke();

    // ------------------------------------------------------------------
    // Optional QR code in the bottom-right page corner: ink bottom flush
    // with the bottom margin, right edge flush with the writing lines.
    // ------------------------------------------------------------------
    if let Some(qr) = qr {
        let module = QR_SIZE / qr.size as f32;
        let qr_left = writing_right - QR_SIZE;
        let qr_bottom = config.page.margin;
        for row in 0..qr.size {
            for column in 0..qr.size {
                if !qr.modules[row * qr.size + column] {
                    continue;
                }
                // Matrix row 0 is the top row; PDF y grows upwards.
                let x = qr_left + column as f32 * module;
                let y = qr_bottom + (qr.size - 1 - row) as f32 * module;
                content.rect(x, y, module, module);
            }
        }
        // Default fill color (black) is never changed anywhere in `draw`;
        // modules are non-overlapping rects, so nonzero fill is exact.
        content.fill_nonzero();
    }

    content.finish().to_vec()
}

/// Exact float equality on point coordinates: both sides are computed from
/// the same `row_top(margin)` arithmetic, so the bottom writing line's
/// y equals the page margin bit-for-bit.
fn approx_eq_pt(a: f32, b: f32) -> bool {
    a == b
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
        let pdf = render_pdf(&sheet, &config).unwrap();
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
        let pdf = render_pdf(&sheet, &config).unwrap();
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
        let pdf = render_pdf(&sheet, &config).unwrap();
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
        let pdf = render_pdf(&sheet, &config).unwrap();
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
        let pdf = render_pdf(&sheet, &config).unwrap();
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
        let pdf = render_pdf(&sheet, &config).unwrap();
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
        let pdf = render_pdf(&sheet, &config).unwrap();
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
                ..TitleConfig::default()
            }),
            ..RenderConfig::default()
        };
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
        let pdf = render_pdf(&sheet, &config).unwrap();
        let text = String::from_utf8_lossy(&pdf).into_owned();
        // The content stream is stored uncompressed; "My Calendar" appears
        // verbatim, like the month-name tests.
        assert!(text.contains("My Calendar"));
    }

    /// Parse the `cm` matrix preceding `/<image> Do` from the uncompressed
    /// content stream: `[a, b, c, d, e, f]`.
    fn cm_before_image(text: &str, image: &str) -> [f32; 6] {
        let index = text
            .find(&format!("/{image} Do"))
            .expect("image operator must be present");
        let before = &text[..index];
        let start = before
            .rfind("cm")
            .map(|end| before[..end].trim_end())
            .expect("cm matrix must precede the image operator");
        let numbers: Vec<f32> = start
            .rsplit(char::is_whitespace)
            .map(str::trim)
            .take(6)
            .filter_map(|token| token.parse().ok())
            .collect();
        assert_eq!(numbers.len(), 6, "cm matrix with 6 numbers");
        [
            numbers[5], numbers[4], numbers[3], numbers[2], numbers[1], numbers[0],
        ]
    }

    #[test]
    fn title_with_logo_embeds_image() {
        // 1x1 RGBA PNG (200, 30, 40, 128); same bytes as in `renders::logo`
        // tests (byte literal, no binary fixture file).
        const PNG_1X1_RGBA: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0xd, 0xa, 0x1a, 0xa, 0x0, 0x0, 0x0, 0xd, 0x49, 0x48, 0x44,
            0x52, 0x0, 0x0, 0x0, 0x1, 0x0, 0x0, 0x0, 0x1, 0x8, 0x6, 0x0, 0x0, 0x0, 0x1f, 0x15,
            0xc4, 0x89, 0x0, 0x0, 0x0, 0xd, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x38, 0x21,
            0xa7, 0xd1, 0x0, 0x0, 0x4, 0x4f, 0x1, 0x8f, 0xd1, 0xc9, 0x9e, 0xa5, 0x0, 0x0, 0x0,
            0x0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let logo = crate::renders::logo::decode_png(PNG_1X1_RGBA).unwrap();
        let config = RenderConfig {
            title: Some(TitleConfig {
                text: "My Calendar".to_string(),
                logo: Some(logo),
                ..TitleConfig::default()
            }),
            ..RenderConfig::default()
        };
        // The logo shares the existing title band: identical grid height to
        // a title-only run.
        let title_only = RenderConfig {
            title: Some(TitleConfig {
                text: "My Calendar".to_string(),
                ..TitleConfig::default()
            }),
            ..RenderConfig::default()
        };
        assert_eq!(
            config.page_fill_weeks(),
            title_only.page_fill_weeks(),
            "the logo must not change the band height"
        );
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
        let pdf = render_pdf(&sheet, &config).unwrap();
        let text = String::from_utf8_lossy(&pdf).into_owned();
        assert!(text.contains("Im1"), "image XObject must be registered");
        // Regression guard: writing the resources dict twice used to produce
        // two `/Resources` entries, breaking font lookup in viewers.
        assert_eq!(
            text.matches("/Resources").count(),
            1,
            "exactly one /Resources dictionary"
        );
        assert!(text.contains("/FlateDecode"), "samples must be compressed");
        // The test PNG carries an alpha channel, so a DeviceGray soft mask
        // must be present.
        assert!(text.contains("/DeviceGray"), "SMask must be present");
        assert!(text.contains("My Calendar"));

        // Geometry: parse the `cm` matrix preceding `/Im1 Do` from the
        // uncompressed content stream. The 1x1 opaque-ink logo must draw at
        // exactly 42x42 pt, ink right edge flush with the writing lines
        // (page width - margin), vertically centered on the title text's
        // cap-height center.
        let approx_eq = |a: f32, b: f32| (a - b).abs() < 0.01;
        let [a, _b, _c, d, e, f] = cm_before_image(&text, "Im1");
        assert!(approx_eq(a, 42.0), "cm a = {a}, expected 42");
        assert!(approx_eq(d, 42.0), "cm d = {d}, expected 42");
        // Right edge e + a = page width - margin = 555.0 (writing lines
        // end exactly there).
        assert!(
            approx_eq(e + a, 555.0),
            "right edge = {}, expected 555.0",
            e + a
        );
        // Vertical center f + d/2 = baseline + 0.359 * font size.
        let expected_center = 802.0 - 16.0 * 0.718 + 16.0 * 0.359;
        assert!(
            approx_eq(f + d / 2.0, expected_center),
            "vertical center = {}, expected {expected_center}",
            f + d / 2.0
        );

        // No-stretch rule: a 1x2 px opaque logo must scale uniformly —
        // height is the longer edge (60 pt), width shrinks to keep aspect.
        const PNG_1X2_RGBA: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0xd, 0xa, 0x1a, 0xa, 0x0, 0x0, 0x0, 0xd, 0x49, 0x48, 0x44,
            0x52, 0x0, 0x0, 0x0, 0x1, 0x0, 0x0, 0x0, 0x2, 0x8, 0x6, 0x0, 0x0, 0x0, 0x99, 0x81,
            0xb6, 0x27, 0x0, 0x0, 0x0, 0xf, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xe0, 0x12,
            0x91, 0xfb, 0xcf, 0x0, 0x22, 0x0, 0x9, 0x6f, 0x2, 0x77, 0x24, 0xc5, 0xa3, 0xee, 0x0,
            0x0, 0x0, 0x0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let tall_logo = crate::renders::logo::decode_png(PNG_1X2_RGBA).unwrap();
        let tall_config = RenderConfig {
            title: Some(TitleConfig {
                text: "My Calendar".to_string(),
                logo: Some(tall_logo),
                ..TitleConfig::default()
            }),
            ..RenderConfig::default()
        };
        let tall_sheet = Sheet::new(
            date(2026, 9, 15),
            tall_config.page_fill_weeks(),
            Language::English,
        );
        let tall_text =
            String::from_utf8_lossy(&render_pdf(&tall_sheet, &tall_config).unwrap()).into_owned();
        let [a, _b, _c, d, _e, _f] = cm_before_image(&tall_text, "Im1");
        assert!(approx_eq(a, 21.0), "cm a = {a}, expected 21 (42 * 1/2)");
        assert!(approx_eq(d, 42.0), "cm d = {d}, expected 42");
    }

    #[test]
    fn logo_without_title_overlays_unchanged_page() {
        // 1x1 RGBA PNG (200, 30, 40, 128); same bytes as in `renders::logo`
        // tests (byte literal, no binary fixture file).
        const PNG_1X1_RGBA: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0xd, 0xa, 0x1a, 0xa, 0x0, 0x0, 0x0, 0xd, 0x49, 0x48, 0x44,
            0x52, 0x0, 0x0, 0x0, 0x1, 0x0, 0x0, 0x0, 0x1, 0x8, 0x6, 0x0, 0x0, 0x0, 0x1f, 0x15,
            0xc4, 0x89, 0x0, 0x0, 0x0, 0xd, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x38, 0x21,
            0xa7, 0xd1, 0x0, 0x0, 0x4, 0x4f, 0x1, 0x8f, 0xd1, 0xc9, 0x9e, 0xa5, 0x0, 0x0, 0x0,
            0x0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        // Logo with empty title text must not create a band: the page
        // (grid row count) stays identical to a title-less run, and the
        // logo ink is pinned to the top-right corner — flush with the top
        // margin and the writing lines' right end.
        let logo_only = RenderConfig {
            title: Some(TitleConfig {
                text: String::new(),
                logo: Some(crate::renders::logo::decode_png(PNG_1X1_RGBA).unwrap()),
                ..TitleConfig::default()
            }),
            ..RenderConfig::default()
        };
        let no_title = RenderConfig::default();
        assert_eq!(
            logo_only.page_fill_weeks(),
            no_title.page_fill_weeks(),
            "a logo without title text must not change the row count"
        );
        let sheet = Sheet::new(
            date(2026, 9, 15),
            logo_only.page_fill_weeks(),
            Language::English,
        );
        let text = String::from_utf8_lossy(&render_pdf(&sheet, &logo_only).unwrap()).into_owned();
        let [a, _b, _c, d, e, f] = cm_before_image(&text, "Im1");
        let approx_eq = |a: f32, b: f32| (a - b).abs() < 0.01;
        assert!(approx_eq(a, 42.0), "cm a = {a}, expected 42");
        assert!(approx_eq(d, 42.0), "cm d = {d}, expected 42");
        // Ink right edge flush with the writing lines.
        assert!(
            approx_eq(e + a, logo_only.page.width - logo_only.page.margin),
            "right edge = {}, expected {}",
            e + a,
            logo_only.page.width - logo_only.page.margin
        );
        // Ink top flush with the paper margin (page height - margin).
        assert!(
            approx_eq(f + d, logo_only.page.height - logo_only.page.margin),
            "ink top = {}, expected {}",
            f + d,
            logo_only.page.height - logo_only.page.margin
        );

        // Writing lines crossing the logo ink rect must stop short of it:
        // 42 pt ink spanning y in [760, 802]; with the default config only
        // the first writing line (y = 802 - 762/31 = 777.42) falls inside,
        // ending at ink_left - 4 = 509.0, everything else at 555.0.
        // Writing lines are `x1 y m x2 y l` pairs between `m`/`l` ops.
        let mut line_ends: Vec<(f32, f32)> = Vec::new();
        for segment in text.split(" l") {
            // `x1 y m\nx2 y` -> endpoint from the part after the `m`.
            if let Some((_start, tail)) = segment.rsplit_once(" m") {
                let coords: Vec<f32> = tail
                    .rsplit(char::is_whitespace)
                    .map(str::trim)
                    .take(2)
                    .filter_map(|token| token.parse().ok())
                    .collect();
                if coords.len() == 2 {
                    line_ends.push((coords[0], coords[1]));
                }
            }
        }
        let logo_only_line_end = 555.0 - 42.0 - 4.0;
        let shortened: Vec<_> = line_ends
            .iter()
            .filter(|&&(_y, x2)| approx_eq(x2, logo_only_line_end))
            .collect();
        assert_eq!(
            shortened.len(),
            1,
            "exactly one line shortened (y 777.42): {line_ends:?}"
        );
        for &&(y, x2) in &shortened {
            assert!(
                y > 802.0 - 42.0 && y < 802.0,
                "shortened line y = {y} must cross the logo ink"
            );
            assert!(
                approx_eq(x2, logo_only_line_end),
                "shortened line ends at {x2}, expected {logo_only_line_end}"
            );
        }
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
        let pdf = render_pdf(&sheet, &config).unwrap();
        let text = String::from_utf8_lossy(&pdf).into_owned();
        // No title text should appear.
        assert!(!text.contains("My Calendar"));
    }

    #[test]
    fn qr_code_keeps_grid_and_shortens_bottom_line() {
        let plain = RenderConfig::default();
        let mut with_qr = RenderConfig::default();
        with_qr.qr_code = Some("https://example.com/".to_string());
        // The QR sits in the corner beside the lines: the grid (and row
        // count) is unchanged from a plain page.
        assert_eq!(plain.page_fill_weeks(), with_qr.page_fill_weeks());
        let sheet = Sheet::new(
            date(2026, 9, 15),
            with_qr.page_fill_weeks(),
            Language::English,
        );
        let text = String::from_utf8_lossy(&render_pdf(&sheet, &with_qr).unwrap()).into_owned();

        // Dark modules are drawn as `x y w h re` rects filled with `f`.
        // Month-name boxes use the same operator, so keep only rects inside
        // the QR ink area (x >= 535, y in [40, 60]).
        let qr = crate::renders::qr::encode_qr("https://example.com/").unwrap();
        let dark_modules = qr.modules.iter().filter(|&&dark| dark).count();
        let rect_tokens: Vec<Vec<f32>> = text
            .split(" re")
            .skip(1)
            .filter_map(|head| {
                let start = head.rfind(" re").map(|_| head).unwrap_or(head);
                let tail: Vec<f32> = start
                    .rsplit(char::is_whitespace)
                    .map(str::trim)
                    .take(4)
                    .filter_map(|token| token.parse().ok())
                    .collect();
                (tail.len() == 4).then(|| {
                    let mut it = tail.into_iter();
                    let h = it.next().unwrap();
                    let w = it.next().unwrap();
                    let y = it.next().unwrap();
                    let x = it.next().unwrap();
                    vec![x, y, w, h]
                })
            })
            .filter(|r| r[0] >= 534.0 && (40.0..=60.0).contains(&r[1]))
            .collect();
        assert_eq!(
            rect_tokens.len(),
            dark_modules,
            "one rect per dark module ({} vs {})",
            rect_tokens.len(),
            dark_modules
        );
        assert!(text.matches(" f").count() >= 1, "fill operator present");
        let approx_eq = |a: f32, b: f32| (a - b).abs() < 0.01;
        let min_x = rect_tokens.iter().map(|r| r[0]).fold(f32::INFINITY, f32::min);
        let max_right = rect_tokens
            .iter()
            .map(|r| r[0] + r[2])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = rect_tokens.iter().map(|r| r[1]).fold(f32::INFINITY, f32::min);
        let max_top = rect_tokens
            .iter()
            .map(|r| r[1] + r[3])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(approx_eq(min_x, 535.0), "min x = {min_x}, expected 535");
        assert!(approx_eq(max_right, 555.0), "max right = {max_right}, expected 555");
        assert!(approx_eq(min_y, 40.0), "min y = {min_y}, expected 40");
        assert!(approx_eq(max_top, 60.0), "max top = {max_top}, expected 60");

        // The bottom writing line (y = 40) stops short of the QR ink
        // (535) with a 4 pt gap -> ends at x = 531; every other writing
        // line ends at the full 555. Writing lines start at x = 228
        // (calendar right + month-cell width), so key on that start:
        // `228 <y> m <x> <y> l` with equal y on both points.
        let mut shortened = 0;
        let mut full = 0;
        for segment in text.split(" l") {
            let Some((start, tail)) = segment.rsplit_once(" m") else {
                continue;
            };
            let start_tail: Vec<f32> = start
                .rsplit(char::is_whitespace)
                .map(str::trim)
                .take(2)
                .filter_map(|token| token.parse().ok())
                .collect();
            let coords: Vec<f32> = tail
                .rsplit(char::is_whitespace)
                .map(str::trim)
                .take(2)
                .filter_map(|token| token.parse().ok())
                .collect();
            // start_tail = [y1, x1] with x1 = 228; coords = [y2, x2].
            if start_tail.len() == 2
                && coords.len() == 2
                && approx_eq(start_tail[1], 228.0)
                && approx_eq(start_tail[0], coords[0])
            {
                if approx_eq(coords[0], 40.0) {
                    assert!(
                        approx_eq(coords[1], 531.0),
                        "bottom line ends at {}, expected 531 (QR left 535 - 4 gap)",
                        coords[1]
                    );
                    shortened += 1;
                } else if approx_eq(coords[1], 555.0) {
                    full += 1;
                }
            }
        }
        assert_eq!(shortened, 1, "exactly one shortened bottom line");
        assert!(full > 20, "all other writing lines keep full length");
    }

    #[test]
    fn qr_code_too_long_fails() {
        let mut config = RenderConfig::default();
        config.qr_code = Some("x".repeat(10_000));
        let sheet = Sheet::new(
            date(2026, 9, 15),
            config.page_fill_weeks(),
            Language::English,
        );
        let error = render_pdf(&sheet, &config).unwrap_err();
        assert!(
            matches!(error, crate::RecordError::InvalidQrCode(_)),
            "unexpected error: {error:?}"
        );
        assert!(error.to_string().contains("invalid QR code"));
    }
}
