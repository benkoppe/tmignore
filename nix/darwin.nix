{ self, inputs, ... }:
{
  flake.darwinModules = rec {
    tmignore = import ./darwin/module.nix { inherit self; };
    default = tmignore;
  };

  perSystem =
    {
      pkgs,
      lib,
      self',
      ...
    }:
    {
      checks = lib.optionalAttrs pkgs.stdenv.isDarwin (
        import ./darwin/module-checks.nix {
          inherit
            inputs
            lib
            pkgs
            self
            self'
            ;
        }
      );
    };
}
