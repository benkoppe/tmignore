use camino::Utf8PathBuf;
use clap::Parser;

use crate::config::Config;

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
}

impl Cli {
    pub fn into_config(self) -> Config {
        Config::new(self.root, self.skip, self.dry_run)
    }
}
