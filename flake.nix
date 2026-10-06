{
  description = "dir2dir — directory → directory, the over-engineered way";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          dir2dir = pkgs.callPackage ./nix/package.nix { src = self; };
          default = self.packages.${system}.dir2dir;
        });

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.callPackage ./nix/shell.nix { };
        });

      overlays.default = final: _prev: {
        dir2dir = final.callPackage ./nix/package.nix { src = self; };
      };

      homeManagerModules.default = import ./nix/hm-module.nix;

      checks = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          dir2dir = self.packages.${system}.dir2dir;
        in
        {
          inherit dir2dir;

          # Exercise the JSON port end to end through the built binary.
          json-port-roundtrip = pkgs.runCommand "dir2dir-json-port-roundtrip" { } ''
            mkdir -p $out
            exe=${dir2dir}/bin/dir2dir
            printf '%s' '{"greeting":"Hello, world!","run.sh":["script","#!/bin/sh\necho Howdy!"],"l":["link","/"]}' | $exe from-json $out
            test -f $out/greeting
            test -x $out/run.sh
            test -L $out/l
            touch $out
          '';
        });
    };
}
