{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    # Build toolchain.
    rustup
    cargo-embed

    # Debugging tools.
    openocd
    gcc-arm-embedded
  ];
}
