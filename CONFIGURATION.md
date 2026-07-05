# Configuration

This is the full reference for the nix-darwin module and the TOML config file consumed by `tmignore --config`.

Paths are passed directly to `tmignore`; they are not interpreted by a shell. Do not use `~` expecting home-directory expansion. Prefer absolute paths for scheduled nix-darwin runs.

## nix-darwin Module

Import `tmignore.darwinModules.default` and configure `services.tmignore`:

```nix
{ config, ... }:
let
  home = config.users.users.alice.home;
in
{
  services.tmignore = {
    enable = true;
    roots = [ "${home}/Developer" ];
  };
}
```

The module generates a TOML config in the Nix store and schedules a user launchd agent. It does not mutate project files.

### Options

`services.tmignore.enable`

Type: boolean

Default: `false`

Enables the user launchd agent.

`services.tmignore.package`

Type: package

Default: `inputs.tmignore.packages.${pkgs.system}.tmignore`

Package used for the scheduled `tmignore` binary.

`services.tmignore.roots`

Type: list of strings

Default: `[]`

Absolute filesystem roots to scan. This must be non-empty when the service is enabled. Relative paths and `~` are rejected by the module. These values are written to TOML as `roots`.

`services.tmignore.skipPaths`

Type: list of strings

Default: `[]`

Absolute paths to skip while scanning. Relative paths and `~` are rejected by the module. These values are written to TOML as `skip_paths`.

`services.tmignore.mode`

Type: one of `"dry-run"`, `"apply"`

Default: `"dry-run"`

Controls whether the launchd job only reports matches or applies Time Machine exclusions. The generated TOML does not contain this value; `"apply"` adds `--apply` to `ProgramArguments`.

`services.tmignore.builtinRules`

Type: one of `"defaults"`, `"none"`

Default: `"defaults"`

Controls whether built-in dependency/cache rules are enabled. This value is written to TOML as `builtin_rules`.

`services.tmignore.disabledBuiltinRules`

Type: list of strings

Default: `[]`

Built-in rule IDs to disable while keeping the rest of the default catalog enabled. These values are written to TOML as `disabled_builtin_rules`.

`services.tmignore.extraRules`

Type: attribute set of rules

Default: `{}`

Additional named rules. These values are written to TOML as `extra_rules`.

`services.tmignore.schedule`

Type: list of launchd calendar interval attribute sets

Default: `[ { Hour = 3; Minute = 30; } ]`

Schedules launchd runs through `StartCalendarInterval`. A single daily run is still represented as a one-item list.

Each schedule entry supports these fields:

`Minute`: integer from `0` to `59`.

`Hour`: integer from `0` to `23`.

`Day`: optional integer from `1` to `31`.

`Weekday`: optional integer from `0` to `7`, using launchd numbering.

`Month`: optional integer from `1` to `12`.

`services.tmignore.runAtLoad`

Type: boolean

Default: `false`

Sets launchd `RunAtLoad`. This can catch laptops that were asleep or off during a scheduled time.

`services.tmignore.stdoutPath`

Type: null or absolute path

Default: `null`

Optional launchd `StandardOutPath`.

`services.tmignore.stderrPath`

Type: null or absolute path

Default: `null`

Optional launchd `StandardErrorPath`.

### Full Module Example

```nix
{ config, ... }:
let
  home = config.users.users.alice.home;
in
{
  services.tmignore = {
    enable = true;
    roots = [ "${home}/Developer" ];
    skipPaths = [ "${home}/Developer/archive" ];
    mode = "apply";
    builtinRules = "defaults";
    disabledBuiltinRules = [ "node.parcel-cache" ];
    schedule = [
      { Hour = 9; Minute = 0; }
      { Hour = 17; Minute = 0; }
    ];
    runAtLoad = true;
    stdoutPath = "${home}/Library/Logs/tmignore.log";
    stderrPath = "${home}/Library/Logs/tmignore.error.log";

    extraRules.pnpm_store = {
      cases = [
        {
          targets = [
            { path = ".pnpm-store"; kind = "directory"; }
          ];
          requirements = [
            {
              any_of = [
                { path = "package.json"; kind = "file"; base = "candidate_parent"; }
              ];
            }
          ];
        }
      ];
    };
  };
}
```

