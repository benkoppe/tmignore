use camino::{Utf8Path, Utf8PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub id: &'static str,
    pub cases: &'static [RuleCase],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleCase {
    pub targets: &'static [Target],
    pub requirements: &'static [Requirement],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub path: &'static str,
    pub kind: TargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requirement {
    pub any_of: &'static [Evidence],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evidence {
    pub path: &'static str,
    pub kind: EvidenceKind,
    pub base: EvidenceBase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    File,
    Directory,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceBase {
    Candidate,
    CandidateParent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedRule {
    pub rule_id: &'static str,
    pub target: Target,
    pub evidence: Vec<MatchedRuleEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedRuleEvidence {
    pub evidence: Evidence,
    pub path: Utf8PathBuf,
}

impl Evidence {
    pub const fn candidate(path: &'static str, kind: EvidenceKind) -> Self {
        Self {
            path,
            kind,
            base: EvidenceBase::Candidate,
        }
    }

    pub const fn candidate_parent(path: &'static str, kind: EvidenceKind) -> Self {
        Self {
            path,
            kind,
            base: EvidenceBase::CandidateParent,
        }
    }

    pub fn resolve_against(self, candidate_path: &Utf8Path) -> Option<Utf8PathBuf> {
        match self.base {
            EvidenceBase::Candidate => Some(candidate_path.join(self.path)),
            EvidenceBase::CandidateParent => {
                candidate_path.parent().map(|parent| parent.join(self.path))
            }
        }
    }
}

const FILE: EvidenceKind = EvidenceKind::File;

pub const DEFAULT_RULES: &[Rule] = &[
    Rule {
        id: "node",
        cases: &[RuleCase {
            targets: &[Target {
                path: "node_modules",
                kind: TargetKind::Directory,
            }],
            requirements: &[Requirement {
                any_of: &[Evidence::candidate_parent("package.json", FILE)],
            }],
        }],
    },
    Rule {
        id: "rust",
        cases: &[RuleCase {
            targets: &[Target {
                path: "target",
                kind: TargetKind::Directory,
            }],
            requirements: &[Requirement {
                any_of: &[Evidence::candidate_parent("Cargo.toml", FILE)],
            }],
        }],
    },
    Rule {
        id: "vendor",
        cases: &[RuleCase {
            targets: &[Target {
                path: "vendor",
                kind: TargetKind::Directory,
            }],
            requirements: &[Requirement {
                any_of: &[
                    Evidence::candidate_parent("composer.json", FILE),
                    Evidence::candidate_parent("Gemfile", FILE),
                    Evidence::candidate_parent("go.mod", FILE),
                ],
            }],
        }],
    },
    Rule {
        id: "python-venv",
        cases: &[RuleCase {
            targets: &[
                Target {
                    path: ".venv",
                    kind: TargetKind::Directory,
                },
                Target {
                    path: "venv",
                    kind: TargetKind::Directory,
                },
            ],
            requirements: &[Requirement {
                any_of: &[
                    Evidence::candidate_parent("pyproject.toml", FILE),
                    Evidence::candidate_parent("requirements.txt", FILE),
                ],
            }],
        }],
    },
    Rule {
        id: "tox",
        cases: &[RuleCase {
            targets: &[Target {
                path: ".tox",
                kind: TargetKind::Directory,
            }],
            requirements: &[Requirement {
                any_of: &[Evidence::candidate_parent("tox.ini", FILE)],
            }],
        }],
    },
    Rule {
        id: "nox",
        cases: &[RuleCase {
            targets: &[Target {
                path: ".nox",
                kind: TargetKind::Directory,
            }],
            requirements: &[Requirement {
                any_of: &[Evidence::candidate_parent("noxfile.py", FILE)],
            }],
        }],
    },
    Rule {
        id: "parcel",
        cases: &[RuleCase {
            targets: &[Target {
                path: ".parcel-cache",
                kind: TargetKind::Directory,
            }],
            requirements: &[Requirement {
                any_of: &[Evidence::candidate_parent("package.json", FILE)],
            }],
        }],
    },
    Rule {
        id: "terragrunt",
        cases: &[RuleCase {
            targets: &[Target {
                path: ".terragrunt-cache",
                kind: TargetKind::Directory,
            }],
            requirements: &[Requirement {
                any_of: &[Evidence::candidate_parent("terragrunt.hcl", FILE)],
            }],
        }],
    },
    Rule {
        id: "cdk",
        cases: &[RuleCase {
            targets: &[Target {
                path: "cdk.out",
                kind: TargetKind::Directory,
            }],
            requirements: &[Requirement {
                any_of: &[Evidence::candidate_parent("cdk.json", FILE)],
            }],
        }],
    },
];
