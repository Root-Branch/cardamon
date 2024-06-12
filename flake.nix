{
  description = "Cardamon Flake ";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
        with pkgs; {
          devShells.default = mkShell {
            LD_LIBRARY_PATH = lib.makeLibraryPath [openssl];
            buildInputs = [
              openssl
              pkg-config
              eza
              fd
              rust-bin.stable.latest.default
              rust-analyzer
              cargo-watch
              pkgs.sqlite
              cargo-udeps # cargo udeps --remove
              cargo-audit # cargo audit
            ];

            shellHook = ''
              alias ls=eza
              export PATH=$PATH:${pkgs.rust-analyzer}/bin
              alias find=fd
            '';
          };
        }
    );
}
