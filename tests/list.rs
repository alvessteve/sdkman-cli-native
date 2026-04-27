#[cfg(test)]
use std::env;
use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use support::{TestCandidate, VirtualEnv};

mod support;

#[test]
#[serial]
fn should_list_all_candidates() -> Result<(), Box<dyn std::error::Error>> {
    let env = VirtualEnv {
        cli_version: "5.0.0".to_string(),
        native_version: "0.1.0".to_string(),
        candidates: vec![
            TestCandidate {
                name: "java",
                versions: vec!["11.0.15-tem"],
                current_version: "11.0.15-tem",
            },
            TestCandidate {
                name: "kotlin",
                versions: vec!["1.7.22"],
                current_version: "1.7.22",
            },
        ],
    };

    let sdkman_dir = support::virtual_env(env);
    env::set_var("SDKMAN_DIR", sdkman_dir.path().as_os_str());

    let contains_header = predicate::str::contains("Available Candidates");
    let contains_separator = predicate::str::contains(
        "--------------------------------------------------------------------------------",
    );
    let contains_java = predicate::str::contains("java");
    let contains_kotlin = predicate::str::contains("kotlin");

    Command::new(assert_cmd::cargo::cargo_bin!("list"))
        .assert()
        .success()
        .stdout(
            contains_header
                .and(contains_separator)
                .and(contains_java)
                .and(contains_kotlin),
        )
        .code(0);

    Ok(())
}

#[test]
#[serial]
fn should_list_installed_versions_for_candidate() -> Result<(), Box<dyn std::error::Error>> {
    let name = "java";
    let versions = vec!["11.0.15-tem", "17.0.3-tem", "21.0.0-tem"];
    let current_version = "17.0.3-tem";

    let env = VirtualEnv {
        cli_version: "5.0.0".to_string(),
        native_version: "0.1.0".to_string(),
        candidates: vec![TestCandidate {
            name,
            versions: versions.clone(),
            current_version,
        }],
    };

    let sdkman_dir = support::virtual_env(env);
    env::set_var("SDKMAN_DIR", sdkman_dir.path().as_os_str());
    env::set_var("SDKMAN_OFFLINE_MODE", "true");

    let contains_header =
        predicate::str::contains(format!("Offline: only showing installed {} versions", name));
    let contains_separator = predicate::str::contains(
        "--------------------------------------------------------------------------------",
    );
    let contains_version1 = predicate::str::contains("11.0.15-tem");
    let contains_version2 = predicate::str::contains("17.0.3-tem");
    let contains_version3 = predicate::str::contains("21.0.0-tem");
    let contains_installed_marker = predicate::str::contains("* - installed");
    let contains_current_marker = predicate::str::contains("> - currently in use");

    Command::new(assert_cmd::cargo::cargo_bin!("list"))
        .arg(name)
        .assert()
        .success()
        .stdout(
            contains_header
                .and(contains_separator)
                .and(contains_version1)
                .and(contains_version2)
                .and(contains_version3)
                .and(contains_installed_marker)
                .and(contains_current_marker),
        )
        .code(0);

    env::remove_var("SDKMAN_OFFLINE_MODE");

    Ok(())
}

#[test]
#[serial]
fn should_mark_current_version_with_arrow_and_others_with_asterisk(
) -> Result<(), Box<dyn std::error::Error>> {
    let name = "java";
    let current_version = "17.0.3-tem";
    let other_version = "11.0.15-tem";
    let versions = vec!["11.0.15-tem", "17.0.3-tem"];

    let env = VirtualEnv {
        cli_version: "5.0.0".to_string(),
        native_version: "0.1.0".to_string(),
        candidates: vec![TestCandidate {
            name,
            versions: versions.clone(),
            current_version,
        }],
    };

    let sdkman_dir = support::virtual_env(env);
    env::set_var("SDKMAN_DIR", sdkman_dir.path().as_os_str());
    env::set_var("SDKMAN_OFFLINE_MODE", "true");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("list"))
        .arg(name)
        .output()
        .expect("Failed to execute command");

    env::remove_var("SDKMAN_OFFLINE_MODE");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8");

    // Check that current version is marked with >
    assert!(
        stdout.contains(&format!("> {}", current_version)),
        "Current version should be marked with '>'"
    );

    // Check that other installed version is marked with *
    assert!(
        stdout.contains(&format!("* {}", other_version)),
        "Installed (non-current) version should be marked with '*'"
    );

    Ok(())
}

#[test]
#[serial]
fn should_error_for_non_existent_candidate() -> Result<(), Box<dyn std::error::Error>> {
    let invalid_name = "invalid";

    let env = VirtualEnv {
        cli_version: "5.0.0".to_string(),
        native_version: "0.1.0".to_string(),
        candidates: vec![],
    };

    let sdkman_dir = support::virtual_env(env);

    // Write at least one valid candidate to avoid empty candidates list error
    support::write_file(
        sdkman_dir.path(),
        Path::new("var"),
        "candidates",
        "java".to_string(),
    );

    env::set_var("SDKMAN_DIR", sdkman_dir.path().as_os_str());

    let contains_error = predicate::str::contains(invalid_name);

    Command::new(assert_cmd::cargo::cargo_bin!("list"))
        .arg(invalid_name)
        .assert()
        .failure()
        .stderr(contains_error)
        .code(1);

    Ok(())
}

