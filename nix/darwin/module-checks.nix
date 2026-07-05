{
  inputs,
  lib,
  pkgs,
  self,
  self',
}:
let
  evalDarwin = moduleConfig:
    inputs.nix-darwin.lib.darwinSystem {
      system = pkgs.system;
      modules = [
        self.darwinModules.tmignore
        {
          nixpkgs.pkgs = pkgs;
          system.stateVersion = 6;
          services.tmignore = moduleConfig;
        }
      ];
    };

  evalSystem = moduleConfig: (evalDarwin moduleConfig).config.system.build.toplevel;

  enabledDryRun = evalDarwin {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "/Users/alice/Developer" ];
    scan.skipPaths = [ "/Users/alice/Developer/archive" ];
    scan.disabledBuiltinRules = [ "node.parcel-cache" ];
    stdoutPath = "/tmp/tmignore.log";
    stderrPath = "/tmp/tmignore.error.log";
  };

  disabled = evalDarwin {
    enable = false;
  };

  enabledApply = evalDarwin {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "/Users/alice/Developer" ];
    mode = "apply";
    schedule = [
      {
        Hour = 9;
        Minute = 0;
      }
      {
        Hour = 17;
        Minute = 0;
      }
    ];
    runAtLoad = true;
  };

  withExtraRules = evalDarwin {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "/Users/alice/Developer" ];
    scan.builtinRules = "none";
    scan.extraRules.pnpm_store = {
      cases = [
        {
          targets = [
            {
              path = ".pnpm-store";
              kind = "directory";
            }
          ];
          requirements = [
            {
              any_of = [
                {
                  path = "package.json";
                  kind = "file";
                  base = "candidate_parent";
                }
              ];
            }
          ];
        }
      ];
    };
    global.builtinRules = "none";
    global.extraRules.custom_cache.path = ".cargo/registry/custom";
  };

  scanOnly = evalDarwin {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "/Users/alice/Developer" ];
    global.enable = false;
  };

  invalidSchedule = builtins.tryEval (
    (evalDarwin {
      enable = true;
      package = self'.packages.tmignore;
      scan.roots = [ "/Users/alice/Developer" ];
      schedule = {
        Hour = 3;
        Minute = 30;
      };
    }).config.launchd.user.agents.tmignore.serviceConfig.StartCalendarInterval
  );

  missingRoots = builtins.tryEval (evalSystem {
    enable = true;
    package = self'.packages.tmignore;
  });

  relativeRoot = builtins.tryEval (evalSystem {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "Developer" ];
  });

  tildeRoot = builtins.tryEval (evalSystem {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "~/Developer" ];
  });

  relativeSkipPath = builtins.tryEval (evalSystem {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "/Users/alice/Developer" ];
    scan.skipPaths = [ "archive" ];
  });

  tildeSkipPath = builtins.tryEval (evalSystem {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "/Users/alice/Developer" ];
    scan.skipPaths = [ "~/Developer/archive" ];
  });

  invalidGlobalExtraPath = builtins.tryEval (evalSystem {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "/Users/alice/Developer" ];
    global.extraRules.bad.path = "Documents/project";
  });

  absoluteGlobalExtraPath = builtins.tryEval (evalSystem {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "/Users/alice/Developer" ];
    global.extraRules.bad.path = "/Users/alice/.cargo/registry";
  });

  tildeGlobalExtraPath = builtins.tryEval (evalSystem {
    enable = true;
    package = self'.packages.tmignore;
    scan.roots = [ "/Users/alice/Developer" ];
    global.extraRules.bad.path = "~/.cargo/registry";
  });

  dryRunAgent = enabledDryRun.config.launchd.user.agents.tmignore.serviceConfig;
  applyAgent = enabledApply.config.launchd.user.agents.tmignore.serviceConfig;
  extraRulesAgent = withExtraRules.config.launchd.user.agents.tmignore.serviceConfig;
  scanOnlyAgent = scanOnly.config.launchd.user.agents.tmignore.serviceConfig;

  dryRunConfig = builtins.elemAt dryRunAgent.ProgramArguments 3;
  applyConfig = builtins.elemAt applyAgent.ProgramArguments 3;
  extraRulesConfig = builtins.elemAt extraRulesAgent.ProgramArguments 3;
