use std::path::{Path, PathBuf};

use clap::{Args, ColorChoice, Parser, Subcommand};

use crate::commands;
use crate::commands::deps::Dependency;
#[cfg(target_os = "macos")]
use crate::commands::ios::{
    BuildConfiguration as IosBuildConfiguration, DeviceKind as IosDeviceKind,
};

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
    /// Install this checkout's Photon CLI in Cargo's bin directory.
    Install,
    /// Install or remove Linux desktop integration.
    #[cfg(target_os = "linux")]
    Desktop {
        #[command(subcommand)]
        command: DesktopCommand,
    },
    /// Report tool versions, installed Rust tools, and dependency checkout state.
    Doctor,
    /// Report Rust editor metadata.
    #[command(hide = true)]
    Ide {
        #[command(subcommand)]
        command: IdeCommand,
    },
    /// Build Photon Engine and the GPUI-CE application.
    Build {
        #[command(flatten)]
        profile: BuildProfile,
    },
    /// Build and launch Photon.
    Run {
        #[command(flatten)]
        profile: BuildProfile,
        /// Build and launch the shell without building, linking, or starting Photon Engine.
        #[arg(long)]
        ui: bool,
        /// Navigate the PhotonWebView to this URL after launch.
        #[arg(long, conflicts_with = "url_argument")]
        url: Option<String>,
        /// URL to open. Used by the Linux desktop entry for web links.
        #[arg(value_name = "URL", conflicts_with = "url")]
        url_argument: Option<String>,
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
    /// Build, launch, and inspect the native iOS app.
    #[cfg(target_os = "macos")]
    Ios {
        #[command(subcommand)]
        command: IosCommand,
    },
}

#[cfg(target_os = "macos")]
#[derive(Debug, Subcommand)]
enum IosCommand {
    /// List installed iPhone simulator models and devices known to Xcode.
    Devices {
        /// Choose which iPhone inventory to display.
        #[arg(long, value_enum, default_value = "all")]
        kind: IosDeviceKind,
        /// Show paired physical iPhones only.
        #[arg(long)]
        paired: bool,
        /// Show booted simulators and connected physical iPhones only.
        #[arg(long)]
        connected: bool,
    },
    /// Build the iOS app without launching it.
    Build {
        #[command(flatten)]
        target: IosTargetArgs,
        #[command(flatten)]
        build: IosBuildArgs,
    },
    /// Install and launch a previously built iOS app.
    Launch {
        #[command(flatten)]
        target: IosTargetArgs,
        /// Xcode build configuration to launch.
        #[arg(long, value_enum, default_value = "debug")]
        configuration: IosBuildConfiguration,
    },
    /// Build, install, and launch the iOS app.
    Run {
        #[command(flatten)]
        target: IosTargetArgs,
        #[command(flatten)]
        build: IosBuildArgs,
    },
}

#[cfg(target_os = "macos")]
#[derive(Debug, Args)]
struct IosTargetArgs {
    /// Select an existing simulator or a paired physical iPhone by name or UDID.
    #[arg(long, value_name = "NAME_OR_UDID", conflicts_with = "model")]
    device: Option<String>,
    /// Select an iPhone simulator model. Creates a simulator if one does not exist.
    #[arg(
        long,
        visible_alias = "iphone",
        value_name = "MODEL",
        conflicts_with = "device"
    )]
    model: Option<String>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Args)]
struct IosBuildArgs {
    /// Xcode build configuration.
    #[arg(long, value_enum, default_value = "debug")]
    configuration: IosBuildConfiguration,
    /// Apple Developer Team ID for signing a physical-device build.
    #[arg(long, value_name = "TEAM_ID")]
    team_id: Option<String>,
    /// Allow Xcode to update signing profiles from the Apple Developer account.
    #[arg(long)]
    allow_provisioning_updates: bool,
    /// Allow Xcode to register the target iPhone with the Apple Developer account.
    #[arg(long, requires = "allow_provisioning_updates")]
    allow_device_registration: bool,
}

#[derive(Debug, Subcommand)]
enum IdeCommand {
    /// Prepare the debug build metadata used by editor language servers.
    Setup,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Subcommand)]