#[test]
#[serial]
fn should_error_for_candidate_not_installed() -> Result<(), Box<dyn std::error::Error>> {
    let candidate_name = "kotlin";

    // Create environment with kotlin in candidates file but not installed
    let sdkman_dir = support::prepare_sdkman_dir();
    support::write_file(
        sdkman_dir.path(),
        Path::new("var"),
        "candidates",
        candidate_name.to_string(),
    );

    env::set_var("SDKMAN_DIR", sdkman_dir.path().as_os_str());
    env::set_var("SDKMAN_OFFLINE_MODE", "true");

    let contains_error = predicate::str::contains("is not installed");

    Command::new(assert_cmd::cargo::cargo_bin!("list"))
        .arg(candidate_name)
        .assert()
        .failure()
        .stderr(contains_error)
        .code(1);

    env::remove_var("SDKMAN_OFFLINE_MODE");

    Ok(())
}

#[test]
#[serial]
fn should_handle_candidate_with_no_versions() -> Result<(), Box<dyn std::error::Error>> {
    let candidate_name = "kotlin";

    let sdkman_dir = support::prepare_sdkman_dir();
    support::write_file(
        sdkman_dir.path(),
        Path::new("var"),
        "candidates",
        candidate_name.to_string(),
    );

    // Create candidate directory but no version subdirectories
    let candidate_dir = Path::new("candidates").join(candidate_name);
    std::fs::create_dir_all(sdkman_dir.path().join(&candidate_dir))
        .expect("Failed to create candidate directory");

    env::set_var("SDKMAN_DIR", sdkman_dir.path().as_os_str());
    env::set_var("SDKMAN_OFFLINE_MODE", "true");

    let contains_message = predicate::str::contains("None installed!");
    let contains_offline = predicate::str::contains("Offline:");

    Command::new(assert_cmd::cargo::cargo_bin!("list"))
        .arg(candidate_name)
        .assert()
        .stdout(contains_message.and(contains_offline))
        .code(0);

    env::remove_var("SDKMAN_OFFLINE_MODE");

    Ok(())
}

#[test]
#[serial]
fn should_list_installed_versions_with_installed_subcommand(
) -> Result<(), Box<dyn std::error::Error>> {
    let name = "java";
    let versions = vec!["11.0.15-tem", "17.0.3-tem", "21.0.0-tem"];
    let current_version = "17.0.3-tem";

    let env = VirtualEnv {
        cli_version: "5.0.0".to_string(),
        native_version: "0.1.0".to_string(),
        candidates: vec![TestCandidate {
            name,
            versions: versions.clone(),
            current_version,
        }],
    };

    let sdkman_dir = support::virtual_env(env);
    env::set_var("SDKMAN_DIR", sdkman_dir.path().as_os_str());

    let contains_header =
        predicate::str::contains(format!("Offline: only showing installed {} versions", name));
    let contains_version1 = predicate::str::contains("11.0.15-tem");
    let contains_version2 = predicate::str::contains("17.0.3-tem");
    let contains_version3 = predicate::str::contains("21.0.0-tem");

    Command::new(assert_cmd::cargo::cargo_bin!("list"))
        .arg(name)
        .arg("installed")
        .assert()
        .success()
        .stdout(
            contains_header
                .and(contains_version1)
                .and(contains_version2)
                .and(contains_version3),
        )
        .code(0);

    Ok(())
}

#[test]
#[serial]
fn should_error_for_invalid_subcommand() -> Result<(), Box<dyn std::error::Error>> {
    let name = "java";
    let invalid_subcommand = "invalid";

    let env = VirtualEnv {
        cli_version: "5.0.0".to_string(),
        native_version: "0.1.0".to_string(),
        candidates: vec![TestCandidate {
            name,
            versions: vec!["11.0.15-tem"],
            current_version: "11.0.15-tem",
        }],
    };

    let sdkman_dir = support::virtual_env(env);
    env::set_var("SDKMAN_DIR", sdkman_dir.path().as_os_str());

    let contains_error = predicate::str::contains("Unknown subcommand");
    let contains_invalid = predicate::str::contains(invalid_subcommand);

    Command::new(assert_cmd::cargo::cargo_bin!("list"))
        .arg(name)
        .arg(invalid_subcommand)
        .assert()
        .failure()
        .stderr(contains_error.and(contains_invalid))
        .code(1);

    Ok(())
}

#[test]
#[serial]
fn should_respect_offline_mode_env_var() -> Result<(), Box<dyn std::error::Error>> {
    let name = "java";
    let versions = vec!["11.0.15-tem"];
    let current_version = "11.0.15-tem";

    let env = VirtualEnv {
        cli_version: "5.0.0".to_string(),
        native_version: "0.1.0".to_string(),
        candidates: vec![TestCandidate {
            name,
            versions: versions.clone(),
            current_version,
        }],
    };

    let sdkman_dir = support::virtual_env(env);
    env::set_var("SDKMAN_DIR", sdkman_dir.path().as_os_str());
    env::set_var("SDKMAN_OFFLINE_MODE", "true");

    let contains_offline = predicate::str::contains("Offline: only showing installed");

    Command::new(assert_cmd::cargo::cargo_bin!("list"))
        .arg(name)
        .assert()
        .success()
        .stdout(contains_offline)
        .code(0);

    // Clean up
    env::remove_var("SDKMAN_OFFLINE_MODE");

    Ok(())
}
