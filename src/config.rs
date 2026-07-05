use std::env;
use std::io;
use std::path::PathBuf;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use path_clean::PathClean;

use crate::rule::{Rule, default_rules};

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
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
pub struct PreparedConfig {
    pub roots: Vec<Utf8PathBuf>,
    pub skip_paths: Vec<Utf8PathBuf>,
    pub mode: RunMode,
    pub rules: Vec<Rule>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("at least one scan root is required")]
    MissingRoot,
    #[error("failed to determine current directory: {0}")]
    CurrentDir(#[source] io::Error),
    #[error("current directory is not valid UTF-8: {0}")]
    NonUtf8CurrentDir(PathBuf),
}

impl Config {
    pub fn new(roots: Vec<Utf8PathBuf>, skip_paths: Vec<Utf8PathBuf>, mode: RunMode) -> Self {
        Self {
            roots,
            skip_paths,
            mode,
            rules: default_rules(),
        }
    }

    pub fn prepare(&self) -> Result<PreparedConfig, ConfigError> {
        let cwd = env::current_dir().map_err(ConfigError::CurrentDir)?;
        let cwd = Utf8PathBuf::from_path_buf(cwd).map_err(ConfigError::NonUtf8CurrentDir)?;

        self.prepare_with_cwd(&cwd)
    }

    pub fn prepare_with_cwd(&self, cwd: &Utf8Path) -> Result<PreparedConfig, ConfigError> {
        let roots = prepare_paths(&self.roots, cwd);

        if roots.is_empty() {
            return Err(ConfigError::MissingRoot);
        }

        Ok(PreparedConfig {
            roots,
            skip_paths: prepare_paths(&self.skip_paths, cwd),
            mode: self.mode,
            rules: self.rules.clone(),
        })
    }
}

fn prepare_paths(paths: &[Utf8PathBuf], cwd: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut paths = paths
        .iter()
        .map(|path| clean_absolute_path(path, cwd))
        .collect::<Vec<_>>();

    paths.sort();
    paths.dedup();
    prune_nested_paths(paths)
}

fn clean_absolute_path(path: &Utf8Path, cwd: &Utf8Path) -> Utf8PathBuf {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };

    Utf8PathBuf::from_path_buf(absolute_path.as_std_path().clean())
        .expect("cleaning a UTF-8 path should preserve UTF-8")
}

fn prune_nested_paths(paths: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf> {
    let mut pruned = Vec::new();

    for path in paths {
        if !pruned.iter().any(|parent| path.starts_with(parent)) {
            pruned.push(path);
        }
    }

    pruned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_relative_roots_against_current_directory() {
        let config = Config::new(
            vec![Utf8PathBuf::from("projects")],
            Vec::new(),
            RunMode::DryRun,
        );

        let prepared = config.prepare_with_cwd(Utf8Path::new("/Users/me")).unwrap();

        assert_eq!(
            prepared.roots,
            vec![Utf8PathBuf::from("/Users/me/projects")]
        );
    }

    #[test]
    fn cleans_dot_and_dot_dot_components() {
        let config = Config::new(
            vec![Utf8PathBuf::from("./projects/../Code")],
            vec![Utf8PathBuf::from("./projects/../Code/vendor")],
            RunMode::DryRun,
        );

        let prepared = config.prepare_with_cwd(Utf8Path::new("/Users/me")).unwrap();

        assert_eq!(prepared.roots, vec![Utf8PathBuf::from("/Users/me/Code")]);
        assert_eq!(
            prepared.skip_paths,
            vec![Utf8PathBuf::from("/Users/me/Code/vendor")]
        );
    }

    #[test]
    fn deduplicates_roots_and_skip_paths() {
        let config = Config::new(
            vec![
                Utf8PathBuf::from("/Users/me/Code"),
                Utf8PathBuf::from("/Users/me/./Code"),
            ],
            vec![
                Utf8PathBuf::from("/Users/me/Code/vendor"),
                Utf8PathBuf::from("/Users/me/Code/./vendor"),
            ],
            RunMode::DryRun,
        );

        let prepared = config.prepare_with_cwd(Utf8Path::new("/unused")).unwrap();

        assert_eq!(prepared.roots, vec![Utf8PathBuf::from("/Users/me/Code")]);
        assert_eq!(
            prepared.skip_paths,
            vec![Utf8PathBuf::from("/Users/me/Code/vendor")]
        );
    }

    #[test]
    fn prunes_nested_roots_and_skip_paths() {
        let config = Config::new(
            vec![
                Utf8PathBuf::from("/Users/me/Code/project"),
                Utf8PathBuf::from("/Users/me/Code"),
            ],
            vec![
                Utf8PathBuf::from("/Users/me/Code/vendor/cache"),
                Utf8PathBuf::from("/Users/me/Code/vendor"),
            ],
            RunMode::DryRun,
        );

        let prepared = config.prepare_with_cwd(Utf8Path::new("/unused")).unwrap();

        assert_eq!(prepared.roots, vec![Utf8PathBuf::from("/Users/me/Code")]);
        assert_eq!(
            prepared.skip_paths,
            vec![Utf8PathBuf::from("/Users/me/Code/vendor")]
        );
    }

    #[test]
    fn does_not_treat_same_prefix_sibling_as_nested() {
        let config = Config::new(
            vec![
                Utf8PathBuf::from("/Users/me/Code"),
                Utf8PathBuf::from("/Users/me/Codegen"),
            ],
            Vec::new(),
            RunMode::DryRun,
        );

        let prepared = config.prepare_with_cwd(Utf8Path::new("/unused")).unwrap();

        assert_eq!(
            prepared.roots,
            vec![
                Utf8PathBuf::from("/Users/me/Code"),
                Utf8PathBuf::from("/Users/me/Codegen"),
            ]
        );
    }

    #[test]
    fn reports_missing_roots() {
        let config = Config::new(Vec::new(), Vec::new(), RunMode::DryRun);

        assert!(matches!(
            config.prepare_with_cwd(Utf8Path::new("/Users/me")),
            Err(ConfigError::MissingRoot)
        ));
    }
}
