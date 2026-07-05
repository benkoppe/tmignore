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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedRule {
    pub rule_id: String,
    pub target: Target,
    pub evidence: Vec<MatchedRuleEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedRuleEvidence {
    pub evidence: Evidence,
    pub path: Utf8PathBuf,
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

    pub fn resolve_against(&self, candidate_path: &Utf8Path) -> Utf8PathBuf {
        match self.base {
            EvidenceBase::Candidate => candidate_path.join(&self.path),
            EvidenceBase::CandidateParent => candidate_path
                .parent()
                .unwrap_or_else(|| Utf8Path::new("."))
                .join(&self.path),
        }
    }
}

const FILE: EvidenceKind = EvidenceKind::File;

pub fn default_rules() -> Vec<Rule> {
    vec![
        single_target_rule("node", "node_modules", vec!["package.json"]),
        single_target_rule("rust", "target", vec!["Cargo.toml"]),
        single_target_rule(
            "vendor",
            "vendor",
            vec!["composer.json", "Gemfile", "go.mod"],
        ),
        Rule::new(
            "python-venv",
            vec![RuleCase::new(
                vec![Target::directory(".venv"), Target::directory("venv")],
                vec![Requirement::any_of(vec![
                    Evidence::candidate_parent("pyproject.toml", FILE),
                    Evidence::candidate_parent("requirements.txt", FILE),
                ])],
            )],
        ),
        single_target_rule("tox", ".tox", vec!["tox.ini"]),
        single_target_rule("nox", ".nox", vec!["noxfile.py"]),
        single_target_rule("parcel", ".parcel-cache", vec!["package.json"]),
        single_target_rule("terragrunt", ".terragrunt-cache", vec!["terragrunt.hcl"]),
        single_target_rule("cdk", "cdk.out", vec!["cdk.json"]),
    ]
}

fn single_target_rule(id: &str, target: &str, parent_evidence: Vec<&str>) -> Rule {
    Rule::new(
        id,
        vec![RuleCase::new(
            vec![Target::directory(target)],
            vec![Requirement::any_of(
                parent_evidence
                    .into_iter()
                    .map(|path| Evidence::candidate_parent(path, FILE))
                    .collect(),
            )],
        )],
    )
}
