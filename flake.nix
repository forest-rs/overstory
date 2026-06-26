{
  description = "Local dev shell for building and running the Overstory Wayland example with text (system-fonts).";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [rust-overlay.overlays.default];
      };

      # Latest stable toolchain (the workspace needs edition 2024 /
      # rust-version 1.92, which any recent stable satisfies).
      rust = pkgs.rust-bin.stable.latest.default;

      # Linked at build time via pkg-config (parley -> fontique ->
      # yeslogic-fontconfig-sys links libfontconfig) and loaded at run time.
      # fontconfig.pc also requires freetype2 and expat.
      fontLibs = with pkgs; [fontconfig freetype expat];

      # dlopen'd at run time only: libwayland-client (wayland-backend `dlopen`),
      # libxkbcommon (xkbcommon-dl), libvulkan (wgpu Vulkan backend).
      runtimeLibs = with pkgs; [wayland libxkbcommon vulkan-loader];
    in {
      devShells.default = pkgs.mkShell {
        packages = [rust pkgs.pkg-config] ++ fontLibs ++ runtimeLibs;
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (fontLibs ++ runtimeLibs);
      };
    });
}
