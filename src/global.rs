use std::io;

use camino::{Utf8Path, Utf8PathBuf};

use crate::backend::TimeMachineBackend;
use crate::config::PreparedGlobalConfig;
use crate::run::{ExclusionOutcome, apply_exclusion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalRule {
    pub id: String,
    pub path: Utf8PathBuf,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GlobalScanReport {
    pub home: Utf8PathBuf,
    pub matches: Vec<GlobalMatch>,
    pub absent: Vec<GlobalAbsent>,
    pub skipped: Vec<GlobalSkipped>,
    pub failures: Vec<GlobalFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalMatch {
    pub path: Utf8PathBuf,
    pub rule_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAbsent {
    pub path: Utf8PathBuf,
    pub rule_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSkipped {
    pub path: Utf8PathBuf,
    pub requested_path: Utf8PathBuf,
    pub rule_id: String,
    pub reason: GlobalSkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalSkipReason {
    Symlink,
    NotDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalFailure {
    pub path: Utf8PathBuf,
    pub rule_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalRunReport {
    pub scan: GlobalScanReport,
    pub actions: Vec<GlobalAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAction {
    pub path: Utf8PathBuf,
    pub rule_id: String,
    pub outcome: ExclusionOutcome,
}

impl GlobalRule {
    pub fn home_relative(id: impl Into<String>, path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
        }
    }
}

impl GlobalRunReport {
    pub fn dry_run(scan: GlobalScanReport) -> Self {
        let actions = scan
            .matches
            .iter()
            .map(|global_match| GlobalAction {
                path: global_match.path.clone(),
                rule_id: global_match.rule_id.clone(),
                outcome: ExclusionOutcome::DryRun,
            })
            .collect();

        Self { scan, actions }
    }

    pub fn apply(scan: GlobalScanReport, backend: &impl TimeMachineBackend) -> Self {
        let actions = scan
            .matches
            .iter()
            .map(|global_match| GlobalAction {
                path: global_match.path.clone(),
                rule_id: global_match.rule_id.clone(),
                outcome: apply_exclusion(&global_match.path, backend),
            })
            .collect();

        Self { scan, actions }
    }

    pub fn has_failures(&self) -> bool {
        !self.scan.failures.is_empty()
            || self.actions.iter().any(|action| {
                matches!(
                    action.outcome,
                    ExclusionOutcome::StatusFailed(_) | ExclusionOutcome::AddFailed(_)
                )
            })
    }
}

pub fn scan_global(config: &PreparedGlobalConfig) -> GlobalScanReport {
    let mut report = GlobalScanReport {
        home: config.home.clone(),
        ..GlobalScanReport::default()
    };

    for rule in &config.rules {
        let path = resolve_rule_path(rule, &config.home);

        match global_path_status(&path, &config.home) {
            GlobalPathStatus::Directory => {
                report.matches.push(GlobalMatch {
                    path,
                    rule_id: rule.id.clone(),
                });
            }
            GlobalPathStatus::Absent => {
                report.absent.push(GlobalAbsent {
                    path,
                    rule_id: rule.id.clone(),
                });
            }
            GlobalPathStatus::Symlink(symlink_path) => {
                report.skipped.push(GlobalSkipped {
                    path: symlink_path,
                    requested_path: path,
                    rule_id: rule.id.clone(),
                    reason: GlobalSkipReason::Symlink,
                });
            }
            GlobalPathStatus::NotDirectory => {
                report.skipped.push(GlobalSkipped {
                    path: path.clone(),
                    requested_path: path,
                    rule_id: rule.id.clone(),
                    reason: GlobalSkipReason::NotDirectory,
                });
            }
            GlobalPathStatus::Failure(error) => {
                report.failures.push(GlobalFailure {
                    path,
                    rule_id: rule.id.clone(),
                    message: error.to_string(),
                });
            }
        }
    }

    report
}

enum GlobalPathStatus {
    Directory,
    Absent,
    Symlink(Utf8PathBuf),
    NotDirectory,
    Failure(io::Error),
}

fn global_path_status(path: &Utf8Path, home: &Utf8Path) -> GlobalPathStatus {
    match fs_err::symlink_metadata(home) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return GlobalPathStatus::Symlink(home.to_path_buf());
        }
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => return GlobalPathStatus::NotDirectory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return GlobalPathStatus::Absent,
        Err(error) => return GlobalPathStatus::Failure(error),
    }

    let Ok(relative_path) = path.strip_prefix(home) else {
        return GlobalPathStatus::NotDirectory;
    };
    let mut current = home.to_path_buf();

    for component in relative_path.components() {
        match component.as_str() {
            "/" | "." => continue,
            component => current.push(component),
        }

        match fs_err::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return GlobalPathStatus::Symlink(current.clone());
            }
            Ok(metadata) if current == path => {
                return if metadata.file_type().is_dir() {
                    GlobalPathStatus::Directory
                } else {
                    GlobalPathStatus::NotDirectory
                };
            }
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => return GlobalPathStatus::NotDirectory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return GlobalPathStatus::Absent;
            }
            Err(error) => return GlobalPathStatus::Failure(error),
        }
    }

    GlobalPathStatus::NotDirectory
}

pub fn resolve_rule_path(rule: &GlobalRule, home: &Utf8Path) -> Utf8PathBuf {
    if rule.path.is_absolute() {
        rule.path.clone()
    } else {
        home.join(&rule.path)
    }
}

pub fn default_global_rules() -> Vec<GlobalRule> {
    vec![
        GlobalRule::home_relative("cargo.registry", ".cargo/registry"),
        GlobalRule::home_relative("cargo.git", ".cargo/git"),
        GlobalRule::home_relative("rustup.toolchains", ".rustup/toolchains"),
        GlobalRule::home_relative("go.module-cache", "go/pkg/mod"),
        GlobalRule::home_relative("gradle.caches", ".gradle/caches"),
        GlobalRule::home_relative("maven.repository", ".m2/repository"),
        GlobalRule::home_relative("npm.cache", ".npm/_cacache"),
        GlobalRule::home_relative("pnpm.store", "Library/pnpm/store"),
        GlobalRule::home_relative("bun.install-cache", ".bun/install/cache"),
        GlobalRule::home_relative("composer.cache", ".composer/cache"),
        GlobalRule::home_relative("ivy.cache", ".ivy2/cache"),
        GlobalRule::home_relative("cocoapods.repos", ".cocoapods/repos"),
        GlobalRule::home_relative("vagrant.boxes", ".vagrant.d/boxes"),
        GlobalRule::home_relative("terraform.plugin-cache", ".terraform.d/plugin-cache"),
        GlobalRule::home_relative("xcode.derived-data", "Library/Developer/Xcode/DerivedData"),
        GlobalRule::home_relative("ollama.models", ".ollama/models"),
    ]
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::config::PreparedGlobalConfig;

    #[test]
    fn matches_existing_global_cache_directories() {
        let fixture = Fixture::new();
        fixture.dir(".cargo/registry");

        let report = scan_global(&fixture.config(vec![GlobalRule::home_relative(
            "cargo.registry",
            ".cargo/registry",
        )]));

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].rule_id, "cargo.registry");
        assert_eq!(report.matches[0].path, fixture.path(".cargo/registry"));
        assert!(report.absent.is_empty());
    }

    #[test]
    fn reports_absent_global_cache_directories_without_failure() {
        let fixture = Fixture::new();

        let report = scan_global(&fixture.config(vec![GlobalRule::home_relative(
            "cargo.registry",
            ".cargo/registry",
        )]));

        assert!(report.matches.is_empty());
        assert_eq!(report.absent.len(), 1);
        assert!(report.failures.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn skips_global_cache_symlinks() {
        let fixture = Fixture::new();
        fixture.dir("real-cache");
        std::os::unix::fs::symlink(fixture.path("real-cache"), fixture.path("cache-link")).unwrap();

        let report = scan_global(&fixture.config(vec![GlobalRule::home_relative(
            "custom.cache",
            "cache-link",
        )]));

        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].reason, GlobalSkipReason::Symlink);
        assert_eq!(report.skipped[0].path, fixture.path("cache-link"));
        assert_eq!(report.skipped[0].requested_path, fixture.path("cache-link"));
    }

    #[cfg(unix)]
    #[test]
    fn skips_global_cache_paths_with_intermediate_symlinks() {
        let fixture = Fixture::new();
        fixture.dir("real-cargo/registry");
        std::os::unix::fs::symlink(fixture.path("real-cargo"), fixture.path(".cargo")).unwrap();

        let report = scan_global(&fixture.config(vec![GlobalRule::home_relative(
            "cargo.registry",
            ".cargo/registry",
        )]));

        assert!(report.matches.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].reason, GlobalSkipReason::Symlink);
        assert_eq!(report.skipped[0].path, fixture.path(".cargo"));
        assert_eq!(
            report.skipped[0].requested_path,
            fixture.path(".cargo/registry")
        );
    }

    #[cfg(unix)]
    #[test]
    fn skips_global_cache_paths_when_home_is_a_symlink() {
        let fixture = Fixture::new();
        fixture.dir("real-home/.cargo/registry");
        std::os::unix::fs::symlink(fixture.path("real-home"), fixture.path("linked-home")).unwrap();

        let config = PreparedGlobalConfig {
            home: fixture.path("linked-home"),
            rules: vec![GlobalRule::home_relative(
                "cargo.registry",
                ".cargo/registry",
            )],
        };
        let report = scan_global(&config);

        assert!(report.matches.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].reason, GlobalSkipReason::Symlink);
        assert_eq!(report.skipped[0].path, fixture.path("linked-home"));
        assert_eq!(
            report.skipped[0].requested_path,
            fixture.path("linked-home/.cargo/registry")
        );
    }

    struct Fixture {
        temp_dir: TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                temp_dir: tempfile::tempdir().unwrap(),
            }
        }

        fn root(&self) -> Utf8PathBuf {
            Utf8PathBuf::from_path_buf(self.temp_dir.path().to_path_buf()).unwrap()
        }

        fn path(&self, path: &str) -> Utf8PathBuf {
            self.root().join(path)
        }

        fn dir(&self, path: &str) {
            fs_err::create_dir_all(self.path(path)).unwrap();
        }

        fn config(&self, rules: Vec<GlobalRule>) -> PreparedGlobalConfig {
            PreparedGlobalConfig {
                home: self.root(),
                rules,
            }
        }
    }
}
