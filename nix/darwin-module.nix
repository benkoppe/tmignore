{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.tmignore;
  toml = pkgs.formats.toml { };

  calendarIntervalType = lib.types.submodule {
    options = {
      Minute = lib.mkOption {
        type = lib.types.ints.between 0 59;
        description = "Minute at which to run tmignore.";
      };

      Hour = lib.mkOption {
        type = lib.types.ints.between 0 23;
        description = "Hour at which to run tmignore.";
      };

      Day = lib.mkOption {
        type = lib.types.nullOr (lib.types.ints.between 1 31);
        default = null;
        description = "Optional day of the month at which to run tmignore.";
      };

      Weekday = lib.mkOption {
        type = lib.types.nullOr (lib.types.ints.between 0 7);
        default = null;
        description = "Optional weekday at which to run tmignore, using launchd numbering.";
      };

      Month = lib.mkOption {
        type = lib.types.nullOr (lib.types.ints.between 1 12);
        default = null;
        description = "Optional month at which to run tmignore.";
      };
    };
  };

  targetType = lib.types.submodule {
    options = {
      path = lib.mkOption {
        type = lib.types.str;
        description = "Relative target directory path matched by this rule case.";
      };

      kind = lib.mkOption {
        type = lib.types.enum [ "directory" ];
        default = "directory";
        description = "Target kind. Only directory targets are currently supported.";
      };
    };
  };

  evidenceType = lib.types.submodule {
    options = {
      path = lib.mkOption {
        type = lib.types.str;
        description = "Relative evidence path required by this rule.";
      };

      kind = lib.mkOption {
        type = lib.types.enum [
          "file"
          "directory"
          "any"
        ];
        description = "Evidence path kind.";
      };

      base = lib.mkOption {
        type = lib.types.enum [
          "candidate"
          "candidate_parent"
        ];
        description = "Base path used to resolve this evidence path.";
      };
    };
  };

  requirementType = lib.types.submodule {
    options.any_of = lib.mkOption {
      type = lib.types.listOf evidenceType;
      description = "Evidence alternatives. Any one entry satisfies this requirement.";
    };
  };

  ruleCaseType = lib.types.submodule {
    options = {
      targets = lib.mkOption {
        type = lib.types.listOf targetType;
        description = "Target directories matched by this rule case.";
      };

      requirements = lib.mkOption {
        type = lib.types.listOf requirementType;
        description = "Requirements that must all be satisfied by this rule case.";
      };
    };
  };

  ruleType = lib.types.submodule {
    options.cases = lib.mkOption {
      type = lib.types.listOf ruleCaseType;
      description = "Rule cases. A rule matches when any case matches.";
    };
  };

  scheduleType = lib.types.either calendarIntervalType (lib.types.listOf calendarIntervalType);

  scheduleEntries = if builtins.isList cfg.schedule then cfg.schedule else [ cfg.schedule ];
  normalizedSchedule = map (entry: lib.filterAttrs (_: value: value != null) entry) scheduleEntries;

  tmignoreConfig = toml.generate "tmignore.toml" {
    roots = cfg.roots;
    skip_paths = cfg.skipPaths;
    builtin_rules = cfg.builtinRules;
    extra_rules = cfg.extraRules;
  };

  programArguments = [
    (lib.getExe cfg.package)
    "--config"
    "${tmignoreConfig}"
  ] ++ lib.optional (cfg.mode == "apply") "--apply";
in
{
  options.services.tmignore = {
    enable = lib.mkEnableOption "tmignore Time Machine exclusion maintenance";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.tmignore;
      defaultText = lib.literalExpression "inputs.tmignore.packages.\${pkgs.system}.tmignore";
      description = "tmignore package to run from launchd.";
    };

    roots = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "~/Developer" ];
      description = "Filesystem roots to scan. These must be set explicitly.";
    };

    skipPaths = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "~/Developer/archive" ];
      description = "Paths to skip while scanning.";
    };

    mode = lib.mkOption {
      type = lib.types.enum [
        "dry-run"
        "apply"
      ];
      default = "dry-run";
      description = "Whether scheduled runs only report matches or apply Time Machine exclusions.";
    };

    builtinRules = lib.mkOption {
      type = lib.types.enum [
        "defaults"
        "none"
      ];
      default = "defaults";
      description = "Builtin rule set policy written to tmignore's generated TOML config.";
    };

    extraRules = lib.mkOption {
      type = lib.types.attrsOf ruleType;
      default = { };
      example = lib.literalExpression ''
        {
          pnpm_store = {
            cases = [
              {
                targets = [ { path = ".pnpm-store"; kind = "directory"; } ];
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
        }
      '';
      description = "Named extra dependency/cache rules written under extra_rules in tmignore's TOML config.";
    };

    schedule = lib.mkOption {
      type = scheduleType;
      default = {
        Hour = 3;
        Minute = 30;
      };
      example = lib.literalExpression ''
        [
          { Hour = 9; Minute = 0; }
          { Hour = 17; Minute = 0; }
        ]
      '';
      description = "launchd StartCalendarInterval value or values for scheduled tmignore runs.";
    };

    runAtLoad = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether launchd should run tmignore when the user agent loads.";
    };

    stdoutPath = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      example = "/Users/alice/Library/Logs/tmignore.log";
      description = "Optional StandardOutPath for the launchd agent.";
    };

    stderrPath = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      example = "/Users/alice/Library/Logs/tmignore.error.log";
      description = "Optional StandardErrorPath for the launchd agent.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.roots != [ ];
        message = "services.tmignore.roots must contain at least one explicit scan root.";
      }
    ];

    launchd.user.agents.tmignore.serviceConfig = {
      Label = "com.github.benkoppe.tmignore";
      ProgramArguments = programArguments;
      StartCalendarInterval = normalizedSchedule;
      RunAtLoad = cfg.runAtLoad;
    }
    // lib.optionalAttrs (cfg.stdoutPath != null) {
      StandardOutPath = cfg.stdoutPath;
    }
    // lib.optionalAttrs (cfg.stderrPath != null) {
      StandardErrorPath = cfg.stderrPath;
    };
  };
}
