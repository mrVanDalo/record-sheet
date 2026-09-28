# record-sheet

Generate a printable record sheet PDF: a dated calendar grid with writing
lines for each day, ready to print and fill in by hand.

## Usage

```console
$ record-sheet --help
Generate a printable record sheet PDF

Usage: record-sheet [OPTIONS] [DATE]

Arguments:
  [DATE]  Start date in ISO format (YYYY-MM-DD). Defaults to today

Options:
  -o, --output <FILE>        Output PDF path. Defaults to record-sheet-<start>.pdf in the current directory
  -l, --language <LANGUAGE>  Language of the weekday headings and month names [default: en] [possible values: en, de, es]
  -t, --title <TEXT>         Optional title printed above the calendar grid; shortens the grid
      --logo <FILE>          Optional PNG logo in the top-right: beside the title text with `--title`, overlaid on the unchanged page corner without
      --qr-code <TEXT>       Optional text rendered as a small QR code in the bottom-right corner of the page; the last writing line is shortened to make room for it
  -h, --help                 Print help
  -V, --version              Print version
```

### Examples

```console
# A sheet starting today, written to ./record-sheet-<today>.pdf
record-sheet

# A sheet starting at a given date, in German
record-sheet 2026-09-15 -l de

# Titled sheet with a logo and a QR code linking to a website
record-sheet 2026-09-15 -t "Homework log" --logo logo.png --qr-code "https://example.com/"
```

## WASM version

There is an online version that runs entirely in your browser (WebAssembly),
no installation required:

**https://mrvandalo.github.io/record-sheet/**

The web version supports start date, language, title, and logo, but not the
QR code option and not a custom output path. For full control over the
generated sheet, use the Rust CLI below.

## Building

Requires a Rust toolchain (or Nix):

```console
cargo build --release
# binary in target/release/record-sheet
```

With Nix:

```console
nix build            # builds the record-sheet CLI
nix run . -- --help  # run directly
```

To preview the WASM site locally:

```console
nix run .#wasm-build-tmp
python3 -m http.server -d website/tmp 8000
```

## License

See the repository for licensing details.
