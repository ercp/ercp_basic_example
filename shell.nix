{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    # Build toolchain.
    rustup

    # Debugging tools.
    openocd
    gcc-arm-embedded
  ];
}
