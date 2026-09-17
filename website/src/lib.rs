//! WASM bindings: generate a record-sheet PDF in the browser.
use record_sheet::{render_pdf, Language, RenderConfig, Sheet, TitleConfig};
use wasm_bindgen::prelude::*;

/// Generate a record-sheet PDF for the week grid starting at `date_iso`.
///
/// `date_iso`: ISO date string (YYYY-MM-DD), resolved by the JS side
/// ("today" is computed in JS; the Rust lib never calls `now()` — on
/// wasm that would panic without jiff's `js` feature).
/// `language`: "en" | "de" | "es"; any other string falls back to
/// English (the site UI is a fixed dropdown; the CLI by contrast
/// rejects unknown languages — documented divergence).
/// `title`: optional title text printed above the calendar grid.
/// Returns the raw PDF bytes.
#[wasm_bindgen]
pub fn generate_pdf(
    date_iso: String,
    language: String,
    title: Option<String>,
) -> Result<Vec<u8>, JsError> {
    let start: jiff::civil::Date = date_iso
        .parse()
        .map_err(|e| JsError::new(&format!("invalid date {date_iso:?}: {e}")))?;
    let language = match language.as_str() {
        "de" => Language::German,
        "es" => Language::Spanish,
        _ => Language::English,
    };
    let mut config = RenderConfig::default();
    if let Some(text) = title.as_deref() {
        config.title = Some(TitleConfig {
            text: text.to_owned(),
            ..TitleConfig::default()
        });
    }
    let sheet = Sheet::new(start, config.page_fill_weeks(), language);
    Ok(render_pdf(&sheet, &config))
}
