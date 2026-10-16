use std::{env, process::ExitCode};

use velocity_browser::{BrowserInstallation, BrowserRegistry};
use velocity_core::FRAMEWORK_NAME;

mod browser_selection;
mod comparison;
mod comparison_cli;
mod comparison_statistics;
mod fixture_server;
mod history;
mod history_cli;
mod jetstream_cli;
mod jetstream_source;
mod monitor_cli;
mod resource_benchmark_cli;
#[cfg(target_os = "macos")]
mod resource_report;
mod speedometer_cli;
mod speedometer_source;
mod startup_cli;
mod terminal;

#[cfg(target_os = "macos")]
mod macos_startup;

#[cfg(target_os = "macos")]
use velocity_browser::{
    BrowserLifecycleManager, CloseOutcome, FileSessionStore, LifecycleError,
    macos::{MacOSBrowserDiscovery, MacOSProcessDriver},
};

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}\nTry 'velocity --help' for usage.");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut arguments = arguments.into_iter();
    let Some(command) = arguments.next() else {
        print_help();
        return Ok(());
    };

    match command.as_str() {
        "--help" | "-h" | "help" => {
            require_no_more(arguments)?;
            print_help();
        }
        "--version" | "-V" => {
            require_no_more(arguments)?;
            println!("{FRAMEWORK_NAME} {}", env!("CARGO_PKG_VERSION"));
        }
        "list" => {
            require_no_more(arguments)?;
            list_browsers()?;
        }
        "inspect" => {
            let browser = arguments.next();
            require_no_more(arguments)?;
            inspect_browser(browser.as_deref())?;
        }
        "launch" => {
            let mut browser = None;
            let mut url = None;
            while let Some(argument) = arguments.next() {
                match argument.as_str() {
                    "--url" if url.is_none() => {
                        url = Some(required_argument(&mut arguments, "URL")?);
                    }
                    value if !value.starts_with('-') && browser.is_none() => {
                        browser = Some(value.to_owned());
                    }
                    other => return Err(format!("unexpected argument '{other}'")),
                }
            }
            launch_browser(browser.as_deref(), url.as_deref())?;
        }
        "close" => {
            let browser = required_argument(&mut arguments, "browser")?;
            require_no_more(arguments)?;
            close_browser(&browser)?;
        }
        "bench" => {
            let arguments = arguments.collect::<Vec<_>>();
            match arguments.first().map(String::as_str) {
                Some("startup") | None | Some("--help") | Some("-h") | Some("help") => {
                    startup_cli::run(arguments)?;
                }
                Some("idle" | "page" | "tabs") => resource_benchmark_cli::run(arguments)?,
                Some("speedometer") => speedometer_cli::run(arguments.into_iter().skip(1))?,
                Some("jetstream") => jetstream_cli::run(arguments.into_iter().skip(1))?,
                Some(other) => {
                    return Err(format!(
                        "unknown benchmark '{other}'; available benchmarks: startup, idle, page, tabs, speedometer, jetstream"
                    ));
                }
            }
        }
        "compare" => {
            let arguments = arguments.collect::<Vec<_>>();
            if arguments.first().map(String::as_str) == Some("speedometer") {
                speedometer_cli::run_compare(arguments.into_iter().skip(1))?;
            } else if arguments.first().map(String::as_str) == Some("jetstream") {
                jetstream_cli::run_compare(arguments.into_iter().skip(1))?;
            } else {
                comparison_cli::run(arguments)?;
            }
        }
        "history" => history_cli::run_history(arguments.collect::<Vec<_>>())?,
        "baseline" => history_cli::run_baseline(arguments.collect::<Vec<_>>())?,
        "setup" => {
            let benchmark = required_argument(&mut arguments, "benchmark")?;
            require_no_more(arguments)?;
            let path = match benchmark.as_str() {
                "speedometer" => speedometer_source::setup_speedometer_source()?,
                "jetstream" => jetstream_source::setup_jetstream_source()?,
                _ => {
                    return Err(format!(
                        "unknown setup target '{benchmark}'; available targets: speedometer, jetstream"
                    ));
                }
            };
            let name = match benchmark.as_str() {
                "speedometer" => "Speedometer 3.1",
                _ => "JetStream 3.0",
            };
            println!("Prepared official {name} at {}", path.display());
        }
        "monitor" => {
            monitor_cli::run(arguments.collect::<Vec<_>>())?;
        }
        _ => return Err(format!("unknown command '{command}'")),
    }

    Ok(())
}

