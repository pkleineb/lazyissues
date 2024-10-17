{
  description = "Nix Development Flake for my game engine";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      };

      toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    in {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          toolchain
          pkg-config
          openssl

          rust-analyzer-unwrapped
        ];

        RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";

        shellHook = ''
          export PATH=$PATH:''${CARGO_HOME:-~/.cargo}/bin
          exec $SHELL
        '';
      };
    };
}
