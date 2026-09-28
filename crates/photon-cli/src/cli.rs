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
    /// Check prerequisites and initialize the Engine checkout.
    Setup,
    /// Report toolchain, Qt, Engine, and build environment details.
    Doctor,
    /// Configure and validate Photon editor metadata.
    Ide {
        #[command(subcommand)]
        command: IdeCommand,
    },
    /// Run checks, then build Photon Engine and the Qt Quick shell.
    Build {
        /// Build with optimizations.
        #[arg(long)]
        release: bool,
    },
    /// Run checks, build Photon, and launch it.
    Run {
        /// Build with optimizations.
        #[arg(long)]
        release: bool,
        /// Deprecated CPU painting fallback for Vulkan troubleshooting; may be removed.
        #[arg(long)]
        force_cpu_painting: bool,
    },
    /// Remove generated build directories.
    Clean {
        /// Remove only Engine build directories.
        #[arg(value_enum)]
        scope: Option<CleanScope>,
    },
    /// Check Rust formatting, compilation, QML lint, and architecture rules.
    Check,
    /// Print a copyable AI prompt for fixing a check failure.
    FixPrompt,
    /// Format Photon-owned Rust, C++, and QML files.
    Format,
    /// Run Rust workspace tests.
    Test,
    /// Inspect or maintain the Photon Engine checkout.
    Engine {
        #[command(subcommand)]
        command: Option<EngineCommand>,
    },
    /// Import assets from supported providers.
    Import {
        #[command(subcommand)]
        command: ImportCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ImportCommand {
    /// Import an SVG icon into Photon UI assets.
    Icon {
        /// Icon name in the selected provider.
        name: String,
        /// Asset provider (currently: lucide).
        #[arg(long = "from")]
        provider: String,
        /// Local name for the imported asset.
        #[arg(long = "as")]
        alias: Option<String>,
        /// Replace an existing icon and its manifest entry.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
enum IdeCommand {
    /// Prepare the debug build metadata used by editor language servers.
    Setup,
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
            force_cpu_painting,
        }) => commands::build::watch_run(&root, release, verbose, force_cpu_painting),
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
        Some(Command::Import { command }) => match command {
            ImportCommand::Icon {
                name,
                provider,
                alias,
                force,
            } => commands::import::icon(&root, &name, &provider, alias.as_deref(), force),
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