fn required_argument(
    arguments: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("missing {name} argument"))
}

fn require_no_more(mut arguments: impl Iterator<Item = String>) -> Result<(), String> {
    if let Some(extra) = arguments.next() {
        Err(format!("unexpected argument '{extra}'"))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn discover_browsers() -> Result<BrowserRegistry, String> {
    MacOSBrowserDiscovery
        .discover()
        .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn discover_browsers() -> Result<BrowserRegistry, String> {
    Err("browser discovery currently supports macOS only".to_owned())
}

fn list_browsers() -> Result<(), String> {
    let registry = discover_browsers()?;
    let installations = registry.installations().collect::<Vec<_>>();
    if installations.is_empty() {
        println!("No supported browsers were found.");
        return Ok(());
    }

    let rows = installations
        .iter()
        .enumerate()
        .map(|(index, installation)| {
            let family_count = registry.by_browser(&installation.metadata.id).len();
            let name = if family_count > 1 {
                format!(
                    "{} [{}]",
                    installation.metadata.name, installation.installation_id
                )
            } else {
                installation.metadata.name.clone()
            };
            let version = installation.version.as_deref().unwrap_or("unknown");
            let path = if installation.metadata.bundle_identifier.is_some() {
                installation.application_path.display().to_string()
            } else {
                installation.executable_path.display().to_string()
            };
            vec![(index + 1).to_string(), name, version.to_owned(), path]
        })
        .collect::<Vec<_>>();
    terminal::print_markdown(
        "## Installed browsers\n\nDiscovered browser installations. Use a number (for example, `--browser 1`) or select one interactively.",
    );
    terminal::print_table(&["#", "Browser", "Version", "Path"], rows, &[0]);
    Ok(())
}

#[cfg(target_os = "macos")]
fn inspect_browser(selector: Option<&str>) -> Result<(), String> {
    let registry = discover_browsers()?;
    let installation = browser_selection::choose(&registry, selector, false)?[0];
    print_installation(installation);

    let manager = lifecycle_manager()?;
    let sessions = manager
        .managed_sessions()
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|session| session.installation_id == installation.installation_id.as_str())
        .collect::<Vec<_>>();
    if sessions.is_empty() {
        println!("Velocity-managed instances: none");
    } else {
        println!("Velocity-managed instances:");
        for session in sessions {
            println!("  {} (PID {})", session.session_id, session.identity.pid);
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn inspect_browser(_selector: Option<&str>) -> Result<(), String> {
    Err("browser inspection currently supports macOS only".to_owned())
}

fn print_installation(installation: &BrowserInstallation) {
    terminal::print_markdown(&format!(
        "## {}\n\nBrowser installation metadata.",
        installation.metadata.name
    ));
    terminal::print_table(
        &["Property", "Value"],
        [
            vec![
                "Browser ID".to_owned(),
                installation.metadata.id.to_string(),
            ],
            vec![
                "Installation ID".to_owned(),
                installation.installation_id.to_string(),
            ],
            vec![
                "Version".to_owned(),
                installation
                    .version
                    .as_deref()
                    .unwrap_or("unknown")
                    .to_owned(),
            ],
            vec![
                "Bundle identifier".to_owned(),
                installation
                    .metadata
                    .bundle_identifier
                    .as_deref()
                    .unwrap_or("not available")
                    .to_owned(),
            ],
            vec![
                "Engine".to_owned(),
                installation
                    .metadata
                    .engine
                    .map(engine_name)
                    .unwrap_or("unknown")
                    .to_owned(),
            ],
            vec![
                "Application path".to_owned(),
                installation.application_path.display().to_string(),
            ],
            vec![
                "Executable path".to_owned(),
                installation.executable_path.display().to_string(),
            ],
        ],
        &[],
    );
}

fn engine_name(engine: velocity_browser::BrowserEngine) -> &'static str {
    match engine {
        velocity_browser::BrowserEngine::Chromium => "Chromium",
        velocity_browser::BrowserEngine::Gecko => "Gecko",
        velocity_browser::BrowserEngine::WebKit => "WebKit",
        velocity_browser::BrowserEngine::Ladybird => "Ladybird",
    }
}

#[cfg(target_os = "macos")]
fn lifecycle_manager()
-> Result<BrowserLifecycleManager<MacOSProcessDriver, FileSessionStore>, String> {
    let store_path = velocity_browser::macos::default_session_store_path()?;
    let profile_root = store_path
        .parent()
        .ok_or_else(|| "Velocity session store has no parent directory".to_owned())?
        .join("Profiles");
    Ok(BrowserLifecycleManager::new(
        MacOSProcessDriver::new(profile_root),
        FileSessionStore::new(store_path),
    ))
}

#[cfg(target_os = "macos")]
fn launch_browser(selector: Option<&str>, url: Option<&str>) -> Result<(), String> {
    let registry = discover_browsers()?;
    let installation = browser_selection::choose(&registry, selector, false)?[0];
    let manager = lifecycle_manager()?;
    let session = manager
        .launch(installation, url)
        .map_err(lifecycle_error_message)?;
    println!(
        "Launched {} (PID {}). Close it with 'velocity close {}'.",
        session.browser_name, session.identity.pid, session.session_id
    );
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn launch_browser(_selector: Option<&str>, _url: Option<&str>) -> Result<(), String> {
    Err("browser process lifecycle currently supports macOS only".to_owned())
}

#[cfg(target_os = "macos")]
fn close_browser(selector: &str) -> Result<(), String> {
    let manager = lifecycle_manager()?;
    match manager.close(selector).map_err(lifecycle_error_message)? {
        CloseOutcome::Closed(session) => {
            println!(
                "Closed Velocity-managed {} (PID {}).",
                session.browser_name, session.identity.pid
            );
        }
        CloseOutcome::AlreadyExited(session) => {
            println!(
                "{} had already exited; removed its stale session record.",
                session.browser_name
            );
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn close_browser(_selector: &str) -> Result<(), String> {
    Err("browser process lifecycle currently supports macOS only".to_owned())
}

#[cfg(target_os = "macos")]
fn lifecycle_error_message(error: LifecycleError) -> String {
    error.to_string()
}

fn print_help() {
    terminal::print_markdown(&format!(
        "# {FRAMEWORK_NAME} {}\n\n## Usage\n\n`velocity <command>`\n\n## Commands\n\n- `list` — List browsers with numbered selectors\n- `inspect [browser|number]` — Show browser metadata; prompts if omitted\n- `launch [browser|number] [--url URL]` — Launch a browser; prompts if omitted\n- `close <browser|installation-id|session-id>` — Close a managed instance\n- `bench startup` — Run startup benchmarks and comparisons; prompts if no target is given\n- `bench idle|page|tabs` — Run automated resource benchmarks; prompts if no target is given\n- `bench speedometer` — Run official Speedometer 3.1\n- `bench jetstream` — Run official JetStream 3.0\n- `compare startup` — Check a report against a baseline\n- `compare speedometer` — Compare Speedometer scores against a baseline\n- `compare jetstream` — Compare JetStream scores against a baseline\n- `history import|list|show|report|export` — Store and inspect benchmark history\n- `baseline set|list|show|delete` — Manage named result baselines\n- `setup speedometer` — Prepare the pinned local benchmark source\n- `setup jetstream` — Prepare the pinned local benchmark source\n- `monitor [browser|number] [--duration SECONDS] [--interval MILLISECONDS]` — Diagnose managed browser resource usage; prompts if omitted\n- `help` — Print this help message\n\nUse the number shown by `velocity list` anywhere a browser ID is accepted (for example, `--browser 2`). Interactive benchmark prompts accept comma-separated numbers or `all`.\n\n## Options\n\n- `-h, --help` — Print help\n- `-V, --version` — Print version",
        env!("CARGO_PKG_VERSION")
    ));
}
