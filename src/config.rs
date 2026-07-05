use camino::Utf8PathBuf;

use crate::rule::{DEFAULT_RULES, Rule};

#[derive(Debug, Clone)]
pub struct Config {
    pub roots: Vec<Utf8PathBuf>,
    pub skip_paths: Vec<Utf8PathBuf>,
    pub dry_run: bool,
    pub rules: &'static [Rule],
}

impl Config {
    pub fn new(roots: Vec<Utf8PathBuf>, skip_paths: Vec<Utf8PathBuf>, dry_run: bool) -> Self {
        Self {
            roots,
            skip_paths,
            dry_run,
            rules: DEFAULT_RULES,
        }
    }
}
