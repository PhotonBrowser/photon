use std::path::{Path, PathBuf};

use clap::{Args, ColorChoice, Parser, Subcommand};

use crate::commands;
use crate::commands::deps::Dependency;

#[derive(Debug, Parser)]
#[command(
    name = "photon",
    bin_name = "photon",
    version,
    about = "Build and maintain the Photon browser",
    long_about = None,
    color = ColorChoice::Auto,
    styles = help_styles(),
    subcommand_required = false,
    arg_required_else_help = false,
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    options: GlobalOptions,
}

#[derive(Debug, Args)]
struct GlobalOptions {
    /// Show the output of the underlying build tools.
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Verify prerequisites, initialize dependencies, prepare tools, and install the CLI.
    Setup {
        /// Install system build packages before setup where supported.
        #[arg(long)]
        install_system_deps: bool,
        /// Prepare the Rust and GPUI shell without initializing Photon Engine.
        #[arg(long)]
        ui_only: bool,
    },
    /// Install this checkout's Photon command in Cargo's bin directory.
    Install,
    /// Report tool versions, installed Rust tools, and dependency checkout state.
    Doctor,
    /// Report Rust editor metadata.
    Ide {
        #[command(subcommand)]
        command: IdeCommand,
    },
    /// Build Photon Engine and the GPUI-CE application.
    Build {
        /// Build with optimizations (the default).
        #[arg(long, default_value_t = true)]
        release: bool,
    },
    /// Build and launch Photon.
    Run {
        /// Build with optimizations (the default).
        #[arg(long, default_value_t = true)]
        release: bool,
        /// Build and launch the shell without building, linking, or starting Photon Engine.
        #[arg(long)]
        ui: bool,
        /// Hot-patch GPUI rendering code via Dioxus CLI (development build).
        #[arg(long, conflicts_with = "no_hot_reload")]
        hot_reload: bool,
        /// Use the direct launch path instead of GPUI hot reloading.
        #[arg(long, conflicts_with = "hot_reload")]
        no_hot_reload: bool,
        /// Navigate the PhotonWebView to this URL after launch.
        #[arg(long)]
        url: Option<String>,
        /// Quit after this many seconds (for shutdown and lease-drain verification).
        #[arg(long)]
        shutdown_after_seconds: Option<u64>,
    },
    /// Remove generated build directories.
    Clean {
        /// Remove only Engine build directories.
        #[arg(value_enum)]
        scope: Option<CleanScope>,
    },
    /// Check formatting, lints, compilation, and architecture rules.
    Check,
    /// Run Rust lints and native formatting checks.
    Lint,
    /// Print a copyable AI prompt for fixing a check failure.
    FixPrompt,
    /// Format Photon-owned Rust and C++ files.
    Format,
    /// Verify and commit the current dependency branch tips as Photon pins.
    #[command(hide = true)]
    Pin,
    /// Merge upstream changes and dispatch a review when dependency branches advance.
    #[command(hide = true)]
    Sync {
        /// Fetch and preview commit counts and merge conflicts without merging.
        #[arg(long)]
        check: bool,
        /// Skip GitHub review issue dispatch after merging all dependencies.
        #[arg(long)]
        no_review: bool,
    },
    /// Inspect and maintain all Photon dependency checkouts.
    Deps {
        #[command(subcommand)]
        command: DepsCommand,
    },
    /// Maintain the vendored GPUI-CE checkout.
    #[command(hide = true)]
    Gpui {
        #[command(subcommand)]
        command: GpuiCommand,
    },
    /// Run Rust workspace tests.
    Test,
    /// Inspect or build Photon Engine.
    Engine {
        #[command(subcommand)]
        command: Option<EngineCommand>,
    },
}

#[derive(Debug, Subcommand)]
enum IdeCommand {
    /// Prepare the debug build metadata used by editor language servers.
    Setup,
}

