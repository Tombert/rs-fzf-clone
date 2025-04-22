{
  description = "Statically linked swaybar using musl";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };

        targetTriple = "x86_64-unknown-linux-musl";

        crossPkgs = pkgs.pkgsCross.musl64;

      in {
        packages.default = crossPkgs.rustPlatform.buildRustPackage {
          pname = "swaybar-rs";
          version = "0.1.0";
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;

          target = targetTriple;

	  buildInputs = with crossPkgs; [ musl.dev ];
	  nativeBuildInputs = with pkgs; [ pkg-config binutils ];

          RUSTFLAGS = "-C target-feature=+crt-static";

          doCheck = false;
          stripAll = true;

          installPhase = ''
            mkdir -p $out/bin
            cp target/${targetTriple}/release/rust-fzf-clone $out/bin/
            strip $out/bin/rust-fzf-clone
          '';
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.rust-bin.stable.latest.complete
            pkgs.pkg-config
            pkgs.musl
            pkgs.rust-analyzer
          ];
        };
      });
}
