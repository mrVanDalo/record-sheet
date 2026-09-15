{
  description = "Description for the project";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {

      imports = [
        inputs.flake-parts.flakeModules.partitions
      ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      # Outputs are only built when the named partition is loaded. Consumers
      # of this flake as a dependency will not see these inputs.
      partitionedAttrs.checks = "ci";
      partitionedAttrs.formatter = "ci";
      partitionedAttrs.devShells = "dev";

      # CI partition: lightweight inputs needed by CI runs (formatter checks,
      # website renderers, ...). Loaded by `nix flake check` / `nix fmt`.
      partitions.ci = {
        extraInputsFlake = ./nix/ci;
        module = {
          imports = [
            ./nix/ci/formatter.nix
          ];
        };
      };

      # Dev partition: everything a human needs interactively. Loaded by
      # `nix develop`. Inherits CI inputs implicitly via partitionedAttrs.
      partitions.dev = {
        extraInputsFlake = ./nix/dev;
        module = {
          imports = [
            ./nix/dev/devshells.nix
          ];
        };
      };

      perSystem =
        {
          pkgs,
          ...
        }:
        let
          rustPlatform = pkgs.rustPlatform;
          record-sheet = rustPlatform.buildRustPackage {
            pname = "record-sheet";
            version = "0.1.0";
            src = pkgs.lib.cleanSource ./.;
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = true;
          };
        in
        {
          packages = {
            inherit record-sheet;
            default = record-sheet;
          };
          checks.record-sheet = record-sheet;
        };
      flake = {
        # The usual flake attributes can be defined here, including system-
        # agnostic ones like nixosModule and system-enumerating ones, although
        # those are more easily expressed in perSystem.
      };
    };
}
