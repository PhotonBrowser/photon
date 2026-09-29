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
    /// Report toolchain, GPUIX, Engine, and build environment details.
    Doctor,
    /// Report Rust and TypeScript editor metadata.
    Ide {
        #[command(subcommand)]
        command: IdeCommand,
    },
    /// Run checks, then build Photon Engine and the GPUIX addon.
    Build {
        /// Build with optimizations.
        #[arg(long)]
        release: bool,
    },
    /// Build Photon and launch it.
    Run {
        /// Build with optimizations.
        #[arg(long)]
        release: bool,
        /// Build and launch the GPUIX shell without Photon Engine.
        #[arg(long)]
        ui: bool,
        /// Navigate the PhotonWebView to this URL after launch.
        #[arg(long)]
        url: Option<String>,
        /// Force Ladybird's CPU painting path for comparison.
        #[arg(long)]
        force_cpu_painting: bool,
    },
    /// Remove generated build directories.
    Clean {
        /// Remove only Engine build directories.
        #[arg(value_enum)]
        scope: Option<CleanScope>,
    },
    /// Check Rust formatting, compilation, TypeScript, and architecture rules.
    Check,
    /// Print a copyable AI prompt for fixing a check failure.
    FixPrompt,
    /// Format Photon-owned Rust, C++, and TypeScript files.
    Format,
    /// Switch between pinned GPUI dependencies and persistent edit branches.
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
    /// Switch GPUIX and Zed to their persistent Photon development branches.
    Edit,
    /// Return GPUIX and Zed to the revisions pinned by this Photon checkout.
    Pin,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum CleanScope {
    Engine,
}

#[derive(Debug, Subcommand)]
enum EngineCommand {
    /// Show repository state and upstream commit counts.
    Status,
    /// Build Photon Engine libraries and services.
    Build {
        /// Build with optimizations.
        #[arg(long)]
        release: bool,
    },
    /// Fetch and merge upstream/master into the Engine checkout.
    Sync,
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
        Some(Command::Build { release }) => commands::build::build(&root, release, verbose),
        Some(Command::Run {
            release,
            ui,
            url,
            force_cpu_painting,
        }) => {
            if ui {
                commands::build::run_ui(&root, release, verbose)
            } else {
                commands::build::run(&root, release, verbose, url.as_deref(), force_cpu_painting)
            }
        }
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
        Some(Command::Gpui { command }) => match command {
            GpuiCommand::Edit => commands::gpui::edit(&root, verbose),
            GpuiCommand::Pin => commands::gpui::pin(&root, verbose),
        },
        Some(Command::Test) => commands::tests::run(&root, verbose),
        Some(Command::Engine { command }) => match command {
            None | Some(EngineCommand::Status) => {
                commands::engine::engine(&root, Some("status"), false, verbose)
            }
            Some(EngineCommand::Build { release }) => {
                commands::engine::engine(&root, Some("build"), release, verbose)
            }
            Some(EngineCommand::Sync) => {
                commands::engine::engine(&root, Some("sync"), false, verbose)
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