## TOML Config

The TOML config is loaded with `tmignore --config <path>`. Unknown fields are rejected.

There is no TOML setting for dry-run or apply mode. That is controlled by CLI flags: default dry-run behavior, or `--apply` to change Time Machine exclusions.

### Top-Level Fields

`roots`

Type: array of strings

Default: `[]`

Filesystem roots to scan. At least one root is required after CLI overrides are applied.

`skip_paths`

Type: array of strings

Default: `[]`

Paths to skip while scanning.

`builtin_rules`

Type: `"defaults"` or `"none"`

Default: `"defaults"`

Controls whether built-in rules are enabled.

`disabled_builtin_rules`

Type: array of strings

Default: `[]`

Built-in rule IDs to disable. Unknown IDs are rejected. Built-in rule IDs remain reserved and cannot be reused by `extra_rules`.

`extra_rules`

Type: table of named rules

Default: `{}`

Additional dependency/cache rules.

### Built-In Rules

The `defaults` catalog contains these built-in rules:

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
| `gradle.cache` | `.gradle` | `build.gradle`, `build.gradle.kts`, `settings.gradle`, `settings.gradle.kts` |
| `gradle.build` | `build` | `build.gradle`, `build.gradle.kts` |
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

The catalog deliberately targets per-project dependency/cache directories only. Home-directory or global caches, such as `~/.terraform.d`, are out of scope for the built-in rules and are not yet handled.

### Path Handling

Absolute paths are used as-is.

Relative `roots` and `skip_paths` are resolved against the process current directory.

`~` is not expanded.

After path preparation, duplicate paths are removed and nested roots or skip paths are pruned.

CLI `--root` values replace TOML `roots`. CLI `--skip` values are appended to TOML `skip_paths`.

### Rule Semantics

A rule matches when any of its cases matches.

A case matches when the candidate path matches any target and every requirement is satisfied.

A requirement is satisfied when any one of its `any_of` evidence entries exists.

Multiple requirements are logical AND. Multiple evidence entries inside one requirement are logical OR.

When a dependency/cache directory is matched, the scanner records it and prunes traversal below that directory.

### Rule Schema

`extra_rules.<id>`

Rule IDs must be non-empty and contain only ASCII letters, numbers, `.`, `_`, or `-`. Extra rule IDs must not collide with enabled built-in rule IDs.

`cases`

Type: array of case tables

Required. Must contain at least one case.

`targets`

Type: array of target tables

Required on each case. Must contain at least one target.

Target fields:

`path`: relative path string. It must not be empty, absolute, or contain `..`.

`kind`: currently only `"directory"`.

`requirements`

Type: array of requirement tables

Required on each case. Must contain at least one requirement.

Requirement fields:

`any_of`: array of evidence tables. Must contain at least one evidence entry.

Evidence fields:

`path`: relative path string. It must not be empty, absolute, or contain `..`.

`kind`: one of `"file"`, `"directory"`, or `"any"`.

`base`: one of `"candidate"`, `"candidate_parent"`, or `"target_parent"`.

`candidate` resolves the evidence path inside the candidate target directory. `candidate_parent` resolves it next to the candidate target directory. `target_parent` resolves it next to the configured target path, which is useful for nested targets such as `vendor/bundle` with a project-level `Gemfile`.

### TOML Example

```toml
roots = ["/Users/alice/Developer"]
skip_paths = ["/Users/alice/Developer/archive"]
builtin_rules = "defaults"
disabled_builtin_rules = ["node.parcel-cache"]

[[extra_rules.pnpm_store.cases]]

[[extra_rules.pnpm_store.cases.targets]]
path = ".pnpm-store"
kind = "directory"

[[extra_rules.pnpm_store.cases.requirements]]

[[extra_rules.pnpm_store.cases.requirements.any_of]]
path = "package.json"
kind = "file"
base = "candidate_parent"
```

Equivalent TOML using inline arrays is also valid for the same schema.

## Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | The run completed without failures. |
| `1` | The run completed, but one or more per-path operations failed, such as an unreadable directory or a `tmutil` error. The report lists each failure. |
| `2` | A global precondition failed and no run was performed, such as invalid configuration or missing scan roots. |