enum DesktopCommand {
    /// Install Photon’s launcher, icon, and web URL handlers.
    Install,
    /// Remove Photon’s launcher and icon without uninstalling the CLI.
    Uninstall,
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
        #[command(flatten)]
        profile: BuildProfile,
    },
    /// Fetch and merge upstream/master into the Engine checkout.
    #[command(hide = true)]
    Sync {
        /// Fetch and preview commit counts and merge conflicts without merging.
        #[arg(long)]
        check: bool,
    },
}

#[derive(Debug, Args)]
struct BuildProfile {
    /// Build without optimizations for faster iteration and debugging.
    #[arg(long, conflicts_with = "release_compat")]
    debug: bool,
    /// Build with optimizations (the default).
    #[arg(
        long = "release",
        hide = true,
        id = "release_compat",
        conflicts_with = "debug"
    )]
    release_compat: bool,
}

impl BuildProfile {
    fn release(&self) -> bool {
        !self.debug || self.release_compat
    }
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
        #[cfg(target_os = "linux")]
        Some(Command::Desktop { command }) => match command {
            DesktopCommand::Install => commands::linux_desktop::install(&root),
            DesktopCommand::Uninstall => commands::linux_desktop::uninstall(),
        },
        Some(Command::Doctor) => commands::setup::doctor(&root),
        Some(Command::Ide { command }) => match command {
            IdeCommand::Setup => commands::ide::setup(&root, verbose),
        },
        Some(Command::Build { profile }) => {
            commands::build::build(&root, profile.release(), verbose)
        }
        Some(Command::Run {
            profile,
            ui,
            url,
            url_argument,
            shutdown_after_seconds,
        }) => {
            commands::build::warn_if_dependencies_are_behind(&root);
            let url = url.or(url_argument);
            if ui && url.is_some() {
                return Err("a URL cannot be used with --ui".into());
            }
            if ui {
                commands::build::run_ui(&root, profile.release(), verbose, shutdown_after_seconds)
            } else {
                commands::build::run_direct(
                    &root,
                    profile.release(),
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
            Some(EngineCommand::Build { profile }) => {
                commands::engine::engine(&root, Some("build"), profile.release(), verbose)
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
        #[cfg(target_os = "macos")]
        Some(Command::Ios { command }) => match command {
            IosCommand::Devices {
                kind,
                paired,
                connected,
            } => commands::ios::list_devices(&root, kind, paired, connected),
            IosCommand::Build { target, build } => commands::ios::build(
                &root,
                target.into_selection(),
                build.configuration,
                build.team_id.as_deref(),
                build.allow_provisioning_updates,
                build.allow_device_registration,
                verbose,
            ),
            IosCommand::Launch {
                target,
                configuration,
            } => commands::ios::launch(&root, target.into_selection(), configuration, verbose),
            IosCommand::Run { target, build } => commands::ios::run(
                &root,
                target.into_selection(),
                build.configuration,
                build.team_id.as_deref(),
                build.allow_provisioning_updates,
                build.allow_device_registration,
                verbose,
            ),
        },
    }
}

#[cfg(target_os = "macos")]
impl IosTargetArgs {
    fn into_selection(self) -> commands::ios::DeviceSelection {
        commands::ios::DeviceSelection {
            device: self.device,
            model: self.model,
        }
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
    use super::{Cli, Command, Dependency, DepsCommand};
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
        let Some(Command::Run { profile, .. }) = cli.command else {
            panic!("expected run command");
        };

        assert!(profile.release());
    }

    #[test]
    fn run_can_select_a_debug_build() {
        let cli = Cli::try_parse_from(["photon", "run", "--debug"]).unwrap();
        let Some(Command::Run { profile, .. }) = cli.command else {
            panic!("expected run command");
        };

        assert!(!profile.release());
    }

    #[test]
    fn release_flag_remains_supported_for_existing_scripts() {
        let cli = Cli::try_parse_from(["photon", "build", "--release"]).unwrap();
        let Some(Command::Build { profile }) = cli.command else {
            panic!("expected build command");
        };

        assert!(profile.release());
    }
}
