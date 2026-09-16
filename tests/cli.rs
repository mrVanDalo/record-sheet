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
