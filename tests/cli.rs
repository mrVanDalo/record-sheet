/// Integration tests for the record-sheet CLI binary.
use assert_cmd::Command;
#[test]
fn cli_default_uses_today() {
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    // Run in a temp dir so the default-named PDF doesn't pollute the repo.
    let temp_dir = assert_fs::TempDir::new().unwrap();
    command.current_dir(&temp_dir);
    let output = command.output().unwrap();
    assert!(output.status.success());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("Wrote record-sheet-"));
    assert!(stdout.contains(".pdf"));
    let mut pdfs: Vec<_> = std::fs::read_dir(&temp_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "pdf"))
        .collect();
    assert_eq!(pdfs.len(), 1, "exactly one PDF written");
    let bytes = std::fs::read(pdfs.remove(0)).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn cli_with_date_argument() {
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    let temp_dir = assert_fs::TempDir::new().unwrap();
    command.current_dir(&temp_dir);
    let output = command
        .args(["2026-09-15", "-o", "sheet.pdf"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("Wrote sheet.pdf"));
    let bytes = std::fs::read(temp_dir.join("sheet.pdf")).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 1000);
}

#[test]
fn cli_language_flag_changes_month_names() {
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    let temp_dir = assert_fs::TempDir::new().unwrap();
    command.current_dir(&temp_dir);
    let output = command
        .args(["2026-09-15", "-o", "de.pdf", "--language", "de"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let bytes = std::fs::read(temp_dir.join("de.pdf")).unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    // September spans enough cells for its full German name.
    assert!(text.contains("September"));
    assert!(text.contains("Oktober"));
}

#[test]
fn cli_rejects_unknown_language() {
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    let output = command.args(["--language", "fr"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("invalid value"), "stderr: {stderr}");
}

#[test]
fn cli_rejects_invalid_date() {
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    let temp_dir = assert_fs::TempDir::new().unwrap();
    command.current_dir(&temp_dir);
    let output = command.args(["not-a-date"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("invalid date"), "stderr: {stderr}");
}

#[test]
fn cli_help_mentions_date() {
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    let output = command.arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("date"));
    assert!(stdout.contains("output"));
}

#[test]
fn cli_title_argument_writes_pdf() {
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    let temp_dir = assert_fs::TempDir::new().unwrap();
    command.current_dir(&temp_dir);
    let output = command
        .args(["2026-09-15", "-o", "title.pdf", "--title", "Week 38"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let bytes = std::fs::read(temp_dir.join("title.pdf")).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("Week 38"));
}

#[test]
fn cli_no_title_keeps_default_layout() {
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    let temp_dir = assert_fs::TempDir::new().unwrap();
    command.current_dir(&temp_dir);
    let output = command
        .args(["2026-09-15", "-o", "default.pdf"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("Wrote"));
    let bytes = std::fs::read(temp_dir.join("default.pdf")).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

/// 1x1 RGBA PNG (200, 30, 40, 128) as raw bytes; same literal as in
/// `src/renders/logo.rs` and `src/renders/pdf.rs` tests.
const PNG_1X1_RGBA: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0xd, 0xa, 0x1a, 0xa, 0x0, 0x0, 0x0, 0xd, 0x49, 0x48, 0x44, 0x52, 0x0,
    0x0, 0x0, 0x1, 0x0, 0x0, 0x0, 0x1, 0x8, 0x6, 0x0, 0x0, 0x0, 0x1f, 0x15, 0xc4, 0x89, 0x0, 0x0,
    0x0, 0xd, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x38, 0x21, 0xa7, 0xd1, 0x0, 0x0, 0x4,
    0x4f, 0x1, 0x8f, 0xd1, 0xc9, 0x9e, 0xa5, 0x0, 0x0, 0x0, 0x0, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

#[test]
fn cli_logo_with_invalid_png_fails() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let logo_path = temp_dir.join("logo.png");
    std::fs::write(&logo_path, b"not a png").unwrap();
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    command.current_dir(&temp_dir);
    let output = command
        .args(["2026-09-15", "-o", "out.pdf", "--logo"])
        .arg(&logo_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("logo.png"), "stderr: {stderr}");
    assert!(stderr.contains("invalid PNG logo"), "stderr: {stderr}");
    assert!(!temp_dir.join("out.pdf").exists(), "no PDF must be written");
}

#[test]
fn cli_logo_writes_pdf() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let logo_path = temp_dir.join("logo.png");
    std::fs::write(&logo_path, PNG_1X1_RGBA).unwrap();
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    command.current_dir(&temp_dir);
    let output = command
        .args(["2026-09-15", "-o", "out.pdf", "--title", "Week 38", "--logo"])
        .arg(&logo_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(temp_dir.join("out.pdf")).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("Week 38"));
    assert!(text.contains("Im1"));
    assert!(text.contains("/FlateDecode"));
}

#[test]
fn cli_logo_without_title_writes_pdf() {
    let temp_dir = assert_fs::TempDir::new().unwrap();
    let logo_path = temp_dir.join("logo.png");
    std::fs::write(&logo_path, PNG_1X1_RGBA).unwrap();
    let mut command = Command::cargo_bin("record-sheet").unwrap();
    command.current_dir(&temp_dir);
    let output = command
        .args(["2026-09-15", "-o", "out.pdf", "--logo"])
        .arg(&logo_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(temp_dir.join("out.pdf")).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("Im1"));
}
