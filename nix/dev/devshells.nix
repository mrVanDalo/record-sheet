{ inputs, ... }:
{

  imports = [ inputs.devshell.flakeModule ];

  perSystem =
    {
      pkgs,
      self',
      system,
      ...
    }:
    {

      # allow unfree packages
      _module.args.pkgs = import inputs.nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };

      devshells.default = {

        commands = [
          {
            help = "run cargo test and auto-accept Insta snapshots";
            name = "insta-accept";
            command = "INSTA_UPDATE=always cargo test";
          }
          {
            help = "run Insta interactive review for pending snapshots";
            name = "insta-review";
            command = "cargo insta review";
          }
        ];

        packages = with pkgs; [
          clang
          cargo
          cargo-insta
          clippy
          rust-analyzer
          rustc
          rustfmt
        ];
      };
    };
}
