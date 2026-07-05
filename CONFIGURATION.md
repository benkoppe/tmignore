# Configuration

This is the reference for the nix-darwin module and the TOML config file consumed by `tmignore --config`.

Paths are passed directly to `tmignore`; they are not interpreted by a shell. Do not use `~` expecting home-directory expansion. Prefer absolute scan paths for scheduled nix-darwin runs.

## Commands

`tmignore scan` walks configured project roots and applies project dependency/cache rules.

`tmignore global` checks configured home/global cache paths without walking filesystem roots.

`tmignore all` runs `scan` and then `global` in one process and report. The nix-darwin module uses `all` by default and switches to `scan` when `services.tmignore.global.enable = false`.

All commands default to dry-run mode. Use `--apply` to change Time Machine exclusions.

## nix-darwin Module

```nix
{ config, ... }:
let
  home = config.users.users.alice.home;
in
{
  services.tmignore = {
    enable = true;
    scan.roots = [ "${home}/Developer" ];
  };
}
```

Shared options:

| Option | Default | Description |
| --- | --- | --- |
| `services.tmignore.enable` | `false` | Enables the user launchd agent. |
| `services.tmignore.package` | flake package | Package used for the scheduled binary. |
| `services.tmignore.mode` | `"dry-run"` | `"dry-run"` or `"apply"`; apply adds `--apply` to launchd `ProgramArguments`. |
| `services.tmignore.schedule` | `[ { Hour = 3; Minute = 30; } ]` | launchd `StartCalendarInterval` values. |
| `services.tmignore.runAtLoad` | `false` | Sets launchd `RunAtLoad`. |
| `services.tmignore.stdoutPath` | `null` | Optional launchd `StandardOutPath`. |
| `services.tmignore.stderrPath` | `null` | Optional launchd `StandardErrorPath`. |

Scan options:

| Option | Default | Description |
| --- | --- | --- |
| `services.tmignore.scan.roots` | `[]` | Absolute filesystem roots to scan. Required when the service is enabled. |
| `services.tmignore.scan.skipPaths` | `[]` | Absolute paths to skip while scanning. |
| `services.tmignore.scan.builtinRules` | `"defaults"` | Built-in project rule policy: `"defaults"` or `"none"`. |
| `services.tmignore.scan.disabledBuiltinRules` | `[]` | Built-in project rule IDs to disable. |
| `services.tmignore.scan.extraRules` | `{}` | Extra structured project scan rules. |

Global options:

| Option | Default | Description |
| --- | --- | --- |
| `services.tmignore.global.enable` | `true` | Whether the scheduled job should also process global dependency/cache directories. |
| `services.tmignore.global.builtinRules` | `"defaults"` | Built-in global rule policy: `"defaults"` or `"none"`. |
| `services.tmignore.global.disabledBuiltinRules` | `[]` | Built-in global rule IDs to disable. |
| `services.tmignore.global.extraRules` | `{}` | Extra named global cache paths under known cache namespaces. Paths resolve against the user's home directory. |

Full example:

```nix
{ config, ... }:
let
  home = config.users.users.alice.home;
in
{
  services.tmignore = {
    enable = true;
    mode = "apply";
    scan.roots = [ "${home}/Developer" ];
    scan.skipPaths = [ "${home}/Developer/archive" ];
    scan.disabledBuiltinRules = [ "node.parcel-cache" ];
    global.disabledBuiltinRules = [ "ollama.models" ];
    global.extraRules.custom_cache.path = ".cargo/registry/custom";
    stdoutPath = "${home}/Library/Logs/tmignore.log";
    stderrPath = "${home}/Library/Logs/tmignore.error.log";
  };
}
```

## TOML Config

The TOML config has separate `[scan]` and `[global]` sections. Unknown fields are rejected.

```toml
[scan]
roots = ["/Users/alice/Developer"]
skip_paths = ["/Users/alice/Developer/archive"]
builtin_rules = "defaults"
disabled_builtin_rules = ["node.parcel-cache"]

[[scan.extra_rules.pnpm_store.cases]]

[[scan.extra_rules.pnpm_store.cases.targets]]
path = ".pnpm-store"
kind = "directory"

[[scan.extra_rules.pnpm_store.cases.requirements]]

[[scan.extra_rules.pnpm_store.cases.requirements.any_of]]
path = "package.json"
kind = "file"
base = "candidate_parent"

[global]
builtin_rules = "defaults"
disabled_builtin_rules = ["ollama.models"]

[global.extra_rules.custom_cache]
path = ".cargo/registry/custom"
```

