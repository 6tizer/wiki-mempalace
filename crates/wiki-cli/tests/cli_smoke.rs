use assert_cmd::Command;
use predicates::str::contains;

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

#[test]
fn global_db_arg_before_subcommand_still_works() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("metrics")
        .assert()
        .success()
        .stdout(contains("metrics report:"));
}

#[test]
fn global_db_arg_after_subcommand_is_rejected() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("metrics")
        .arg("--db")
        .arg(&db_path)
        .assert()
        .failure()
        .stderr(contains("unexpected argument"));
}
