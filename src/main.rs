//! CLI: generate a printable record-sheet PDF for the week grid starting at
//! a given date (default: today).

use clap::Parser;
use jiff::civil::Date;
use record_sheet::{render_pdf, RenderConfig, Sheet};
use std::path::PathBuf;

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
    let sheet = Sheet::new(start, config.page_fill_weeks());
    let pdf = render_pdf(&sheet, &config);

    let output_path = command_line
        .output
        .unwrap_or_else(|| PathBuf::from(format!("record-sheet-{}.pdf", sheet.start)));
    std::fs::write(&output_path, pdf)?;
    println!("Wrote {}", output_path.display());
    Ok(())
}
