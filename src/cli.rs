use camino::Utf8PathBuf;
use clap::{ArgAction, Parser};

use crate::config::Config;
use crate::report::ReportVerbosity;

#[derive(Debug, Parser)]
#[command(name = "tmignore")]
#[command(about = "Exclude restoreable development dependency directories from Time Machine")]
pub struct Cli {
    #[arg(long, value_name = "PATH", required = true)]
    pub root: Vec<Utf8PathBuf>,

    #[arg(long, value_name = "PATH")]
    pub skip: Vec<Utf8PathBuf>,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,
}

impl Cli {
    pub fn report_verbosity(&self) -> ReportVerbosity {
        self.verbose.into()
    }

    pub fn into_config(self) -> Config {
        Config::new(self.root, self.skip, self.dry_run)
    }
}
