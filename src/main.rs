//! CLI: generate a printable record-sheet PDF for the week grid starting at
//! a given date (default: today).

use clap::Parser;
use jiff::civil::Date;
use record_sheet::{render::PAGE_FILL_WEEKS, render_pdf, Sheet};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Generate a printable record sheet PDF")]
struct Cli {
    /// Start date in ISO format (YYYY-MM-DD). Defaults to today.
    #[arg(value_parser = parse_date)]
    date: Option<Date>,

    /// Output PDF path. Defaults to record-sheet-<start>.pdf in the current
    /// directory.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

fn parse_date(s: &str) -> Result<Date, String> {
    s.parse::<Date>()
        .map_err(|e| format!("invalid date {s:?}: {e}"))
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let start = cli.date.unwrap_or_else(|| Date::from(jiff::Zoned::now()));

    let sheet = Sheet::new(start, PAGE_FILL_WEEKS);
    let pdf = render_pdf(&sheet);

    let out = cli
        .output
        .unwrap_or_else(|| PathBuf::from(format!("record-sheet-{}.pdf", sheet.start)));
    std::fs::write(&out, pdf)?;
    println!("Wrote {}", out.display());
    Ok(())
}
