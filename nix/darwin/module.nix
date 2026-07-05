{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.tmignore;
  scanCfg = cfg.scan;
  globalCfg = cfg.global;
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
          "target_parent"
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

  tmignoreConfig = toml.generate "tmignore.toml" {
    scan = {
      roots = scanCfg.roots;
      skip_paths = scanCfg.skipPaths;
      builtin_rules = scanCfg.builtinRules;
      disabled_builtin_rules = scanCfg.disabledBuiltinRules;
      extra_rules = scanCfg.extraRules;
    };
    global = {
      builtin_rules = globalCfg.builtinRules;
      disabled_builtin_rules = globalCfg.disabledBuiltinRules;
      extra_targets = globalCfg.extraTargets;
    };
  };

  programArguments = [
    (lib.getExe cfg.package)
    (if globalCfg.enable then "all" else "scan")
    "--config"
    "${tmignoreConfig}"
  ] ++ lib.optional (cfg.mode == "apply") "--apply";

  isAbsolutePath = path: lib.hasPrefix "/" path;
  hasPathComponent = component: path: lib.elem component (lib.splitString "/" path);
  exactOrChildPath = prefix: path: path == prefix || lib.hasPrefix "${prefix}/" path;
  allowedGlobalCachePrefixes = [
    ".cargo/registry"
    ".cargo/git"
    ".rustup/toolchains"
    "go/pkg/mod"
    ".gradle/caches"
    ".m2/repository"
    ".npm/_cacache"
    "Library/pnpm/store"
    ".bun/install/cache"
    ".composer/cache"
    ".ivy2/cache"
    ".cocoapods/repos"
    ".vagrant.d/boxes"
    ".terraform.d/plugin-cache"
    "Library/Developer/Xcode/DerivedData"
    ".ollama/models"
    ".config/lima/_disks"
    ".config/lima/colima"
    "Virtual Machines.localized"
    "Virtual Machines"
  ];
  isAllowedGlobalExtraPath = path:
    path != ""
    && path != "."
    && !(isAbsolutePath path)
    && !(lib.hasPrefix "~" path)
    && !(hasPathComponent ".." path)
    && !(hasPathComponent "." path)
    && lib.any (prefix: exactOrChildPath prefix path) allowedGlobalCachePrefixes;
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

    mode = lib.mkOption {
      type = lib.types.enum [
        "dry-run"
        "apply"
      ];
      default = "dry-run";
      description = "Whether scheduled runs only report matches or apply Time Machine exclusions.";
    };

    scan = {
      roots = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        example = [ "/Users/alice/Developer" ];
        description = "Absolute filesystem roots to scan. These must be set explicitly.";
      };

      skipPaths = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        example = [ "/Users/alice/Developer/archive" ];
        description = "Absolute paths to skip while scanning.";
      };

      builtinRules = lib.mkOption {
        type = lib.types.enum [
          "defaults"
          "none"
        ];
        default = "defaults";
        description = "Builtin project scan rule set policy written to tmignore's generated TOML config.";
      };

      disabledBuiltinRules = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        example = [ "node.parcel-cache" ];
        description = "Built-in scan rule IDs to disable while keeping the rest of the default catalog enabled.";
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
        description = "Named extra project dependency/cache rules written under scan.extra_rules in tmignore's TOML config.";
      };
    };

      global = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether the scheduled launchd job should also process global dependency/cache directories.";
      };

      builtinRules = lib.mkOption {
        type = lib.types.enum [
          "defaults"
          "none"
        ];
        default = "defaults";
        description = "Builtin global cache rule set policy written to tmignore's generated TOML config.";
      };

      disabledBuiltinRules = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        example = [ "ollama.models" ];
        description = "Built-in global rule IDs to disable while keeping the rest of the default catalog enabled.";
      };

      extraTargets = lib.mkOption {
        type = lib.types.attrsOf (lib.types.submodule {
          options.path = lib.mkOption {
            type = lib.types.str;
            description = "Home-relative global cache directory path under a known cache namespace.";
          };
        });
        default = { };
        example = lib.literalExpression ''
          {
            custom_cache.path = ".cargo/registry/custom";
          }
        '';
        description = "Named extra global cache targets written under global.extra_targets in tmignore's TOML config.";
      };
    };

    schedule = lib.mkOption {
      type = lib.types.listOf calendarIntervalType;
      default = [
        {
          Hour = 3;
          Minute = 30;
        }
      ];
      example = lib.literalExpression ''
        [
          { Hour = 9; Minute = 0; }
          { Hour = 17; Minute = 0; }
        ]
      '';
      description = "launchd StartCalendarInterval values for scheduled tmignore runs.";
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
        assertion = scanCfg.roots != [ ];
        message = "services.tmignore.scan.roots must contain at least one explicit scan root.";
      }
      {
        assertion = lib.all isAbsolutePath scanCfg.roots;
        message = "services.tmignore.scan.roots must contain absolute paths; use config.users.users.<name>.home instead of `~`.";
      }
      {
        assertion = lib.all isAbsolutePath scanCfg.skipPaths;
        message = "services.tmignore.scan.skipPaths must contain absolute paths; use config.users.users.<name>.home instead of `~`.";
      }
      {
        assertion = !globalCfg.enable || lib.all isAllowedGlobalExtraPath (lib.mapAttrsToList (_: target: target.path) globalCfg.extraTargets);
        message = "services.tmignore.global.extraTargets paths must be home-relative paths under a known cache namespace.";
      }
    ];

    launchd.user.agents.tmignore.serviceConfig = {
      Label = "com.github.benkoppe.tmignore";
      ProgramArguments = programArguments;
      StartCalendarInterval = cfg.schedule;
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
