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

  enabledDryRun = evalDarwin {
    enable = true;
    package = self'.packages.tmignore;
    roots = [ "~/Developer" ];
    skipPaths = [ "~/Developer/archive" ];
    stdoutPath = "/tmp/tmignore.log";
    stderrPath = "/tmp/tmignore.error.log";
  };

  disabled = evalDarwin {
    enable = false;
  };

  enabledApply = evalDarwin {
    enable = true;
    package = self'.packages.tmignore;
    roots = [ "~/Developer" ];
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
    roots = [ "~/Developer" ];
    builtinRules = "none";
    extraRules.pnpm_store = {
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
  };

  dryRunAgent = enabledDryRun.config.launchd.user.agents.tmignore.serviceConfig;
  applyAgent = enabledApply.config.launchd.user.agents.tmignore.serviceConfig;
  extraRulesAgent = withExtraRules.config.launchd.user.agents.tmignore.serviceConfig;

  dryRunConfig = builtins.elemAt dryRunAgent.ProgramArguments 2;
  applyConfig = builtins.elemAt applyAgent.ProgramArguments 2;
  extraRulesConfig = builtins.elemAt extraRulesAgent.ProgramArguments 2;
in
{
  tmignore-darwin-module = pkgs.runCommand "tmignore-darwin-module-check" { } ''
    set -eu

    test "${if disabled.config.launchd.user.agents ? tmignore then "true" else "false"}" = "false"

    test "${builtins.elemAt dryRunAgent.ProgramArguments 0}" = "${lib.getExe self'.packages.tmignore}"
    test "${builtins.elemAt dryRunAgent.ProgramArguments 1}" = "--config"
    test "${dryRunConfig}" = "${builtins.elemAt dryRunAgent.ProgramArguments 2}"
    test "${toString (builtins.length dryRunAgent.ProgramArguments)}" = "3"
    test "${toString ((builtins.elemAt dryRunAgent.StartCalendarInterval 0).Hour)}" = "3"
    test "${toString ((builtins.elemAt dryRunAgent.StartCalendarInterval 0).Minute)}" = "30"
    test "${toString (builtins.length dryRunAgent.StartCalendarInterval)}" = "1"
    test "${if dryRunAgent.RunAtLoad then "true" else "false"}" = "false"
    test "${dryRunAgent.StandardOutPath}" = "/tmp/tmignore.log"
    test "${dryRunAgent.StandardErrorPath}" = "/tmp/tmignore.error.log"

    test "${builtins.elemAt applyAgent.ProgramArguments 0}" = "${lib.getExe self'.packages.tmignore}"
    test "${builtins.elemAt applyAgent.ProgramArguments 1}" = "--config"
    test "${applyConfig}" = "${builtins.elemAt applyAgent.ProgramArguments 2}"
    test "${builtins.elemAt applyAgent.ProgramArguments 3}" = "--apply"
    test "${toString (builtins.length applyAgent.ProgramArguments)}" = "4"
    test "${toString ((builtins.elemAt applyAgent.StartCalendarInterval 0).Hour)}" = "9"
    test "${toString ((builtins.elemAt applyAgent.StartCalendarInterval 0).Minute)}" = "0"
    test "${toString ((builtins.elemAt applyAgent.StartCalendarInterval 1).Hour)}" = "17"
    test "${toString ((builtins.elemAt applyAgent.StartCalendarInterval 1).Minute)}" = "0"
    test "${if applyAgent.RunAtLoad then "true" else "false"}" = "true"

    grep -q 'roots = \["~/Developer"\]' '${dryRunConfig}'
    grep -q 'skip_paths = \["~/Developer/archive"\]' '${dryRunConfig}'
    grep -q 'builtin_rules = "defaults"' '${dryRunConfig}'

    grep -q 'builtin_rules = "none"' '${extraRulesConfig}'
    grep -q '\[\[extra_rules.pnpm_store.cases\]\]' '${extraRulesConfig}'
    grep -q '\[\[extra_rules.pnpm_store.cases.targets\]\]' '${extraRulesConfig}'
    grep -q '\[\[extra_rules.pnpm_store.cases.requirements.any_of\]\]' '${extraRulesConfig}'
    grep -q 'path = ".pnpm-store"' '${extraRulesConfig}'
    grep -q 'base = "candidate_parent"' '${extraRulesConfig}'

    touch $out
  '';
}