in
{
  tmignore-darwin-module = pkgs.runCommand "tmignore-darwin-module-check" { } ''
    set -eu

    test "${if disabled.config.launchd.user.agents ? tmignore then "true" else "false"}" = "false"
    test "${if invalidSchedule.success then "true" else "false"}" = "false"
    test "${if missingRoots.success then "true" else "false"}" = "false"
    test "${if relativeRoot.success then "true" else "false"}" = "false"
    test "${if tildeRoot.success then "true" else "false"}" = "false"
    test "${if relativeSkipPath.success then "true" else "false"}" = "false"
    test "${if tildeSkipPath.success then "true" else "false"}" = "false"
    test "${if invalidGlobalExtraPath.success then "true" else "false"}" = "false"
    test "${if absoluteGlobalExtraPath.success then "true" else "false"}" = "false"
    test "${if tildeGlobalExtraPath.success then "true" else "false"}" = "false"

    test "${builtins.elemAt dryRunAgent.ProgramArguments 0}" = "${lib.getExe self'.packages.tmignore}"
    test "${builtins.elemAt dryRunAgent.ProgramArguments 1}" = "all"
    test "${builtins.elemAt dryRunAgent.ProgramArguments 2}" = "--config"
    test "${dryRunConfig}" = "${builtins.elemAt dryRunAgent.ProgramArguments 3}"
    test "${toString (builtins.length dryRunAgent.ProgramArguments)}" = "4"
    test "${toString ((builtins.elemAt dryRunAgent.StartCalendarInterval 0).Hour)}" = "3"
    test "${toString ((builtins.elemAt dryRunAgent.StartCalendarInterval 0).Minute)}" = "30"
    test "${toString (builtins.length dryRunAgent.StartCalendarInterval)}" = "1"
    test "${if dryRunAgent.RunAtLoad then "true" else "false"}" = "false"
    test "${dryRunAgent.StandardOutPath}" = "/tmp/tmignore.log"
    test "${dryRunAgent.StandardErrorPath}" = "/tmp/tmignore.error.log"

    test "${builtins.elemAt applyAgent.ProgramArguments 0}" = "${lib.getExe self'.packages.tmignore}"
    test "${builtins.elemAt applyAgent.ProgramArguments 1}" = "all"
    test "${builtins.elemAt applyAgent.ProgramArguments 2}" = "--config"
    test "${applyConfig}" = "${builtins.elemAt applyAgent.ProgramArguments 3}"
    test "${builtins.elemAt applyAgent.ProgramArguments 4}" = "--apply"
    test "${toString (builtins.length applyAgent.ProgramArguments)}" = "5"
    test "${toString ((builtins.elemAt applyAgent.StartCalendarInterval 0).Hour)}" = "9"
    test "${toString ((builtins.elemAt applyAgent.StartCalendarInterval 0).Minute)}" = "0"
    test "${toString ((builtins.elemAt applyAgent.StartCalendarInterval 1).Hour)}" = "17"
    test "${toString ((builtins.elemAt applyAgent.StartCalendarInterval 1).Minute)}" = "0"
    test "${if applyAgent.RunAtLoad then "true" else "false"}" = "true"

    test "${builtins.elemAt scanOnlyAgent.ProgramArguments 1}" = "scan"

    grep -q '\[scan\]' '${dryRunConfig}'
    grep -q 'roots = \["/Users/alice/Developer"\]' '${dryRunConfig}'
    grep -q 'skip_paths = \["/Users/alice/Developer/archive"\]' '${dryRunConfig}'
    grep -q 'builtin_rules = "defaults"' '${dryRunConfig}'
    grep -q 'disabled_builtin_rules = \["node.parcel-cache"\]' '${dryRunConfig}'
    grep -q '\[global\]' '${dryRunConfig}'

    grep -q 'builtin_rules = "none"' '${extraRulesConfig}'
    grep -q '\[\[scan.extra_rules.pnpm_store.cases\]\]' '${extraRulesConfig}'
    grep -q '\[\[scan.extra_rules.pnpm_store.cases.targets\]\]' '${extraRulesConfig}'
    grep -q '\[\[scan.extra_rules.pnpm_store.cases.requirements.any_of\]\]' '${extraRulesConfig}'
    grep -q 'path = ".pnpm-store"' '${extraRulesConfig}'
    grep -q 'base = "candidate_parent"' '${extraRulesConfig}'
    grep -q '\[global.extra_rules.custom_cache\]' '${extraRulesConfig}'
    grep -q 'path = ".cargo/registry/custom"' '${extraRulesConfig}'

    touch $out
  '';
}
