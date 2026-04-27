use std::fs;
use std::path::PathBuf;
use std::process;

use clap::Parser;
use colored::Colorize;

use sdkman_cli_native::constants::{CANDIDATES_DIR, CURRENT_DIR, SDKMAN_CANDIDATES_API};
use sdkman_cli_native::helpers::{
    get_platform, infer_sdkman_dir, is_offline, known_candidates, validate_candidate,
};

#[derive(Parser, Debug)]
#[command(
    bin_name = "sdk list",
    about = "sdk subcommand to list all candidates or candidate versions",
    after_help = "Examples:
  sdk list                      List all available candidates
  sdk list java                 List all versions for java (online if available)
  sdk list java installed       List only installed versions for java"
)]
struct Args {
    #[arg(help = "The candidate to list versions for")]
    candidate: Option<String>,

    #[arg(help = "Subcommand: 'installed' to show only installed versions")]
    subcommand: Option<String>,
}

fn main() {
    let args = Args::parse();
    let sdkman_dir = infer_sdkman_dir();
    let all_candidates = known_candidates(sdkman_dir.clone());

    match args.candidate {
        Some(candidate) => {
            let candidate = validate_candidate(all_candidates, &candidate);

            match args.subcommand.as_deref() {
                Some("installed") => {
                    let candidate_dir = sdkman_dir.join(CANDIDATES_DIR).join(&candidate);
                    let current_version = get_current_version(&candidate_dir);
                    list_installed_versions(&candidate_dir, &candidate, current_version);
                }
                Some(unknown) => {
                    eprintln!(
                        "Unknown subcommand: '{}'. Valid subcommands: {}",
                        unknown.bold(),
                        "installed".italic()
                    );
                    process::exit(1);
                }
                None => {
                    list_candidate_versions(sdkman_dir, &candidate);
                }
            }
        }
        None => {
            list_all_candidates(all_candidates);
        }
    }
}

fn list_all_candidates(candidates: Vec<&str>) {
    if !is_offline() {
        match fetch_all_candidates() {
            Ok(response) => {
                println!("{}", response);
                return;
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    "Unable to fetch online candidates, showing local cache only.".yellow()
                );
                if std::env::var("SDKMAN_DEBUG").is_ok() {
                    eprintln!("{}: {}", "Debug".dimmed(), e);
                }
            }
        }
    }

    println!(
        "{}",
        "--------------------------------------------------------------------------------".normal()
    );
    println!("{}", "Available Candidates (local cache)".yellow());
    println!(
        "{}",
        "--------------------------------------------------------------------------------".normal()
    );
    println!();

    for candidate in candidates {
        println!("   {}", candidate);
    }

    println!();
    println!(
        "{}",
        "--------------------------------------------------------------------------------".normal()
    );
    println!(
        "Use {} to see versions for a specific candidate",
        "sdk list <candidate>".italic()
    );
    println!(
        "{}",
        "Note: For full candidate details with descriptions, use the online version".dimmed()
    );
    println!(
        "{}",
        "--------------------------------------------------------------------------------".normal()
    );
}

fn list_candidate_versions(sdkman_dir: PathBuf, candidate: &str) {
    let candidate_dir = sdkman_dir.join(CANDIDATES_DIR).join(candidate);

    let current_version = get_current_version(&candidate_dir);
    let installed_csv = build_versions_csv(&candidate_dir);

    if !is_offline() {
        let current = current_version.as_deref().unwrap_or("");
        match fetch_online_versions(candidate, current, &installed_csv) {
            Ok(response) => {
                println!("{}", response);
                return;
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    "Unable to fetch online versions, showing installed only.".yellow()
                );
                if std::env::var("SDKMAN_DEBUG").is_ok() {
                    eprintln!("{}: {}", "Debug".dimmed(), e);
                }
            }
        }
    }

    list_installed_versions(&candidate_dir, candidate, current_version);
}

