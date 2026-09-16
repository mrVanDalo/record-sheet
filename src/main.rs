//! CLI: generate a printable record-sheet PDF for the week grid starting at
//! a given date (default: today).

use clap::{Parser, ValueEnum};
use jiff::civil::Date;
use record_sheet::{render_pdf, Language, RenderConfig, Sheet};
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

    let config = RenderConfig::default();
    let sheet = Sheet::new(
        start,
        config.page_fill_weeks(),
        command_line.language.into(),
    );
    let pdf = render_pdf(&sheet, &config);

    let output_path = command_line
        .output
        .unwrap_or_else(|| PathBuf::from(format!("record-sheet-{}.pdf", sheet.start)));
    std::fs::write(&output_path, pdf)?;
    println!("Wrote {}", output_path.display());
    Ok(())
}
