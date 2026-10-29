{
  description = "Zoi - Advanced Package Manager & Environment Orchestrator";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "clippy"
          ];
        };

        nightlyRustfmt = pkgs.rust-bin.nightly.latest.rustfmt;

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        version = cargoToml.workspace.package.version;

        commonNativeBuildInputs = with pkgs; [
          pkg-config
          clang
          llvmPackages.libclang
        ];

        commonBuildInputs =
          with pkgs;
          [
            openssl
            xz
          ]
          ++ lib.optionals stdenv.hostPlatform.isLinux [
            bubblewrap
          ]
          ++ lib.optionals stdenv.hostPlatform.isDarwin [
            darwin.apple_sdk.frameworks.Security
            darwin.apple_sdk.frameworks.SystemConfiguration
          ];

        commonEnv = {
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.llvmPackages.libclang.version}/include";
          ZOI_COMMIT_HASH = self.shortRev or self.dirtyShortRev or "dev";
        };

        zoiPkg = pkgs.rustPlatform.buildRustPackage (
          commonEnv
          // {
            pname = "zoi";
            inherit version;

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            nativeBuildInputs = commonNativeBuildInputs;
            buildInputs = commonBuildInputs;

            cargoBuildFlags = [
              "-p"
              "zoi-rs"
            ];

            meta = with pkgs.lib; {
              description = "Advanced Package Manager & Environment Orchestrator";
              homepage = "https://zillowe.qzz.io/zds/zoi";
              license = licenses.asl20;
              mainProgram = "zoi";
              platforms = platforms.unix;
            };
          }
        );
      in
      {
        packages = {
          default = zoiPkg;
          zoi = zoiPkg;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = zoiPkg;
        };

        devShells.default = pkgs.mkShell (
          commonEnv
          // {
            name = "zoi-dev";

            nativeBuildInputs = with pkgs; [
              rustToolchain
              nightlyRustfmt

              just
              pkg-config
              git
              gnupg
              asciidoctor

              clang
              lld
              llvm
            ];

            buildInputs =
              with pkgs;
              [
                openssl
                xz
                llvmPackages.libclang
              ]
              ++ lib.optionals stdenv.hostPlatform.isLinux [
                bubblewrap
              ]
              ++ lib.optionals stdenv.hostPlatform.isDarwin [
                darwin.apple_sdk.frameworks.Security
                darwin.apple_sdk.frameworks.SystemConfiguration
              ];

            CC = "clang";
            CXX = "clang++";

            shellHook = ''
              echo "Zoi development shell"
              echo "   Clang:   $(clang --version | head -n 1)"
              echo "   Rust:    $(rustc --version)"
              echo "   Clippy:  $(cargo-clippy --version 2>/dev/null || clippy-driver --version 2>/dev/null)"
              echo "   Rustfmt: $(rustfmt --version)"
              echo "   LIBCLANG_PATH: $LIBCLANG_PATH"
            '';
          }
        );
      }
    )
    // {
      overlays.default = final: _prev: {
        zoi = self.packages.${final.system}.default;
      };
    };
}
