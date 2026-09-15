/// Integration tests for the record-sheet CLI binary.
use assert_cmd::Command;
#[test]
fn cli_default_uses_today() {
    let mut cmd = Command::cargo_bin("record-sheet").unwrap();
    // Run in a temp dir so the default-named PDF doesn't pollute the repo.
    let tmp = assert_fs::TempDir::new().unwrap();
    cmd.current_dir(&tmp);
    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("Wrote record-sheet-"));
    assert!(stdout.contains(".pdf"));
    let mut pdfs: Vec<_> = std::fs::read_dir(&tmp)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "pdf"))
        .collect();
    assert_eq!(pdfs.len(), 1, "exactly one PDF written");
    let bytes = std::fs::read(pdfs.remove(0)).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn cli_with_date_argument() {
    let mut cmd = Command::cargo_bin("record-sheet").unwrap();
    let tmp = assert_fs::TempDir::new().unwrap();
    cmd.current_dir(&tmp);
    let output = cmd
        .args(["2026-09-15", "-o", "sheet.pdf"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("Wrote sheet.pdf"));
    let bytes = std::fs::read(tmp.join("sheet.pdf")).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 1000);
}

#[test]
fn cli_rejects_invalid_date() {
    let mut cmd = Command::cargo_bin("record-sheet").unwrap();
    let tmp = assert_fs::TempDir::new().unwrap();
    cmd.current_dir(&tmp);
    let output = cmd.args(["not-a-date"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("invalid date"), "stderr: {stderr}");
}

#[test]
fn cli_help_mentions_date() {
    let mut cmd = Command::cargo_bin("record-sheet").unwrap();
    let output = cmd.arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("date"));
    assert!(stdout.contains("output"));
}
