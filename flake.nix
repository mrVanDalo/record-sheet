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

          # Shared src filtered to exclude build artifacts (each cargoRoot
          # manages its own target dir).
          src = pkgs.lib.cleanSourceWith {
            src = pkgs.lib.cleanSource ./.;
            filter =
              name: type:
              let
                base = builtins.baseNameOf (toString name);
              in
              !(base == "target" && type == "directory");
          };

          record-sheet = rustPlatform.buildRustPackage {
            pname = "record-sheet";
            version = "0.1.0";
            src = src;
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = true;
          };

          wasm-site = rustPlatform.buildRustPackage {
            pname = "record-sheet-wasm";
            version = "0.1.0";
            src = src;
            cargoRoot = "website";
            cargoLock.lockFile = ./website/Cargo.lock;
            doCheck = false;
            # The cargo hook hardcodes native target; override buildPhase
            # to build for wasm32.
            dontUseCargoBuild = true;
            buildPhase = ''
              runHook preBuild
              CARGO_TARGET_DIR="$(pwd)/target"
              export CARGO_TARGET_DIR
              cd "$cargoRoot"
              cargo build \
                -j "$NIX_BUILD_CORES" \
                --target wasm32-unknown-unknown \
                --offline \
                --release
              runHook postBuild
            '';
            nativeBuildInputs = [
              pkgs.wasm-bindgen-cli
              pkgs.lld # provides wasm-ld for wasm32 linking
            ];
            CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "wasm-ld";
            installPhase = ''
                          runHook preInstall
              mkdir -p $out/pkg
              wasm-bindgen \
                --target web \
                --out-dir $out/pkg \
                "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/record_sheet_wasm.wasm"
              # Stage the full site: static assets alongside the wasm pkg.
              # installPhase cwd is the cargoRoot (website/); files are local.
              cp index.html app.js style.css "$out/"
              touch "$out/.nojekyll"
              runHook postInstall
            '';
          };

          # Full staged site, for CI: `nix build .#wasm-build -o site`
          wasm-build = wasm-site;

          wasm-build-tmp = pkgs.writeShellApplication {
            name = "wasm-build-tmp";
            runtimeInputs = [ pkgs.coreutils ];
            text = ''
              set -euo pipefail
              dst="website/tmp"
              rm -rf "$dst"
              mkdir -p "$dst"
              cp -r ${wasm-site}/. "$dst/"
              chmod -R u+w "$dst"
              echo ""
              echo "wasm site written to website/tmp"
              echo "serve with:"
              echo "  python3 -m http.server -d website/tmp 8000"
            '';
          };
          # Generate sample PDFs exercising every title/logo combination.
          create-test-pdfs = pkgs.writeShellApplication {
            name = "create-test-pdfs";
            runtimeInputs = [ record-sheet ];
            text = ''
              set -euo pipefail
              out=''${1:-.}
              mkdir -p "$out"
              record-sheet 2026-09-15 -o "$out/test-with-title.pdf" --title "Homework log"
              record-sheet 2026-09-15 -o "$out/test-with-title-and-logo.pdf" \
                --title "Homework log" --logo ${./assets/qr-code.png}
              record-sheet 2026-09-15 -o "$out/test-logo-only.pdf" \
                --logo ${./assets/qr-code.png}
              record-sheet 2026-09-15 -o "$out/test-plain.pdf"
              record-sheet 2026-09-15 -o "$out/test-with-title-qr.pdf" \
                --title "Homework log" --qr-code "https://example.com/"
              record-sheet 2026-09-15 -o "$out/test-with-title-and-logo-qr.pdf" \
                --title "Homework log" --logo ${./assets/qr-code.png} \
                --qr-code "https://example.com/"
              record-sheet 2026-09-15 -o "$out/test-logo-only-qr.pdf" \
                --logo ${./assets/qr-code.png} --qr-code "https://example.com/"
              record-sheet 2026-09-15 -o "$out/test-plain-qr.pdf" \
                --qr-code "https://example.com/"
              echo "8 test PDFs written to $out"
            '';
          };
        in
        {
          packages = {
            inherit
              record-sheet
              wasm-site
              wasm-build
              wasm-build-tmp
              create-test-pdfs
              ;
            default = record-sheet;
          };
          checks.record-sheet = record-sheet;
          apps = {
            create-test-pdfs = {
              type = "app";
              program = "${pkgs.writeShellScript "create-test-pdfs" ''
                exec ${create-test-pdfs}/bin/create-test-pdfs "$@"
              ''}";
            };
            wasm-build = {
              type = "app";
              program = "${pkgs.writeShellScript "wasm-build" ''
                echo "${wasm-build}"
              ''}";
            };
            wasm-site = {
              type = "app";
              program = "${pkgs.writeShellScript "wasm-site" ''
                echo "${wasm-site}"
              ''}";
            };
          };
        };
      flake = {
        # The usual flake attributes can be defined here, including system-
        # agnostic ones like nixosModule and system-enumerating ones, although
        # those are more easily expressed in perSystem.
      };
    };
}
