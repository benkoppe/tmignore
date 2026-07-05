use camino::Utf8PathBuf;
use clap::{ArgAction, Args, Parser, Subcommand};

use crate::config::RunMode;
use crate::report::ReportVerbosity;

#[derive(Debug, Parser)]
#[command(name = "tmignore")]
#[command(about = "Exclude restoreable development dependency directories from Time Machine")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scan project roots for dependency directories and exclude them.
    Scan(ScanArgs),
    /// Process configured global dependency/cache directories.
    Global(GlobalArgs),
    /// Run both project scanning and global cache processing.
    All(AllArgs),
}

#[derive(Debug, Args)]
pub struct CommonArgs {
    /// Path to a tmignore TOML config file.
    #[arg(long, value_name = "PATH")]
    pub config: Option<Utf8PathBuf>,

    /// Report what would be excluded without changing Time Machine state (default).
    #[arg(long, conflicts_with = "apply")]
    pub dry_run: bool,

    /// Apply Time Machine exclusions.
    #[arg(long)]
    pub apply: bool,

    /// Increase report verbosity.
    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Scan root to walk; repeat for multiple roots. Replaces config file roots.
    #[arg(long, value_name = "PATH")]
    pub root: Vec<Utf8PathBuf>,

    /// Path to skip while scanning; repeat for multiple paths. Appended to config file skip paths.
    #[arg(long, value_name = "PATH")]
    pub skip: Vec<Utf8PathBuf>,
}

#[derive(Debug, Args)]
pub struct GlobalArgs {
    #[command(flatten)]
    pub common: CommonArgs,
}

#[derive(Debug, Args)]
pub struct AllArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Scan root to walk; repeat for multiple roots. Replaces config file roots.
    #[arg(long, value_name = "PATH")]
    pub root: Vec<Utf8PathBuf>,

    /// Path to skip while scanning; repeat for multiple paths. Appended to config file skip paths.
    #[arg(long, value_name = "PATH")]
    pub skip: Vec<Utf8PathBuf>,
}

impl CommonArgs {
    pub fn report_verbosity(&self) -> ReportVerbosity {
        self.verbose.into()
    }

    pub fn run_mode(&self) -> RunMode {
        if self.apply {
            RunMode::Apply
        } else {
            RunMode::DryRun
        }
    }
}
