use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn dry_run_reports_matched_dependency_directory() {
    let fixture = Fixture::new();
    fixture.dir("project/node_modules");
    fixture.file("project/package.json");

    let mut command = Command::cargo_bin("tmignore").unwrap();

    command
        .args(["--root", fixture.root(), "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Would exclude:")
                .and(predicate::str::contains("node_modules"))
                .and(predicate::str::contains("rule: node"))
                .and(predicate::str::contains("target: node_modules"))
                .and(predicate::str::contains("package.json"))
                .and(predicate::str::contains("Summary:")),
        );
}

#[cfg(unix)]
#[test]
fn dry_run_groups_skipped_paths_by_default() {
    let fixture = Fixture::new();
    fixture.dir("real/first");
    fixture.dir("real/second");
    fixture.dir(".direnv");
    fixture.symlink("real/first", ".direnv/first");
    fixture.symlink("real/second", ".direnv/second");

    let mut command = Command::cargo_bin("tmignore").unwrap();

    command
        .args(["--root", fixture.root(), "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains(".direnv  2 symlinks")
                .and(predicate::str::contains(".direnv/first  symlink").not())
                .and(predicate::str::contains(".direnv/second  symlink").not()),
        );
}

#[cfg(unix)]
#[test]
fn dry_run_verbose_lists_every_skipped_path() {
    let fixture = Fixture::new();
    fixture.dir("real/first");
    fixture.dir("real/second");
    fixture.dir(".direnv");
    fixture.symlink("real/first", ".direnv/first");
    fixture.symlink("real/second", ".direnv/second");

    let mut command = Command::cargo_bin("tmignore").unwrap();

    command
        .args(["--root", fixture.root(), "--dry-run", "-v"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains(".direnv/first  symlink")
                .and(predicate::str::contains(".direnv/second  symlink"))
                .and(predicate::str::contains(".direnv  2 symlinks").not()),
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

    fn root(&self) -> &str {
        self.temp_dir.path().to_str().unwrap()
    }

    fn path(&self, path: &str) -> std::path::PathBuf {
        self.temp_dir.path().join(path)
    }

    fn dir(&self, path: &str) {
        fs_err::create_dir_all(self.path(path)).unwrap();
    }

    fn file(&self, path: &str) {
        let path = self.path(path);
        fs_err::create_dir_all(path.parent().unwrap()).unwrap();
        fs_err::write(path, b"").unwrap();
    }

    #[cfg(unix)]
    fn symlink(&self, original: &str, link: &str) {
        std::os::unix::fs::symlink(self.path(original), self.path(link)).unwrap();
    }
}
