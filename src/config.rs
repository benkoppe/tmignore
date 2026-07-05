use camino::Utf8PathBuf;

use crate::rule::{DEFAULT_RULES, Rule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    DryRun,
    Apply,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub roots: Vec<Utf8PathBuf>,
    pub skip_paths: Vec<Utf8PathBuf>,
    pub mode: RunMode,
    pub rules: &'static [Rule],
}

impl Config {
    pub fn new(roots: Vec<Utf8PathBuf>, skip_paths: Vec<Utf8PathBuf>, mode: RunMode) -> Self {
        Self {
            roots,
            skip_paths,
            mode,
            rules: DEFAULT_RULES,
        }
    }
}