#[derive(Debug, Subcommand)]
enum GpuiCommand {
    /// Switch GPUI-CE to its persistent Photon development branch.
    Edit,
    /// Return GPUI-CE to the revision pinned by this Photon checkout.
    Pin,
    /// Merge the configured upstream GPUI-CE branch into the Photon branch.
    Sync {
        /// Fetch and preview commit counts and merge conflicts without merging.
        #[arg(long)]
        check: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DepsCommand {
    /// Show branch, pin, worktree, and upstream status for every dependency.
    Status,
    /// Switch clean dependency checkouts to their persistent Photon branches.
    Edit {
        #[arg(value_enum)]
        dependency: Option<Dependency>,
    },
    /// Restore dependency checkouts to the commits pinned by this Photon root.
    Restore {
        #[arg(value_enum)]
        dependency: Option<Dependency>,
    },
    /// Validate and commit the current dependency branch tips as Photon pins.
    Pin,
    /// Fetch and merge upstream changes for one dependency or all of them.
    Sync {
        #[arg(value_enum)]
        dependency: Option<Dependency>,
        /// Preview commit counts and conflicts without switching or merging.
        #[arg(long)]
        check: bool,
        /// Dispatch a GitHub review issue after syncing all dependencies.
        #[arg(long)]
        review: bool,
    },
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum CleanScope {
    Engine,
}

#[derive(Debug, Subcommand)]
enum EngineCommand {
    /// Show repository state and upstream commit counts.
    Status,
    /// Switch Engine to Photon’s persistent master development branch.
    #[command(hide = true)]
    Edit,
    /// Return Engine to the commit pinned by this Photon checkout.
    #[command(hide = true)]
    Pin,
    /// Build Photon Engine libraries and services.
    Build {
        /// Build with optimizations (the default).
        #[arg(long, default_value_t = true)]
        release: bool,
    },
    /// Fetch and merge upstream/master into the Engine checkout.
    #[command(hide = true)]
    Sync {
        /// Fetch and preview commit counts and merge conflicts without merging.
        #[arg(long)]
        check: bool,
    },
}

pub fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let root = workspace_root()?;
    let verbose = cli.options.verbose;

    match cli.command {
        None => print_help(),
        Some(Command::Setup {
            install_system_deps,
            ui_only,
        }) => commands::setup::setup(&root, verbose, install_system_deps, ui_only),
        Some(Command::Install) => commands::install::install(&root, verbose),
        Some(Command::Doctor) => commands::setup::doctor(&root),
        Some(Command::Ide { command }) => match command {
            IdeCommand::Setup => commands::ide::setup(&root, verbose),
        },
        Some(Command::Build { release }) => commands::build::build(&root, release, verbose),
        Some(Command::Run {
            release,
            ui,
            hot_reload,
            no_hot_reload,
            url,
            shutdown_after_seconds,
        }) => {
            commands::build::warn_if_dependencies_are_behind(&root);
            if ui && url.is_some() {
                return Err("--url cannot be used with --ui".into());
            }
            if should_use_hot_reload(hot_reload, no_hot_reload) {
                return commands::build::run_hot_reload(
                    &root,
                    release,
                    ui,
                    verbose,
                    url.as_deref(),
                    shutdown_after_seconds,
                );
            }
            if ui {
                commands::build::run_ui(&root, release, verbose, shutdown_after_seconds)
            } else {
                commands::build::run_direct(
                    &root,
                    release,
                    verbose,
                    url.as_deref(),
                    shutdown_after_seconds,
                )
            }
        }
        Some(Command::Clean { scope }) => {
            let scope = scope.map(|scope| match scope {
                CleanScope::Engine => "engine",
            });
            commands::build::clean(&root, scope)
        }
        Some(Command::Check) => commands::check::check(&root, verbose),
        Some(Command::Lint) => commands::lint::lint(&root, verbose),
        Some(Command::FixPrompt) => {
            commands::check::print_fix_prompt(
                &root,
                "Paste the failing command and its output here.",
            );
            Ok(())
        }
        Some(Command::Format) => commands::format::format(&root, verbose),
        Some(Command::Pin) => commands::pin::pin(&root, verbose),
        Some(Command::Sync { check, no_review }) => {
            commands::deps::sync(&root, None, check, no_review, verbose)
        }
        Some(Command::Deps { command }) => match command {
            DepsCommand::Status => commands::deps::status(&root),
            DepsCommand::Edit { dependency } => commands::deps::edit(&root, dependency, verbose),
            DepsCommand::Restore { dependency } => {
                commands::deps::restore(&root, dependency, verbose)
            }
            DepsCommand::Pin => commands::pin::pin(&root, verbose),
            DepsCommand::Sync {
                dependency,
                check,
                review,
            } => {
                if review && (check || dependency.is_some()) {
                    return Err(
                        "`--review` applies only to a completed sync of all dependencies".into(),
                    );
                }
                commands::deps::sync(&root, dependency, check, !review, verbose)
            }
        },
        Some(Command::Gpui { command }) => match command {
            GpuiCommand::Edit => commands::gpui::edit(&root, verbose),
            GpuiCommand::Pin => commands::gpui::pin(&root, verbose),
            GpuiCommand::Sync { check } => {
                if check {
                    commands::gpui::check_upstream(&root, verbose)
                } else {
                    commands::gpui::sync(&root, verbose)
                }
            }
        },
        Some(Command::Test) => commands::tests::run(&root, verbose),
        Some(Command::Engine { command }) => match command {
            None | Some(EngineCommand::Status) => {
                commands::engine::engine(&root, Some("status"), false, verbose)
            }
            Some(EngineCommand::Build { release }) => {
                commands::engine::engine(&root, Some("build"), release, verbose)
            }
            Some(EngineCommand::Edit) => commands::engine::edit(&root, verbose),
            Some(EngineCommand::Pin) => commands::engine::pin(&root, verbose),
            Some(EngineCommand::Sync { check }) => {
                if check {
                    commands::engine::check_upstream(&root, verbose)
                } else {
                    commands::engine::engine(&root, Some("sync"), false, verbose)
                }
            }
        },
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    if let Ok(current_dir) = std::env::current_dir()
        && let Some(root) = current_dir
            .ancestors()
            .find(|path| path.join("photon.toml").is_file() && path.join(".git").exists())
    {
        return Ok(root.to_path_buf());
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot locate Photon workspace".into())
}

fn should_use_hot_reload(explicit: bool, disabled: bool) -> bool {
    explicit && !disabled
}

fn print_help() -> Result<(), String> {
    use clap::CommandFactory;
    Cli::command()
        .print_help()
        .map_err(|error| format!("cannot print help: {error}"))?;
    println!();
    Ok(())
}

fn help_styles() -> clap::builder::Styles {
    use clap::builder::styling::{AnsiColor, Styles};

    Styles::styled()
        .header(AnsiColor::BrightCyan.on_default().bold())
        .usage(AnsiColor::Green.on_default().bold())
        .literal(AnsiColor::BrightCyan.on_default())
        .placeholder(AnsiColor::Yellow.on_default())
        .error(AnsiColor::BrightRed.on_default().bold())
        .valid(AnsiColor::Green.on_default())
        .invalid(AnsiColor::BrightRed.on_default())
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, Dependency, DepsCommand, should_use_hot_reload};
    use clap::Parser;

    #[test]
    fn dependency_sync_defaults_to_local_all_dependency_sync() {
        let cli = Cli::try_parse_from(["photon", "deps", "sync"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Deps {
                command: DepsCommand::Sync {
                    dependency: None,
                    check: false,
                    review: false,
                }
            })
        ));
    }

    #[test]
    fn dependency_sync_accepts_one_dependency_and_preview() {
        let cli = Cli::try_parse_from(["photon", "deps", "sync", "gpui", "--check"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Deps {
                command: DepsCommand::Sync {
                    dependency: Some(Dependency::Gpui),
                    check: true,
                    review: false,
                }
            })
        ));
    }

    #[test]
    fn dependency_sync_can_request_review_dispatch() {
        let cli = Cli::try_parse_from(["photon", "deps", "sync", "--review"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Deps {
                command: DepsCommand::Sync {
                    dependency: None,
                    check: false,
                    review: true,
                }
            })
        ));
    }