Relative `scan.roots` and `scan.skip_paths` are resolved against the process current directory by the CLI. The nix-darwin module rejects relative scan paths. Global paths must be relative to the user's home directory and under a known cache namespace; `~` is not expanded.

## Built-In Scan Rules

| Rule ID | Target | Evidence |
| --- | --- | --- |
| `node.node-modules` | `node_modules` | `package.json` |
| `node.parcel-cache` | `.parcel-cache` | `package.json` |
| `rust.cargo-target` | `target` | `Cargo.toml` |
| `php.composer-vendor` | `vendor` | `composer.json` |
| `go.vendor` | `vendor` | `go.mod` |
| `ruby.bundle-vendor` | `vendor/bundle` | `Gemfile` |
| `python.venv` | `.venv`, `venv` | `pyproject.toml`, `requirements.txt` |
| `python.tox` | `.tox` | `tox.ini` |
| `python.nox` | `.nox` | `noxfile.py` |
| `swift.build` | `.build` | `Package.swift` |
| `elixir.deps` | `deps` | `mix.exs` |
| `elixir.build` | `_build` | `mix.exs` |
| `gradle.cache` | `.gradle` | Gradle build/settings files |
| `gradle.build` | `build` | Gradle build files |
| `dart.tool` | `.dart_tool` | `pubspec.yaml` |
| `dart.build` | `build` | `pubspec.yaml` |
| `haskell.stack-work` | `.stack-work` | `stack.yaml` |
| `vagrant.state` | `.vagrant` | `Vagrantfile` |
| `ios.carthage` | `Carthage` | `Cartfile` |
| `ios.cocoapods` | `Pods` | `Podfile` |
| `terragrunt.cache` | `.terragrunt-cache` | `terragrunt.hcl` |
| `aws-cdk.out` | `cdk.out` | `cdk.json` |
| `java.maven-target` | `target` | `pom.xml` |
| `scala.sbt-target` | `target` | `build.sbt`, `project/plugins.sbt` |

## Built-In Global Rules

| Rule ID | Home-relative path |
| --- | --- |
| `cargo.registry` | `.cargo/registry` |
| `cargo.git` | `.cargo/git` |
| `rustup.toolchains` | `.rustup/toolchains` |
| `go.module-cache` | `go/pkg/mod` |
| `gradle.caches` | `.gradle/caches` |
| `maven.repository` | `.m2/repository` |
| `npm.cache` | `.npm/_cacache` |
| `pnpm.store` | `Library/pnpm/store` |
| `bun.install-cache` | `.bun/install/cache` |
| `composer.cache` | `.composer/cache` |
| `ivy.cache` | `.ivy2/cache` |
| `cocoapods.repos` | `.cocoapods/repos` |
| `vagrant.boxes` | `.vagrant.d/boxes` |
| `terraform.plugin-cache` | `.terraform.d/plugin-cache` |
| `xcode.derived-data` | `Library/Developer/Xcode/DerivedData` |
| `ollama.models` | `.ollama/models` |

Global rules deliberately target precise cache directories and selected reinstallable runtime/toolchain installs. `rustup.toolchains` is intentionally included because Rust toolchains are large and reinstallable. Whole home-directory config or cache roots such as `~/.cache`, `~/.terraform.d`, `.vagrant.d`, or `.sbt` are not excluded because they may contain credentials, user-authored configuration, or unrelated tool data. `~/Library/Caches` is not listed because macOS already excludes it from Time Machine by default.

### Custom Global Rule Namespaces

Extra global rules are accepted only when their paths are exactly one of these known cache namespaces or children below them:

| Accepted prefix |
| --- |
| `.cargo/registry` |
| `.cargo/git` |
| `.rustup/toolchains` |
| `go/pkg/mod` |
| `.gradle/caches` |
| `.m2/repository` |
| `.npm/_cacache` |
| `Library/pnpm/store` |
| `.bun/install/cache` |
| `.composer/cache` |
| `.ivy2/cache` |
| `.cocoapods/repos` |
| `.vagrant.d/boxes` |
| `.terraform.d/plugin-cache` |
| `Library/Developer/Xcode/DerivedData` |
| `.ollama/models` |

Arbitrary home subtrees such as `Documents/project`, `Desktop/cache`, `Library/Application Support`, `.config/app`, or absolute paths are rejected.

## Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | The command completed without failures. |
| `1` | The command completed, but one or more per-path operations failed, such as an unreadable directory or a `tmutil` error. |
| `2` | A global precondition failed and no run was performed, such as invalid configuration, missing scan roots, missing home directory, or invalid CLI usage. |
