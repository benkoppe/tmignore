# tmignore

`tmignore` finds restoreable development dependency/cache directories and excludes them from Time Machine backups.

The project is intended for declarative nix-darwin use. By default, the nix-darwin module runs in dry-run mode and does not change Time Machine exclusions.

## nix-darwin

Import the module from the flake and enable the service:

```nix
{
  inputs.tmignore.url = "github:benkoppe/tmignore";

  outputs = { self, nix-darwin, tmignore, ... }: {
    darwinConfigurations.example = nix-darwin.lib.darwinSystem {
      modules = [
        tmignore.darwinModules.default
        ({ config, ... }:
        let
          home = config.users.users.alice.home;
        in
        {
          services.tmignore = {
            enable = true;
            scan.roots = [ "${home}/Developer" ];
          };
        })
      ];
    };
  };
}
```

See [CONFIGURATION.md](CONFIGURATION.md) for the full nix-darwin module and TOML config reference.

The default schedule is daily at 03:30 local time:

```nix
services.tmignore.schedule = [
  { Hour = 3; Minute = 30; }
];
```

To run multiple times per day, add more entries:

```nix
services.tmignore.schedule = [
  { Hour = 9; Minute = 0; }
  { Hour = 17; Minute = 0; }
];
```

To apply Time Machine exclusions, opt in explicitly:

```nix
services.tmignore.mode = "apply";
```

The launchd job runs `tmignore all` by default, which combines project scanning with global dependency/cache directory exclusions. To run only the project scanner from nix-darwin:

```nix
services.tmignore.global.enable = false;
```

For laptops, `runAtLoad` can catch cases where the machine was asleep or off during a scheduled time:

```nix
services.tmignore.runAtLoad = true;
```

## Extra Rules

Named extra scan rules are generated as `scan.extra_rules.<name>` in the TOML config:

```nix
services.tmignore.scan.extraRules.pnpm_store = {
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
```

Extra global cache targets are fixed paths under known cache namespaces, resolved relative to the user's home directory:

```nix
services.tmignore.global.extraTargets.custom_cache.path = ".cargo/registry/custom";
```

Manual CLI usage is split into explicit commands:

```sh
tmignore scan --root ~/Developer
tmignore global
tmignore all --root ~/Developer
```
