{
  description = "OpenHome package builds";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        openhome = pkgs.rustPlatform.buildRustPackage {
          pname = "openhome";
          version = "0.1.0";
          src = ./cli;
          cargoHash = "sha256-K1dwtDDPgax+txLfOIl1i1wQQ6aIuw9s/Dam0NjN7kI=";
          doCheck = false; # Process tests need loopback sockets, which the Nix sandbox blocks.
        };
      in {
        packages.openhome = openhome;
        packages.default = openhome;
        checks.openhome = openhome;
      });
}