    #[test]
    fn setup_can_opt_in_to_macos_system_packages() {
        let cli = Cli::try_parse_from(["photon", "setup", "--install-system-deps"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Setup {
                install_system_deps: true,
                ui_only: false,
            })
        ));
    }

    #[test]
    fn setup_can_skip_engine_prerequisites_for_the_ui_shell() {
        let cli = Cli::try_parse_from(["photon", "setup", "--ui-only"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Setup {
                install_system_deps: false,
                ui_only: true,
            })
        ));
    }

    #[test]
    fn run_uses_optimized_direct_build_by_default() {
        let cli = Cli::try_parse_from(["photon", "run"]).unwrap();
        let Some(Command::Run {
            release,
            hot_reload,
            no_hot_reload,
            ..
        }) = cli.command
        else {
            panic!("expected run command");
        };

        assert!(release);
        assert!(!should_use_hot_reload(hot_reload, no_hot_reload));
    }

    #[test]
    fn run_can_explicitly_enable_or_disable_hot_reload() {
        let enabled = Cli::try_parse_from(["photon", "run", "--hot-reload"]).unwrap();
        let disabled = Cli::try_parse_from(["photon", "run", "--no-hot-reload"]).unwrap();
        let Some(Command::Run {
            hot_reload,
            no_hot_reload,
            ..
        }) = enabled.command
        else {
            panic!("expected run command");
        };
        assert!(should_use_hot_reload(hot_reload, no_hot_reload));

        let Some(Command::Run {
            hot_reload,
            no_hot_reload,
            ..
        }) = disabled.command
        else {
            panic!("expected run command");
        };
        assert!(!should_use_hot_reload(hot_reload, no_hot_reload));
    }
}
