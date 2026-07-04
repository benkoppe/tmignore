{ inputs, ... }:
{
  perSystem =
    {
      pkgs,
      lib,
      self',
      ...
    }:
    let
      craneLib = inputs.crane.mkLib pkgs;
      src = craneLib.cleanCargoSource ../.;

      commonBuildArgs = {
        inherit src;
        strictDeps = true;

        buildInputs =
          # [ ] ++
          lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];
      };

      cargoArtifacts = craneLib.buildDepsOnly commonBuildArgs;

      tmignore = craneLib.buildPackage (
        commonBuildArgs
        // {
          inherit cargoArtifacts;
          pname = "tmignore";
        }
      );
    in
    {
      checks = {
        tmignore-clippy = craneLib.cargoClippy (
          commonBuildArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          }
        );

        tmignore-doc = craneLib.cargoDoc (
          commonBuildArgs
          // {
            inherit cargoArtifacts;
            env.RUSTDOCFLAGS = "--deny warnings";
          }
        );

        tmignore-fmt = craneLib.cargoFmt {
          inherit src;
        };

        tmignore-toml-fmt = craneLib.taploFmt {
          src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
        };

        tmignore-audit = craneLib.cargoAudit {
          inherit src;
          inherit (inputs) advisory-db;
        };

        tmignore-deny = craneLib.cargoDeny {
          inherit src;
        };

        tmignore-nextest = craneLib.cargoNextest (
          commonBuildArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";
          }
        );
      };

      packages = {
        inherit cargoArtifacts tmignore;

        default = tmignore;
      };

      apps = rec {
        tmignore = {
          type = "app";
          program = lib.getExe self'.packages.tmignore;
        };

        default = tmignore;
      };

      devShells.default = craneLib.devShell {
        # packages = with pkgs; [
        #
        # ];
      };
    };
}
