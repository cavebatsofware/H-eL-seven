# Dev shell for hl7-ui (web via dx, desktop via dioxus-desktop -> wry/webkitgtk).
#
#   nix-shell --run "dx serve --package hl7-ui --web"
#   nix-shell --run "dx serve --package hl7-ui --desktop"
#
# nixpkgs' dioxus-cli wraps wasm-bindgen-cli 0.2.118, older than what the
# project's Cargo.lock resolves (0.2.126). The wrapper only APPENDS its copy
# to PATH, so the matching build below (prepended by mkShell) wins.
{ pkgs ? import <nixpkgs> { } }:

let
  wasm-bindgen-cli_0_2_126 = pkgs.buildWasmBindgenCli rec {
    src = pkgs.fetchCrate {
      pname = "wasm-bindgen-cli";
      version = "0.2.126";
      hash = "sha256-H6Is3fiZVxZCfOMWK5dWMSrtn50VGv0sfdnsT+cTtyk=";
    };
    cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
      inherit src;
      inherit (src) pname version;
      hash = "sha256-VucqkXbCi4qtQzY/HrXiDnbSURsagPsdNVMn1Tw3UiY=";
    };
  };
in
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [ pkg-config gsettings-desktop-schemas ];
  buildInputs = with pkgs; [
    openssl
    glib
    gtk3
    libsoup_3
    webkitgtk_4_1
    xdotool
    dioxus-cli
    wasm-bindgen-cli_0_2_126
    binaryen # wasm-opt, used by release web builds (dx build --release)
    cargo-audit
  ];
  shellHook = with pkgs; ''
    export XDG_DATA_DIRS=${gsettings-desktop-schemas}/share/gsettings-schemas/${gsettings-desktop-schemas.name}:${gtk3}/share/gsettings-schemas/${gtk3.name}:$XDG_DATA_DIRS
  '';
}
