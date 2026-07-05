use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub cases: Vec<RuleCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleCase {
    pub targets: Vec<Target>,
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub path: String,
    pub kind: TargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub any_of: Vec<Evidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub path: String,
    pub kind: EvidenceKind,
    pub base: EvidenceBase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    File,
    Directory,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceBase {
    Candidate,
    CandidateParent,
    TargetParent,
}

impl Rule {
    pub fn new(id: impl Into<String>, cases: Vec<RuleCase>) -> Self {
        Self {
            id: id.into(),
            cases,
        }
    }
}

impl RuleCase {
    pub fn new(targets: Vec<Target>, requirements: Vec<Requirement>) -> Self {
        Self {
            targets,
            requirements,
        }
    }
}

impl Target {
    pub fn directory(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            kind: TargetKind::Directory,
        }
    }
}

impl Requirement {
    pub fn any_of(evidence: Vec<Evidence>) -> Self {
        Self { any_of: evidence }
    }
}

impl Evidence {
    pub fn candidate(path: impl Into<String>, kind: EvidenceKind) -> Self {
        Self {
            path: path.into(),
            kind,
            base: EvidenceBase::Candidate,
        }
    }

    pub fn candidate_parent(path: impl Into<String>, kind: EvidenceKind) -> Self {
        Self {
            path: path.into(),
            kind,
            base: EvidenceBase::CandidateParent,
        }
    }

    pub fn target_parent(path: impl Into<String>, kind: EvidenceKind) -> Self {
        Self {
            path: path.into(),
            kind,
            base: EvidenceBase::TargetParent,
        }
    }

    pub fn resolve_against_target(
        &self,
        candidate_path: &Utf8Path,
        target_path: &str,
    ) -> Utf8PathBuf {
        match self.base {
            EvidenceBase::Candidate => candidate_path.join(&self.path),
            EvidenceBase::CandidateParent => candidate_path
                .parent()
                .unwrap_or_else(|| Utf8Path::new("."))
                .join(&self.path),
            EvidenceBase::TargetParent => {
                target_parent(candidate_path, target_path).join(&self.path)
            }
        }
    }
}

fn target_parent<'a>(candidate_path: &'a Utf8Path, target_path: &str) -> &'a Utf8Path {
    let mut parent = candidate_path;

    for _ in Utf8Path::new(target_path).components() {
        parent = parent.parent().unwrap_or_else(|| Utf8Path::new("."));
    }

    parent
}

const FILE: EvidenceKind = EvidenceKind::File;

pub fn default_rules() -> Vec<Rule> {
    vec![
        single_target_rule("node.node-modules", "node_modules", vec!["package.json"]),
        single_target_rule("node.parcel-cache", ".parcel-cache", vec!["package.json"]),
        single_target_rule("rust.cargo-target", "target", vec!["Cargo.toml"]),
        single_target_rule("php.composer-vendor", "vendor", vec!["composer.json"]),
        single_target_rule("go.vendor", "vendor", vec!["go.mod"]),
        Rule::new(
            "ruby.bundle-vendor",
            vec![RuleCase::new(
                vec![Target::directory("vendor/bundle")],
                vec![Requirement::any_of(vec![Evidence::target_parent(
                    "Gemfile", FILE,
                )])],
            )],
        ),
        multi_target_rule(
            "python.venv",
            vec![".venv", "venv"],
            vec!["pyproject.toml", "requirements.txt"],
        ),
        single_target_rule("python.tox", ".tox", vec!["tox.ini"]),
        single_target_rule("python.nox", ".nox", vec!["noxfile.py"]),
        single_target_rule("swift.build", ".build", vec!["Package.swift"]),
        single_target_rule("elixir.deps", "deps", vec!["mix.exs"]),
        single_target_rule("elixir.build", "_build", vec!["mix.exs"]),
        single_target_rule(
            "gradle.cache",
            ".gradle",
            vec![
                "build.gradle",
                "build.gradle.kts",
                "settings.gradle",
                "settings.gradle.kts",
            ],
        ),
        single_target_rule(
            "gradle.build",
            "build",
            vec!["build.gradle", "build.gradle.kts"],
        ),
        single_target_rule("dart.tool", ".dart_tool", vec!["pubspec.yaml"]),
        single_target_rule("dart.build", "build", vec!["pubspec.yaml"]),
        single_target_rule("haskell.stack-work", ".stack-work", vec!["stack.yaml"]),
        single_target_rule("vagrant.state", ".vagrant", vec!["Vagrantfile"]),
        single_target_rule("ios.carthage", "Carthage", vec!["Cartfile"]),
        single_target_rule("ios.cocoapods", "Pods", vec!["Podfile"]),
        single_target_rule(
            "terragrunt.cache",
            ".terragrunt-cache",
            vec!["terragrunt.hcl"],
        ),
        single_target_rule("aws-cdk.out", "cdk.out", vec!["cdk.json"]),
        single_target_rule("java.maven-target", "target", vec!["pom.xml"]),
        single_target_rule(
            "scala.sbt-target",
            "target",
            vec!["build.sbt", "project/plugins.sbt"],
        ),
    ]
}

fn single_target_rule(id: &str, target: &str, parent_evidence: Vec<&str>) -> Rule {
    multi_target_rule(id, vec![target], parent_evidence)
}

fn multi_target_rule(id: &str, targets: Vec<&str>, parent_evidence: Vec<&str>) -> Rule {
    Rule::new(
        id,
        vec![RuleCase::new(
            targets.into_iter().map(Target::directory).collect(),
            vec![Requirement::any_of(
                parent_evidence
                    .into_iter()
                    .map(|path| Evidence::candidate_parent(path, FILE))
                    .collect(),
            )],
        )],
    )
}
