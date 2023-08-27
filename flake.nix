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
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, rust-overlay, crane, flake-utils, advisory-db, ... }:
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

        src = pkgs.lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = path: type:
            (builtins.match ".*proto$" path != null)
            || (craneLib.filterCargoSources path type);
        };

        commonArgs = {
          inherit src;

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

        clippy = craneLib.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
        });

        format = craneLib.cargoFmt {
          inherit src;
        };

        audit = craneLib.cargoAudit {
          inherit src advisory-db;
        };

        ciUploadImage = pkgs.writeShellApplication {
          name = "ci-upload-image";
          runtimeInputs = with pkgs; [
            skopeo
          ];
          text = ''
            function dry_run() {
              if [[ "''${DRY_RUN:-false}" == "true" ]]; then
                echo "[dry-run] $*"
              else
                "$@"
              fi
            }

            archive="$1"
            dest="$2"
            tag="$3"
            archs=("x86_64")

            echo "Skopeo version: $(skopeo --version)"
            echo "Uploading image: $archive to $dest"

            if [[ ! -f "$archive" ]]; then
              echo "Image archive doesn't exist: $archive"
              exit 1
            fi

            images=()
            for arch in "''${archs[@]}"; do
              echo "Uploading image for $arch"
              dry_run skopeo --insecure-policy copy "docker-archive:$archive" "docker://$dest:$tag-$arch"
              images+=("$dest:$tag-$arch")
            done

            dry_run buildah manifest create "$dest:$tag" "''${images[@]}"
            dry_run buildah manifest push --all "$dest:$tag"
          '';
        };
      in
      {
        formatter = pkgs.nixpkgs-fmt;

        checks = {
          inherit clippy format audit;
        };

        packages = {
          default = bin;
          deps = cargoArtifacts;
          image = image;
        };

        devShells = {
          default = pkgs.mkShell (commonArgs // { });
          ci = pkgs.mkShell {
            buildInputs = with pkgs; [
              buildah
              skopeo
              ciUploadImage
            ];
          };
        };
      }
    );
}
