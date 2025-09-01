{
  description = "Bitcoin Augur - Fee estimation library and server in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "x86_64-unknown-linux-musl" ];
        };

      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            bashInteractive
            rustToolchain
            pkg-config
            pkgsStatic.stdenv.cc
            cargo-edit
            cargo-watch
            cargo-audit
            cargo-deny
            cargo-outdated
            cargo-tarpaulin
            git
            just
            bacon
            tokio-console
            # For integration testing with Kotlin/Java reference implementation
            jdk17
            gradle
          ];

          # Musl target configuration using pkgsStatic
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
          CC_x86_64_unknown_linux_musl = "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
          
          shellHook = ''
            echo "Bitcoin Augur Rust Development Environment"
            echo "=========================================="
            echo ""
            echo "Available commands:"
            echo "  cargo build                                      # Build for musl target"
            echo "  cargo build --release                            # Build optimized for musl"
            echo "  cargo test                                       # Run tests"
            echo "  cargo bench                                      # Run benchmarks"
            echo "  cargo tarpaulin                                  # Generate code coverage report"
            echo ""
            echo "Rust version: $(rustc --version)"
            echo "Default target: x86_64-unknown-linux-musl"
            echo ""
            
            # Automatically configure Git hooks for code quality
            if [ -d .git ] && [ -d .githooks ]; then
              current_hooks_path=$(git config core.hooksPath || echo "")
              if [ "$current_hooks_path" != ".githooks" ]; then
                echo "📎 Setting up Git hooks for code quality checks..."
                git config core.hooksPath .githooks
                echo "✅ Git hooks configured automatically!"
                echo "   • pre-commit: Checks code formatting"
                echo "   • pre-push: Runs formatting, clippy, and checks for outdated dependencies"
                echo ""
                echo "To disable: git config --unset core.hooksPath"
                echo ""
              fi
            fi
          '';
        };

        # Package definitions
        packages = rec {
          bitcoin-augur = let
            rustPlatformMusl = pkgs.makeRustPlatform {
              cargo = rustToolchain;
              rustc = rustToolchain;
            };
          in rustPlatformMusl.buildRustPackage {
            pname = "bitcoin-augur";
            version = "0.1.0";
            
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            
            nativeBuildInputs = with pkgs; [
              pkg-config
              rustToolchain
              pkgsStatic.stdenv.cc
            ];
            
            # Musl target configuration
            CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
            CC_x86_64_unknown_linux_musl = "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
            CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static -C link-arg=-static";
            
            # Override buildPhase to use the correct target
            buildPhase = ''
              runHook preBuild
              
              echo "Building with musl target..."
              cargo build \
                --release \
                --target x86_64-unknown-linux-musl \
                --offline \
                -j $NIX_BUILD_CORES
              
              runHook postBuild
            '';
            
            installPhase = ''
              runHook preInstall
              
              mkdir -p $out/bin
              cp target/x86_64-unknown-linux-musl/release/bitcoin-augur-server $out/bin/
              
              runHook postInstall
            '';
            
            doCheck = false; # Tests don't work well with static linking
            
            # Verify the binary is statically linked
            postInstall = ''
              echo "Checking if binary is statically linked..."
              file $out/bin/bitcoin-augur-server || true
              # Strip the binary to reduce size
              ${pkgs.binutils}/bin/strip $out/bin/bitcoin-augur-server || true
            '';
          };
          
          default = bitcoin-augur;
          bitcoin-augur-server = bitcoin-augur; # Alias for CI compatibility
          
          # Docker image with static binary and debugging tools
          docker = let
            second = 1000000000; # 1 second in nanoseconds
          in pkgs.dockerTools.buildImage {
            name = "bitcoin-augur-server";
            tag = "latest";
            
            copyToRoot = pkgs.buildEnv {
              name = "image-root";
              paths = [ 
                bitcoin-augur
                pkgs.bashInteractive
                pkgs.coreutils
                pkgs.curl
                pkgs.cacert
                pkgs.bitcoind
                pkgs.jq
                pkgs.netcat
                pkgs.procps
                pkgs.htop
                pkgs.vim
                pkgs.less
                pkgs.gnugrep
                pkgs.gawk
                pkgs.gnused
                pkgs.findutils
                pkgs.which
                pkgs.net-tools
                pkgs.iputils
                pkgs.dnsutils
                pkgs.gnutar
                pkgs.file
              ];
              pathsToLink = [ "/bin" "/etc" "/share" ];
            };
            
            config = {
              Entrypoint = [ "/bin/bitcoin-augur-server" ];
              Cmd = [];
              Env = [
                "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
                "SYSTEM_CERTIFICATE_PATH=${pkgs.cacert}/etc/ssl/certs"
                "RUST_LOG=info"
              ];
              ExposedPorts = {
                "8080/tcp" = {};
              };
              WorkingDir = "/";
              Labels = {
                "org.opencontainers.image.title" = "Bitcoin Augur Server";
                "org.opencontainers.image.description" = "Statistical Bitcoin fee estimation service";
                "org.opencontainers.image.vendor" = "Bitcoin Augur";
                "org.opencontainers.image.source" = "https://github.com/${self.owner or "bitcoin-augur"}/${self.repo or "bitcoin-augur-rust"}";
                "org.opencontainers.image.documentation" = "https://github.com/${self.owner or "bitcoin-augur"}/${self.repo or "bitcoin-augur-rust"}#readme";
              };
              Healthcheck = {
                Test = [ "CMD" "${pkgs.curl}/bin/curl" "-f" "http://localhost:8080/health" ];
                Interval = 30 * second; # 30 seconds
                Timeout = 3 * second;   # 3 seconds
                StartPeriod = 10 * second; # 10 seconds
                Retries = 3;
              };
            };
          };
        };
      }
    );
}