fn list_installed_versions(
    candidate_dir: &std::path::Path,
    candidate: &str,
    current_version: Option<String>,
) {
    if !candidate_dir.exists() || !candidate_dir.is_dir() {
        eprintln!("{} is not installed.", candidate.bold());
        process::exit(1);
    }

    let mut versions: Vec<String> = match get_installed_versions(candidate_dir) {
        Ok(versions) => versions,
        Err(_) => {
            eprintln!("Failed to read {} versions.", candidate.bold());
            process::exit(1);
        }
    };

    println!(
        "{}",
        "--------------------------------------------------------------------------------".normal()
    );
    println!(
        "{}",
        format!("Offline: only showing installed {} versions", candidate).yellow()
    );
    println!(
        "{}",
        "--------------------------------------------------------------------------------".normal()
    );

    if versions.is_empty() {
        println!("{}", "   None installed!".yellow());
    } else {
        versions.sort();
        versions.reverse();

        for version in versions {
            if current_version.as_ref() == Some(&version) {
                println!(" {} {}", ">".normal(), version);
            } else {
                println!(" {} {}", "*".normal(), version);
            }
        }
    }

    println!(
        "{}",
        "--------------------------------------------------------------------------------".normal()
    );
    println!(
        "{}",
        "* - installed                                                                   ".normal()
    );
    println!(
        "{}",
        "> - currently in use                                                            ".normal()
    );
    println!(
        "{}",
        "--------------------------------------------------------------------------------".normal()
    );
}

fn get_current_version(candidate_dir: &std::path::Path) -> Option<String> {
    let current_link = candidate_dir.join(CURRENT_DIR);

    if !current_link.exists() {
        return None;
    }

    if let Ok(target) = fs::read_link(&current_link) {
        // Extract the version from the path
        return target
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string());
    }

    if current_link.is_dir() {
        return current_link
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string());
    }

    None
}

fn get_installed_versions(candidate_dir: &std::path::Path) -> Result<Vec<String>, std::io::Error> {
    let versions: Vec<String> = fs::read_dir(candidate_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let name = entry.file_name().to_str()?.to_string();

            if name == CURRENT_DIR {
                return None;
            }

            path.is_dir().then_some(name)
        })
        .collect();

    Ok(versions)
}

fn build_versions_csv(candidate_dir: &std::path::Path) -> String {
    get_installed_versions(candidate_dir)
        .map(|versions| versions.join(","))
        .unwrap_or_default()
}

fn create_http_client() -> Result<reqwest::blocking::Client, String> {
    use std::time::Duration;

    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("sdkman-cli-native/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

fn handle_http_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "Request timed out after 10 seconds".to_string()
    } else if e.is_connect() {
        "Failed to connect to API - check your internet connection".to_string()
    } else {
        format!("Network error: {}", e)
    }
}

fn fetch_all_candidates() -> Result<String, String> {
    let client = create_http_client()?;
    let url = format!("{}/candidates/list", SDKMAN_CANDIDATES_API);

    let response = client.get(&url).send().map_err(handle_http_error)?;

    if response.status().is_success() {
        response
            .text()
            .map_err(|e| format!("Failed to read response: {}", e))
    } else {
        Err(format!(
            "API request failed with status: {}",
            response.status()
        ))
    }
}

fn fetch_online_versions(
    candidate: &str,
    current: &str,
    installed_csv: &str,
) -> Result<String, String> {
    let client = create_http_client()?;
    let platform = get_platform();

    let url = format!(
        "{}/candidates/{}/{}/versions/list",
        SDKMAN_CANDIDATES_API, candidate, platform
    );

    let response = client
        .get(&url)
        .query(&[("current", current), ("installed", installed_csv)])
        .send()
        .map_err(handle_http_error)?;

    if response.status().is_success() {
        response
            .text()
            .map_err(|e| format!("Failed to read response: {}", e))
    } else {
        Err(format!(
            "API request failed with status: {}",
            response.status()
        ))
    }
}
