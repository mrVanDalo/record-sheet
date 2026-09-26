//! CLI: generate a printable record-sheet PDF for the week grid starting at
//! a given date (default: today).

use anyhow::Context as _;
use clap::{Parser, ValueEnum};
use jiff::civil::Date;
use record_sheet::{render_pdf, Language, RenderConfig, Sheet, TitleConfig};
use std::path::PathBuf;

/// The `--language` values. A manual [`ValueEnum`] impl keeps the clap
/// dependency out of the library.
#[derive(Debug, Clone, Copy)]
struct LanguageArg(Language);

impl ValueEnum for LanguageArg {
    fn value_variants<'a>() -> &'a [LanguageArg] {
        const VARIANTS: [LanguageArg; 3] = [
            LanguageArg(Language::English),
            LanguageArg(Language::German),
            LanguageArg(Language::Spanish),
        ];
        &VARIANTS
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self.0 {
            Language::English => clap::builder::PossibleValue::new("en"),
            Language::German => clap::builder::PossibleValue::new("de"),
            Language::Spanish => clap::builder::PossibleValue::new("es"),
        })
    }
}

impl From<LanguageArg> for Language {
    fn from(language_arg: LanguageArg) -> Language {
        language_arg.0
    }
}

#[derive(Parser)]
#[command(version, about = "Generate a printable record sheet PDF")]
struct CommandLine {
    /// Start date in ISO format (YYYY-MM-DD). Defaults to today.
    #[arg(value_parser = parse_date)]
    date: Option<Date>,

    /// Output PDF path. Defaults to record-sheet-<start>.pdf in the current
    /// directory.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Language of the weekday headings and month names.
    #[arg(short, long, value_enum, default_value = "en")]
    language: LanguageArg,

    /// Optional title printed above the calendar grid; shortens the grid.
    #[arg(short, long, value_name = "TEXT")]
    title: Option<String>,

    /// Optional PNG logo in the top-right: beside the title text with
    /// `--title`, overlaid on the unchanged page corner without.
    #[arg(long, value_name = "FILE")]
    logo: Option<PathBuf>,

    /// Optional text rendered as a small QR code in the bottom-right
    /// corner of the page; the last writing line is shortened to make
    /// room for it.
    #[arg(long, value_name = "TEXT")]
    qr_code: Option<String>,
}

fn parse_date(input: &str) -> Result<Date, String> {
    input
        .parse::<Date>()
        .map_err(|error| format!("invalid date {input:?}: {error}"))
}

fn main() -> anyhow::Result<()> {
    let command_line = CommandLine::parse();
    let start = command_line
        .date
        .unwrap_or_else(|| Date::from(jiff::Zoned::now()));

    let mut config = RenderConfig::default();
    let logo_path = command_line.logo.as_deref();
    let logo = logo_path
        .map(std::fs::read)
        .transpose()
        .with_context(|| format!("failed to read the logo file {}", logo_path.unwrap().display()))?
        .map(|bytes| record_sheet::decode_png(&bytes))
        .transpose()
        .with_context(|| format!("failed to decode the logo file {}", logo_path.unwrap().display()))?;
    if command_line.title.is_some() || logo.is_some() {
        config.title = Some(TitleConfig {
            text: command_line.title.unwrap_or_default(),
            logo,
            ..TitleConfig::default()
        });
    }
    config.qr_code = command_line.qr_code;
    let sheet = Sheet::new(
        start,
        config.page_fill_weeks(),
        command_line.language.into(),
    );
    let pdf = render_pdf(&sheet, &config)?;

    let output_path = command_line
        .output
        .unwrap_or_else(|| PathBuf::from(format!("record-sheet-{}.pdf", sheet.start)));
    std::fs::write(&output_path, pdf)?;
    println!("Wrote {}", output_path.display());
    Ok(())
}
