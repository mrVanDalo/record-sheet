//! WASM bindings: generate a record-sheet PDF in the browser.
use record_sheet::{decode_png, render_pdf, Language, RenderConfig, Sheet, TitleConfig};
use wasm_bindgen::prelude::*;

/// QR code rendered in the bottom-right corner of every website
/// PDF: the site's own URL (the site never takes QR input).
const SITE_URL: &str = "https://mrvandalo.github.io/record-sheet/";

/// Generate a record-sheet PDF for the week grid starting at `date_iso`.
///
/// `date_iso`: ISO date string (YYYY-MM-DD), resolved by the JS side
/// ("today" is computed in JS; the Rust lib never calls `now()` — on
/// wasm that would panic without jiff's `js` feature).
/// `language`: "en" | "de" | "es"; any other string falls back to
/// English (the site UI is a fixed dropdown; the CLI by contrast
/// rejects unknown languages — documented divergence).
/// `title`: optional title text printed above the calendar grid.
/// `logo`: PNG bytes for an optional logo in the top-right: beside the
/// title text with a non-empty `title`, overlaid on the unchanged page
/// corner (writing lines shortened to clear its ink) without; an empty
/// `Vec` means no logo (an `Option<Vec<u8>>` binding
/// is not supported by the pinned wasm-bindgen 0.2.127). A non-empty,
/// non-PNG `Vec` is an error.
/// The PDF always carries a QR code in the bottom-right corner encoding
/// `SITE_URL` (the site's own URL) — the site never takes QR input.
/// Returns the raw PDF bytes.
#[wasm_bindgen]
pub fn generate_pdf(
    date_iso: String,
    language: String,
    title: Option<String>,
    logo: Vec<u8>,
) -> Result<Vec<u8>, JsError> {
    let start: jiff::civil::Date = date_iso
        .parse()
        .map_err(|e| JsError::new(&format!("invalid date {date_iso:?}: {e}")))?;
    let language = match language.as_str() {
        "de" => Language::German,
        "es" => Language::Spanish,
        _ => Language::English,
    };
    let logo = if logo.is_empty() {
        None
    } else {
        decode_png(&logo)
            .map(Some)
            .map_err(|e| JsError::new(&e.to_string()))?
    };
    let mut config = RenderConfig::default();
    if title.is_some() || logo.is_some() {
        config.title = Some(TitleConfig {
            text: title.unwrap_or_default(),
            logo,
            ..TitleConfig::default()
        });
    }
    config.qr_code = Some(SITE_URL.to_string());
    let sheet = Sheet::new(start, config.page_fill_weeks(), language);
    render_pdf(&sheet, &config).map_err(|e| JsError::new(&e.to_string()))
}