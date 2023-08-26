{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils = {
      url = "github:numtide/flake-utils";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        commonArgs = {
          src = pkgs.lib.cleanSourceWith {
            src = craneLib.path ./.;
            filter = path: type:
              (builtins.match ".*proto$" path != null)
              || (craneLib.filterCargoSources path type);
          };

          nativeBuildInputs = with pkgs; [
            clang
            llvmPackages.libclang.lib
            pkg-config
            protobuf
            rustToolchain
          ] ++ pkgs.lib.optional stdenv.isDarwin (with pkgs.darwin.apple_sdk.frameworks; [
            CoreFoundation
            CoreServices
            Security
          ]);
        };

        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          pname = "buildkite-keda-scaler-deps";
        });

        bin = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });

        image = pkgs.dockerTools.buildImage {
          name = "buildkite-keda-scaler";
          tag = "latest";
          created = "now";
          copyToRoot = with pkgs.dockerTools; [
            usrBinEnv
            binSh
            caCertificates
            bin
          ];
          config = {
            Entrypoint = [
              "${bin}/bin/buildkite-keda-scaler"
            ];
            ExposedPorts = {
              "9090/tcp" = { };
            };
          };
        };
      in
      {
        formatter = pkgs.nixpkgs-fmt;

        packages = {
          default = bin;
          image = image;
        };

        devShells = {
          default = pkgs.mkShell (commonArgs // { });
        };
      }
    );
}
