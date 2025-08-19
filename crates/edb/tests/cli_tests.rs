use assert_cmd::Command;
use predicates::prelude::*;
use tracing::{debug, info, warn};

#[test]
fn test_help_command() {
    edb_common::logging::ensure_test_logging(None);
    info!("Testing CLI help command");

    let mut cmd = Command::cargo_bin("edb").unwrap();
    cmd.arg("--help").assert().success().stdout(predicate::str::contains("Ethereum Debugger"));
}

#[test]
fn test_version_command() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let mut cmd = Command::cargo_bin("edb").unwrap();
    cmd.arg("--version").assert().success().stdout(predicate::str::contains("edb"));
}

#[test]
fn test_replay_subcommand_help() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let mut cmd = Command::cargo_bin("edb").unwrap();
    cmd.arg("replay")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Replay an existing transaction"));
}

#[test]
fn test_test_subcommand_help() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let mut cmd = Command::cargo_bin("edb").unwrap();
    cmd.arg("test")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Debug a Foundry test case"));
}

#[test]
fn test_invalid_tx_hash() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let mut cmd = Command::cargo_bin("edb").unwrap();
    cmd.arg("replay").arg("invalid_hash").assert().failure();
}

#[test]
fn test_missing_subcommand() {
    edb_common::logging::ensure_test_logging(None);
    info!("Running test");
    let mut cmd = Command::cargo_bin("edb").unwrap();
    cmd.assert().failure().stderr(predicate::str::contains("Usage"));
}
