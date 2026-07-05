use camino::Utf8PathBuf;
use clap::{ArgAction, Parser};

use crate::config::{Config, ConfigError, RunMode};
use crate::report::ReportVerbosity;

#[derive(Debug, Parser)]
#[command(name = "tmignore")]
#[command(about = "Exclude restoreable development dependency directories from Time Machine")]
pub struct Cli {
    #[arg(long, value_name = "PATH")]
    pub config: Option<Utf8PathBuf>,

    #[arg(long, value_name = "PATH")]
    pub root: Vec<Utf8PathBuf>,

    #[arg(long, value_name = "PATH")]
    pub skip: Vec<Utf8PathBuf>,

    #[arg(long, conflicts_with = "apply")]
    pub dry_run: bool,

    #[arg(long)]
    pub apply: bool,

    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,
}

impl Cli {
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

    pub fn into_config(self) -> Result<Config, ConfigError> {
        let mode = self.run_mode();
        Config::load(self.config.as_deref(), self.root, self.skip, mode)
    }
}
