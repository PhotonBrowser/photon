use std::path::{Path, PathBuf};

use clap::{Args, ColorChoice, Parser, Subcommand};

use crate::commands;

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
    /// Check prerequisites and initialize configured pinned repositories.
    Setup,
    /// Report toolchain, GPUI-CE, Engine, and build environment details.
    Doctor,
    /// Report Rust editor metadata.
    Ide {
        #[command(subcommand)]
        command: IdeCommand,
    },
    /// Build Photon Engine and the GPUI-CE application.
    Build {
        /// Build with optimizations (the default).
        #[arg(long, conflicts_with = "debug")]
        release: bool,
        /// Build without optimizations.
        #[arg(long, conflicts_with = "release")]
        debug: bool,
    },
    /// Build Photon Engine and launch the direct GPUI-CE application.
    Run {
        /// Build with optimizations (the default).
        #[arg(long, conflicts_with = "debug")]
        release: bool,
        /// Build without optimizations.
        #[arg(long, conflicts_with = "release")]
        debug: bool,
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
    /// Check Rust formatting, compilation, and architecture rules.
    Check,
    /// Print a copyable AI prompt for fixing a check failure.
    FixPrompt,
    /// Format Photon-owned Rust and C++ files.
    Format,
    /// Verify and commit the current dependency branch tips as Photon pins.
    Pin,
    /// Merge Ladybird and GPUI-CE upstream changes into the Photon branches.
    Sync {
        /// Fetch and preview commit counts and merge conflicts without merging.
        #[arg(long)]
        check: bool,
    },
    /// Switch between pinned GPUI-CE and its persistent edit branch.
    Gpui {
        #[command(subcommand)]
        command: GpuiCommand,
    },
    /// Run Rust workspace tests.
    Test,
    /// Inspect or maintain the Photon Engine checkout.
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

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum CleanScope {
    Engine,
}

#[derive(Debug, Subcommand)]
enum EngineCommand {
    /// Show repository state and upstream commit counts.
    Status,
    /// Switch Engine to Photon’s persistent master development branch.
    Edit,
    /// Return Engine to the commit pinned by this Photon checkout.
    Pin,
    /// Build Photon Engine libraries and services.
    Build {
        /// Build with optimizations (the default).
        #[arg(long, conflicts_with = "debug")]
        release: bool,
        /// Build without optimizations.
        #[arg(long, conflicts_with = "release")]
        debug: bool,
    },
    /// Fetch and merge upstream/master into the Engine checkout.
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
        Some(Command::Setup) => commands::setup::setup(&root, verbose),
        Some(Command::Doctor) => commands::setup::doctor(&root),
        Some(Command::Ide { command }) => match command {
            IdeCommand::Setup => commands::ide::setup(&root, verbose),
        },
        Some(Command::Build { release, debug }) => {
            commands::build::build(&root, release || !debug, verbose)
        }
        Some(Command::Run {
            release,
            debug,
            url,
            shutdown_after_seconds,
        }) => commands::build::run_direct(
            &root,
            release || !debug,
            verbose,
            url.as_deref(),
            shutdown_after_seconds,
        ),
        Some(Command::Clean { scope }) => {
            let scope = scope.map(|scope| match scope {
                CleanScope::Engine => "engine",
            });
            commands::build::clean(&root, scope)
        }
        Some(Command::Check) => commands::check::check(&root, verbose),
        Some(Command::FixPrompt) => {
            commands::check::print_fix_prompt(
                &root,
                "Paste the failing command and its output here.",
            );
            Ok(())
        }
        Some(Command::Format) => commands::format::format(&root, verbose),
        Some(Command::Pin) => commands::pin::pin(&root, verbose),
        Some(Command::Sync { check }) => {
            if check {
                commands::engine::check_upstream(&root, verbose)?;
                commands::gpui::check_upstream(&root, verbose)
            } else {
                commands::engine::engine(&root, Some("sync"), false, verbose)?;
                commands::gpui::sync(&root, verbose)
            }
        }
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
            Some(EngineCommand::Build { release, debug }) => {
                commands::engine::engine(&root, Some("build"), release || !debug, verbose)
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
