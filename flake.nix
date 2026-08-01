{
  description = "Bio-Formal Stack: LiquidHaskell + SWI-Prolog + Rust + Lean4 + Bifrost Tooling";

  inputs = {
    nixpkgs.url     = "github:NixOS/nixpkgs/nixos-24.05";
    haskell-flake.url = "github:input-output-hk/haskell-flake/v2.6.0";
    lean4-flake.url   = "github:leanprover/lean4-flake/v0.2.0";
    fenix.url         = "github:nix-community/fenix/2024.05.0";
  };

  outputs = { self, nixpkgs, haskell-flake, lean4-flake, fenix, ... }@inputs:
    let
      system  = "x86_64-linux";
      pkgs    = nixpkgs.legacyPackages.${system};

      haskell       = haskell-flake.packages.${system}.compiler.ghc98;
      liquidHaskell = haskell.packages.liquidhaskell;

      bioHaskellPkgs = haskell.packages.extend (self: super: {
        bio-formal = super.haskell.lib.justStaticExecutables
          (super.haskell.lib.dontCheck
            (super.callCabal2nix "bio-formal" ./haskell {
              buildDepends = with super; [ liquidhaskell liquid-base liquid-containers liquid-vector ];
              ghcOptions   = [ "-fplugin=LiquidHaskell"
                               "-fplugin-opt=LiquidHaskell:--no-termination-check" ];
            }));
      });

      swiProlog = pkgs.swiprolog.withPackages
        (p: [ p.clpfd p.sgml p.http p.ssl p.odbc p.jpl ]);

      rustToolchain = fenix.packages.${system}.fromToolchainFile {
        path = ./rust-toolchain.toml;
      };

      lean4         = lean4-flake.packages.${system}.lean4;
      lean4Packages = lean4-flake.packages.${system}.lean4Packages;

      bifrostCli = pkgs.writeShellScriptBin "bifrost" ''
        echo "Bifrost CLI v0.1.0 (Sovereign Build)"
        echo "Usage: bifrost <audit|verify|log> ..."
      '';

      devShell = pkgs.mkShell {
        name = "bio-formal-dev";
        nativeBuildInputs = with pkgs; [
          haskell.ghc liquidHaskell swiProlog rustToolchain lean4 bifrostCli
          git direnv jq cacert gnumake cmake
          pkg-config gmp mpfr mpc openssl clang
        ];

        LIQUID_HASKELL_PATH           = "${liquidHaskell}/bin:${haskell.ghc}/bin";
        HASKELL_PACKAGE_SANDBOX       = "true";
        CARGO_NET_GIT_FETCH_WITH_CLI  = "true";
        RUSTFLAGS = "-L ${pkgs.gmp.lib}/lib -L ${pkgs.mpfr.lib}/lib -C link-arg=-Wl,-rpath,${pkgs.gmp.lib}/lib:${pkgs.mpfr.lib}/lib";
        GMP_DIR       = pkgs.gmp.dev;
        MPFR_DIR      = pkgs.mpfr.dev;
        MPC_DIR       = pkgs.mpc.dev;
        LIBRARY_PATH  = "${pkgs.gmp.lib}/lib:${pkgs.mpfr.lib}/lib:${pkgs.openssl.lib}/lib";
        CPATH         = "${pkgs.gmp.dev}/include:${pkgs.mpfr.dev}/include:${pkgs.openssl.dev}/include";
        PKG_CONFIG_PATH = "${pkgs.gmp.lib}/pkgconfig:${pkgs.mpfr.lib}/pkgconfig:${pkgs.openssl.lib}/pkgconfig";
        LEAN_PATH     = "${lean4}/bin:${lean4Packages.mathlib4}/lib/lean";
        SWI_HOME_DIR  = swiProlog;

        shellHook = ''
          export PS1="\[\e[32m\][Bio-Formal]\[\e[0m\] \w \$ "
          if command -v direnv &>/dev/null; then eval "$(direnv hook bash)"; fi
          echo "--- SOVEREIGN TOOLCHAIN VERIFICATION ---"
          echo "GHC:       $(ghc --numeric-version 2>/dev/null || echo n/a)"
          echo "LH:        $(liquid --version 2>&1 | head -1 || echo n/a)"
          echo "SWI-Prolog:$(swipl --version 2>&1 | head -1 || echo n/a)"
          echo "Rust:      $(rustc --version 2>/dev/null || echo n/a)"
          echo "Lean:      $(lean --version 2>/dev/null || echo n/a)"
          echo "GMP:       $(pkg-config --modversion gmp 2>/dev/null || echo n/a)"
          echo "------------------------------------------"
          alias lh-check='liquid --diff --short-errors'
          alias pl-repl='swipl -q -g "set_prolog_flag(encoding,utf8)" -t main'
          alias lean-build='lake build'
          alias cargo-test='cargo test --all -- --nocapture'
        '';
      };

    in {
      devShells.${system}.default = devShell;
      packages.${system} = {
        inherit bioHaskellPkgs;
        bio-sim-rust   = pkgs.stdenv.mkDerivation { name = "bio-sim-env"; src = ./.; buildPhase = "true"; installPhase = "true"; };
        lean4-env      = lean4Packages.mathlib4;
        bifrost-cli    = bifrostCli;
      };
      checks.${system} = {
        haskell-typecheck = pkgs.runCommand "haskell-typecheck"
          { nativeBuildInputs = [ haskell.ghc liquidHaskell ]; }
          "cd ${./haskell} && cabal build --ghc-options='-fplugin=LiquidHaskell' all";
        prolog-tests = pkgs.runCommand "prolog-tests"
          { nativeBuildInputs = [ swiProlog ]; }
          "cd ${./logic} && swipl -q -g run_tests -t halt test_bio.pl";
        rust-build = pkgs.runCommand "rust-build"
          { nativeBuildInputs = [ rustToolchain pkgs.pkg-config pkgs.gmp pkgs.mpfr ]; }
          "cd ${./crates} && cargo build --release --all-targets && cargo test --release --all";
        lean-proofs = pkgs.runCommand "lean-proofs"
          { nativeBuildInputs = [ lean4 ]; }
          "cd ${./lean} && lake update && lake build";
      };
    };
}
