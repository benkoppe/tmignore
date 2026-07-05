use std::collections::{BTreeMap, HashSet};
use std::env;
use std::io;
use std::path::PathBuf;

use camino::Utf8Path;
use camino::Utf8PathBuf;
use figment::Figment;
use figment::providers::{Format, Toml};
use path_clean::PathClean;
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::global::{GlobalRule, default_global_rules};
use crate::rule::{Evidence, Requirement, Rule, RuleCase, default_rules};

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
pub struct AppConfig {
    pub scan: Config,
    pub global: GlobalConfig,
}

#[derive(Debug, Clone)]
pub struct GlobalConfig {
    pub rules: Vec<GlobalRule>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    scan: FileScanConfig,
    #[serde(default)]
    global: FileGlobalConfig,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileScanOnlyConfig {
    #[serde(default)]
    scan: FileScanConfig,
    #[serde(default, rename = "global")]
    _global: Option<IgnoredAny>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileGlobalOnlyConfig {
    #[serde(default, rename = "scan")]
    _scan: Option<IgnoredAny>,
    #[serde(default)]
    global: FileGlobalConfig,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileScanConfig {
    #[serde(default)]
    roots: Vec<Utf8PathBuf>,
    #[serde(default)]
    skip_paths: Vec<Utf8PathBuf>,
    #[serde(default)]
    builtin_rules: BuiltinRuleMode,
    #[serde(default)]
    disabled_builtin_rules: Vec<String>,
    #[serde(default)]
    extra_rules: BTreeMap<String, FileRule>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileGlobalConfig {
    #[serde(default)]
    builtin_rules: BuiltinRuleMode,
    #[serde(default)]
    disabled_builtin_rules: Vec<String>,
    #[serde(default)]
    extra_rules: BTreeMap<String, FileGlobalRule>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinRuleMode {
    #[default]
    Defaults,
    None,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRule {
    cases: Vec<RuleCase>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileGlobalRule {
    path: Utf8PathBuf,
}

#[derive(Debug, Clone)]
pub struct PreparedConfig {
    pub roots: Vec<Utf8PathBuf>,
    pub skip_paths: Vec<Utf8PathBuf>,
    pub mode: RunMode,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
pub struct PreparedGlobalConfig {
    pub home: Utf8PathBuf,
    pub rules: Vec<GlobalRule>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("at least one scan root is required")]
    MissingRoot,
    #[error("failed to determine current directory: {0}")]
    CurrentDir(#[source] io::Error),
    #[error("current directory is not valid UTF-8: {0}")]
    NonUtf8CurrentDir(PathBuf),
    #[error("failed to load config `{path}`: {source}")]
    LoadFile {
        path: Utf8PathBuf,
        #[source]
        source: Box<figment::Error>,
    },
    #[error("invalid rule `{rule_id}`: {message}")]
    InvalidRule { rule_id: String, message: String },
    #[error("invalid global rule `{rule_id}`: {message}")]
    InvalidGlobalRule { rule_id: String, message: String },
    #[error("failed to determine home directory")]
    MissingHomeDir,
    #[error("home directory is not valid UTF-8: {0}")]
    NonUtf8HomeDir(PathBuf),
}

impl AppConfig {
    pub fn load(config_path: Option<&Utf8Path>, mode: RunMode) -> Result<Self, ConfigError> {
        let file_config = match config_path {
            Some(path) => load_file_config(path)?,
            None => FileConfig::default(),
        };

        Ok(Self {
            scan: Config::from_file(file_config.scan, Vec::new(), Vec::new(), mode)?,
            global: GlobalConfig::from_file(file_config.global)?,
        })
    }

    pub fn load_scan(config_path: Option<&Utf8Path>, mode: RunMode) -> Result<Config, ConfigError> {
        let file_config = match config_path {
            Some(path) => load_file_scan_config(path)?,
            None => FileScanOnlyConfig::default(),
        };

        Config::from_file(file_config.scan, Vec::new(), Vec::new(), mode)
    }

    pub fn load_global(config_path: Option<&Utf8Path>) -> Result<GlobalConfig, ConfigError> {
        let file_config = match config_path {
            Some(path) => load_file_global_config(path)?,
            None => FileGlobalOnlyConfig::default(),
        };

        GlobalConfig::from_file(file_config.global)
    }
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

    pub fn load(
        config_path: Option<&Utf8Path>,
        cli_roots: Vec<Utf8PathBuf>,
        cli_skip_paths: Vec<Utf8PathBuf>,
        mode: RunMode,
    ) -> Result<Self, ConfigError> {
        let file_config = match config_path {
            Some(path) => load_file_config(path)?,
            None => FileConfig::default(),
        };

        Self::from_file(file_config.scan, cli_roots, cli_skip_paths, mode)
    }

    pub fn with_cli_paths(
        mut self,
        cli_roots: Vec<Utf8PathBuf>,
        cli_skip_paths: Vec<Utf8PathBuf>,
    ) -> Self {
        if !cli_roots.is_empty() {
            self.roots = cli_roots;
        }

        self.skip_paths.extend(cli_skip_paths);
        self
    }

    fn from_file(
        file_config: FileScanConfig,
        cli_roots: Vec<Utf8PathBuf>,
        cli_skip_paths: Vec<Utf8PathBuf>,
        mode: RunMode,
    ) -> Result<Self, ConfigError> {
        let roots = if cli_roots.is_empty() {
            file_config.roots
        } else {
            cli_roots
        };

        let mut skip_paths = file_config.skip_paths;
        skip_paths.extend(cli_skip_paths);

        let rules = build_rules(
            file_config.builtin_rules,
            file_config.disabled_builtin_rules,
            file_config.extra_rules,
        )?;

        Ok(Self {
            roots,
            skip_paths,
            mode,
            rules,
        })
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

impl GlobalConfig {
    fn from_file(file_config: FileGlobalConfig) -> Result<Self, ConfigError> {
        Ok(Self {
            rules: build_global_rules(
                file_config.builtin_rules,
                file_config.disabled_builtin_rules,
                file_config.extra_rules,
            )?,
        })
    }

    pub fn prepare(&self) -> Result<PreparedGlobalConfig, ConfigError> {
        let home = env::home_dir().ok_or(ConfigError::MissingHomeDir)?;
        let home = Utf8PathBuf::from_path_buf(home).map_err(ConfigError::NonUtf8HomeDir)?;

        self.prepare_with_home(&home)
    }

    pub fn prepare_with_home(&self, home: &Utf8Path) -> Result<PreparedGlobalConfig, ConfigError> {
        Ok(PreparedGlobalConfig {
            home: clean_absolute_path(home, Utf8Path::new("/")),
            rules: self.rules.clone(),
        })
    }
}

fn load_file_config(path: &Utf8Path) -> Result<FileConfig, ConfigError> {
    Figment::new()
        .merge(Toml::file(path.as_std_path()))
        .extract()
        .map_err(|source| ConfigError::LoadFile {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
}

fn load_file_scan_config(path: &Utf8Path) -> Result<FileScanOnlyConfig, ConfigError> {
    Figment::new()
        .merge(Toml::file(path.as_std_path()))
        .extract()
        .map_err(|source| ConfigError::LoadFile {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
}

fn load_file_global_config(path: &Utf8Path) -> Result<FileGlobalOnlyConfig, ConfigError> {
    Figment::new()
        .merge(Toml::file(path.as_std_path()))
        .extract()
        .map_err(|source| ConfigError::LoadFile {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
}

fn build_rules(
    builtin_rules: BuiltinRuleMode,
    disabled_builtin_rules: Vec<String>,
    extra_rules: BTreeMap<String, FileRule>,
) -> Result<Vec<Rule>, ConfigError> {
    let builtin_catalog = default_rules();
    let builtin_rule_ids = builtin_catalog
        .iter()
        .map(|rule| rule.id.clone())
        .collect::<HashSet<_>>();
    let disabled_builtin_rule_ids = disabled_builtin_rules.into_iter().collect::<HashSet<_>>();

    for rule_id in &disabled_builtin_rule_ids {
        if !builtin_rule_ids.contains(rule_id) {
            return Err(ConfigError::InvalidRule {
                rule_id: rule_id.clone(),
                message: "disabled built-in rule id does not exist".to_string(),
            });
        }
    }

    let mut rules = match builtin_rules {
        BuiltinRuleMode::Defaults => builtin_catalog
            .into_iter()
            .filter(|rule| !disabled_builtin_rule_ids.contains(&rule.id))
            .collect(),
        BuiltinRuleMode::None => Vec::new(),
    };
    let mut rule_ids = rules
        .iter()
        .map(|rule| rule.id.clone())
        .collect::<HashSet<_>>();

    for (rule_id, file_rule) in extra_rules {
        if builtin_rule_ids.contains(&rule_id) {
            return Err(ConfigError::InvalidRule {
                rule_id,
                message: "rule id collides with a built-in rule".to_string(),
            });
        }

        let rule = Rule::new(rule_id.clone(), file_rule.cases);
        validate_rule(&rule)?;

        if !rule_ids.insert(rule_id.clone()) {
            return Err(ConfigError::InvalidRule {
                rule_id,
                message: "rule id collides with another enabled rule".to_string(),
            });
        }

        rules.push(rule);
    }

    Ok(rules)
}

fn build_global_rules(
    builtin_rules: BuiltinRuleMode,
    disabled_builtin_rules: Vec<String>,
    extra_rules: BTreeMap<String, FileGlobalRule>,
) -> Result<Vec<GlobalRule>, ConfigError> {
    let builtin_catalog = default_global_rules();
    for rule in &builtin_catalog {
        validate_global_rule(&rule.id, &rule.path)?;
    }

    let builtin_rule_ids = builtin_catalog
        .iter()
        .map(|rule| rule.id.clone())
        .collect::<HashSet<_>>();
    let disabled_builtin_rule_ids = disabled_builtin_rules.into_iter().collect::<HashSet<_>>();

    for rule_id in &disabled_builtin_rule_ids {
        if !builtin_rule_ids.contains(rule_id) {
            return Err(ConfigError::InvalidGlobalRule {
                rule_id: rule_id.clone(),
                message: "disabled built-in global rule id does not exist".to_string(),
            });
        }
    }

    let mut rules = match builtin_rules {
        BuiltinRuleMode::Defaults => builtin_catalog
            .into_iter()
            .filter(|rule| !disabled_builtin_rule_ids.contains(&rule.id))
            .collect(),
        BuiltinRuleMode::None => Vec::new(),
    };
    let mut rule_ids = rules
        .iter()
        .map(|rule| rule.id.clone())
        .collect::<HashSet<_>>();

    for (rule_id, file_rule) in extra_rules {
        if builtin_rule_ids.contains(&rule_id) {
            return Err(ConfigError::InvalidGlobalRule {
                rule_id,
                message: "global rule id collides with a built-in global rule".to_string(),
            });
        }

        validate_global_rule(&rule_id, &file_rule.path)?;

        if !rule_ids.insert(rule_id.clone()) {
            return Err(ConfigError::InvalidGlobalRule {
                rule_id,
                message: "global rule id collides with another enabled global rule".to_string(),
            });
        }

        rules.push(GlobalRule::home_relative(rule_id, file_rule.path));
    }

    Ok(rules)
}

fn validate_global_rule(rule_id: &str, path: &Utf8Path) -> Result<(), ConfigError> {
    if !is_valid_rule_id(rule_id) {
        return invalid_global_rule(
            rule_id,
            "global rule id must contain only ASCII letters, numbers, `.`, `_`, or `-`",
        );
    }

    validate_global_rule_path(rule_id, path)
}

fn validate_global_rule_path(rule_id: &str, path: &Utf8Path) -> Result<(), ConfigError> {
    if path.as_str().is_empty() {
        return invalid_global_rule(rule_id, "global rule path must not be empty");
    }

    if path.as_str() == "." {
        return invalid_global_rule(rule_id, "global rule path must not be the home directory");
    }

    if path.as_str() == "/" {
        return invalid_global_rule(rule_id, "global rule path must not be the filesystem root");
    }

    if path.is_absolute() {
        return invalid_global_rule(
            rule_id,
            "global rule path must be relative to the home directory",
        );
    }

    if path.as_str().starts_with('~') {
        return invalid_global_rule(
            rule_id,
            "global rule path must be relative to the home directory; `~` is not expanded",
        );
    }

    if path
        .components()
        .any(|component| component.as_str() == "..")
    {
        return invalid_global_rule(rule_id, "global rule path must not contain `..`");
    }

    if path.components().any(|component| component.as_str() == ".") {
        return invalid_global_rule(rule_id, "global rule path must not contain `.` components");
    }

    if normal_component_count(path) < 2 {
        return invalid_global_rule(
            rule_id,
            "global rule path is too broad; use a precise cache subdirectory",
        );
    }

    if let Some(last_component) = path.components().next_back()
        && is_broad_global_component(last_component.as_str())
    {
        return invalid_global_rule(
            rule_id,
            "global rule path is too broad; use a precise cache subdirectory",
        );
    }

    if !is_allowed_global_cache_path(path) {
        return invalid_global_rule(
            rule_id,
            "global rule path must be under a known cache namespace",
        );
    }

    Ok(())
}

fn normal_component_count(path: &Utf8Path) -> usize {
    path.components()
        .filter(|component| !matches!(component.as_str(), "/" | "."))
        .count()
}

fn is_broad_global_component(component: &str) -> bool {
    matches!(
        component,
        "Library"
            | ".cache"
            | ".config"
            | ".local"
            | ".terraform.d"
            | ".vagrant.d"
            | ".sbt"
            | ".gradle"
            | ".cargo"
            | ".rustup"
            | ".m2"
            | ".npm"
            | ".cocoapods"
    )
}

fn is_allowed_global_cache_path(path: &Utf8Path) -> bool {
    let components = normal_components(path);

    allowed_global_cache_prefixes()
        .iter()
        .any(|prefix| components_starts_with(&components, prefix))
}

fn normal_components(path: &Utf8Path) -> Vec<&str> {
    path.components()
        .filter_map(|component| {
            let component = component.as_str();
            if matches!(component, "/" | ".") {
                None
            } else {
                Some(component)
            }
        })
        .collect()
}

fn components_starts_with(components: &[&str], prefix: &[&str]) -> bool {
    components.len() >= prefix.len()
        && components
            .iter()
            .zip(prefix.iter())
            .all(|(component, prefix)| component == prefix)
}

fn allowed_global_cache_prefixes() -> &'static [&'static [&'static str]] {
    &[
        &[".cargo", "registry"],
        &[".cargo", "git"],
        &[".rustup", "toolchains"],
        &["go", "pkg", "mod"],
        &[".gradle", "caches"],
        &[".m2", "repository"],
        &[".npm", "_cacache"],
        &["Library", "pnpm", "store"],
        &[".bun", "install", "cache"],
        &[".composer", "cache"],
        &[".ivy2", "cache"],
        &[".cocoapods", "repos"],
        &[".vagrant.d", "boxes"],
        &[".terraform.d", "plugin-cache"],
        &["Library", "Developer", "Xcode", "DerivedData"],
        &[".ollama", "models"],
    ]
}

fn validate_rule(rule: &Rule) -> Result<(), ConfigError> {
    if !is_valid_rule_id(&rule.id) {
        return invalid_rule(
            rule,
            "rule id must contain only ASCII letters, numbers, `.`, `_`, or `-`",
        );
    }

    if rule.cases.is_empty() {
        return invalid_rule(rule, "at least one case is required");
    }

    for rule_case in &rule.cases {
        validate_rule_case(rule, rule_case)?;
    }

    Ok(())
}

fn validate_rule_case(rule: &Rule, rule_case: &RuleCase) -> Result<(), ConfigError> {
    if rule_case.targets.is_empty() {
        return invalid_rule(rule, "each case must declare at least one target");
    }

    if rule_case.requirements.is_empty() {
        return invalid_rule(rule, "each case must declare at least one requirement");
    }

    for target in &rule_case.targets {
        validate_rule_path(rule, "target", &target.path)?;
    }

    for requirement in &rule_case.requirements {
        validate_requirement(rule, requirement)?;
    }

    Ok(())
}

fn validate_requirement(rule: &Rule, requirement: &Requirement) -> Result<(), ConfigError> {
    if requirement.any_of.is_empty() {
        return invalid_rule(
            rule,
            "each requirement must declare at least one evidence entry",
        );
    }

    for evidence in &requirement.any_of {
        validate_evidence(rule, evidence)?;
    }

    Ok(())
}

fn validate_evidence(rule: &Rule, evidence: &Evidence) -> Result<(), ConfigError> {
    validate_rule_path(rule, "evidence", &evidence.path)
}

fn validate_rule_path(rule: &Rule, label: &'static str, path: &str) -> Result<(), ConfigError> {
    if path.is_empty() {
        return invalid_rule(rule, format!("{label} path must not be empty"));
    }

    let path = Utf8Path::new(path);

    if path.is_absolute() {
        return invalid_rule(rule, format!("{label} path must be relative"));
    }

    if path
        .components()
        .any(|component| component.as_str() == "..")
    {
        return invalid_rule(rule, format!("{label} path must not contain `..`"));
    }

    Ok(())
}

fn is_valid_rule_id(rule_id: &str) -> bool {
    !rule_id.is_empty()
        && rule_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn invalid_rule<T>(rule: &Rule, message: impl Into<String>) -> Result<T, ConfigError> {
    Err(ConfigError::InvalidRule {
        rule_id: rule.id.clone(),
        message: message.into(),
    })
}

fn invalid_global_rule<T>(rule_id: &str, message: impl Into<String>) -> Result<T, ConfigError> {
    Err(ConfigError::InvalidGlobalRule {
        rule_id: rule_id.to_string(),
        message: message.into(),
    })
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

    struct Fixture {
        temp_dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                temp_dir: tempfile::tempdir().unwrap(),
            }
        }

        fn path(&self, path: &str) -> Utf8PathBuf {
            Utf8PathBuf::from_path_buf(self.temp_dir.path().join(path)).unwrap()
        }

        fn config_file(&self, contents: &str) -> Utf8PathBuf {
            let path = self.path("tmignore.toml");
            fs_err::write(&path, contents).unwrap();
            path
        }
    }

    const CUSTOM_RULE_CONFIG: &str = r#"
[scan.extra_rules.custom_cache]
[[scan.extra_rules.custom_cache.cases]]
targets = [{ path = ".custom-cache", kind = "directory" }]
requirements = [
  { any_of = [{ path = "custom.toml", kind = "file", base = "candidate_parent" }] },
]
"#;

    #[test]
    fn loads_config_file_roots_and_skip_paths() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(
            r#"
[scan]
roots = ["projects"]
skip_paths = ["projects/archive"]
"#,
        );

        let config =
            Config::load(Some(&config_path), Vec::new(), Vec::new(), RunMode::DryRun).unwrap();

        assert_eq!(config.roots, vec![Utf8PathBuf::from("projects")]);
        assert_eq!(
            config.skip_paths,
            vec![Utf8PathBuf::from("projects/archive")]
        );
        assert_eq!(config.rules.len(), default_rules().len());
    }

    #[test]
    fn cli_roots_replace_config_roots_and_cli_skips_append() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(
            r#"
[scan]
roots = ["config-root"]
skip_paths = ["config-skip"]
"#,
        );

        let config = Config::load(
            Some(&config_path),
            vec![Utf8PathBuf::from("cli-root")],
            vec![Utf8PathBuf::from("cli-skip")],
            RunMode::DryRun,
        )
        .unwrap();

        assert_eq!(config.roots, vec![Utf8PathBuf::from("cli-root")]);
        assert_eq!(
            config.skip_paths,
            vec![
                Utf8PathBuf::from("config-skip"),
                Utf8PathBuf::from("cli-skip"),
            ]
        );
    }

    #[test]
    fn builtin_rules_none_uses_only_extra_rules() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(&format!(
            "[scan]\nbuiltin_rules = \"none\"\n{CUSTOM_RULE_CONFIG}"
        ));

        let config =
            Config::load(Some(&config_path), Vec::new(), Vec::new(), RunMode::DryRun).unwrap();

        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].id, "custom_cache");
    }

    #[test]
    fn disabled_builtin_rules_remove_specific_defaults() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(
            r#"
[scan]
disabled_builtin_rules = ["node.node-modules"]
"#,
        );

        let config =
            Config::load(Some(&config_path), Vec::new(), Vec::new(), RunMode::DryRun).unwrap();

        assert_eq!(config.rules.len(), default_rules().len() - 1);
        assert!(
            config
                .rules
                .iter()
                .all(|rule| rule.id != "node.node-modules")
        );
    }

    #[test]
    fn rejects_unknown_disabled_builtin_rules() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(
            r#"
[scan]
disabled_builtin_rules = ["missing.rule"]
"#,
        );

        let error =
            Config::load(Some(&config_path), Vec::new(), Vec::new(), RunMode::DryRun).unwrap_err();

        assert!(matches!(
            error,
            ConfigError::InvalidRule { ref rule_id, ref message }
                if rule_id == "missing.rule" && message.contains("does not exist")
        ));
    }

    #[test]
    fn rejects_extra_rule_ids_that_collide_with_builtin_rules() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(
            r#"
[scan.extra_rules."node.node-modules"]
[[scan.extra_rules."node.node-modules".cases]]
targets = [{ path = ".custom-cache", kind = "directory" }]
requirements = [
  { any_of = [{ path = "custom.toml", kind = "file", base = "candidate_parent" }] },
]
"#,
        );

        let error =
            Config::load(Some(&config_path), Vec::new(), Vec::new(), RunMode::DryRun).unwrap_err();

        assert!(matches!(
            error,
            ConfigError::InvalidRule { ref rule_id, .. } if rule_id == "node.node-modules"
        ));
    }

    #[test]
    fn rejects_parent_components_in_rule_paths() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(
            r#"
[scan]
builtin_rules = "none"

[scan.extra_rules.bad_rule]
[[scan.extra_rules.bad_rule.cases]]
targets = [{ path = "../cache", kind = "directory" }]
requirements = [
  { any_of = [{ path = "custom.toml", kind = "file", base = "candidate_parent" }] },
]
"#,
        );

        let error =
            Config::load(Some(&config_path), Vec::new(), Vec::new(), RunMode::DryRun).unwrap_err();

        assert!(matches!(
            error,
            ConfigError::InvalidRule { ref rule_id, ref message }
                if rule_id == "bad_rule" && message.contains("target path must not contain")
        ));
    }

    #[test]
    fn rejects_broad_global_rule_paths() {
        let fixture = Fixture::new();

        for path in [
            ".",
            "/",
            "Library",
            "Library/Application Support",
            "Library/Mobile Documents",
            ".terraform.d",
            ".cargo",
            ".config/app",
            ".local/share/cache",
            "Documents/project",
            "Desktop/cache",
            "Downloads/cache",
            "/Users/alice/Documents",
            "~/.cache",
            "../cache",
        ] {
            let config_path = fixture.config_file(&format!(
                r#"
[global]
builtin_rules = "none"

[global.extra_rules.bad_global]
path = "{path}"
"#
            ));

            let error = AppConfig::load_global(Some(&config_path)).unwrap_err();

            assert!(
                matches!(error, ConfigError::InvalidGlobalRule { ref rule_id, .. } if rule_id == "bad_global"),
                "expected invalid global rule for {path}, got {error:?}"
            );
        }
    }

    #[test]
    fn accepts_precise_global_rule_paths() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(
            r#"
[global]
builtin_rules = "none"

[global.extra_rules.terraform_plugins]
path = ".terraform.d/plugin-cache"

[global.extra_rules.cargo_registry]
path = ".cargo/registry"

[global.extra_rules.pnpm_store]
path = "Library/pnpm/store"

[global.extra_rules.cargo_child]
path = ".cargo/registry/custom"
"#,
        );

        let config = AppConfig::load_global(Some(&config_path)).unwrap();

        assert_eq!(config.rules.len(), 4);
    }

    #[test]
    fn scan_loading_does_not_validate_unused_global_section() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(
            r#"
[scan]
roots = ["projects"]

[global.extra_rules.bad_global]
path = "."
"#,
        );

        let config = AppConfig::load_scan(Some(&config_path), RunMode::DryRun).unwrap();

        assert_eq!(config.roots, vec![Utf8PathBuf::from("projects")]);
    }

    #[test]
    fn global_loading_does_not_validate_unused_scan_section() {
        let fixture = Fixture::new();
        let config_path = fixture.config_file(
            r#"
[scan]
builtin_rules = "none"

[scan.extra_rules.bad_rule]
[[scan.extra_rules.bad_rule.cases]]
targets = [{ path = "../cache", kind = "directory" }]
requirements = [
  { any_of = [{ path = "custom.toml", kind = "file", base = "candidate_parent" }] },
]

[global]
builtin_rules = "none"
"#,
        );

        let config = AppConfig::load_global(Some(&config_path)).unwrap();

        assert!(config.rules.is_empty());
    }

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
