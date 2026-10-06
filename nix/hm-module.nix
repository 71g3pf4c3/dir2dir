# A home-manager module: declarative directory trees materialized by dir2dir.
#
# Trees are written in the json2dir scheme directly as Nix values:
#   strings are files, lists are tagged nodes (["link" t] / ["script" c] /
#   ["b64" data]), attribute sets are directories.
#
# Materialization runs in the activation script, after writeBoundary, and
# honors `home-manager generations --dry` via $DRY_RUN_CMD.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.dir2dir;

  # Activation scripts run without tilde expansion in quoted words.
  expandTilde =
    dest:
    if lib.hasPrefix "~/" dest then "$HOME/" + lib.removePrefix "~/" dest else dest;

  jsonFile = dest: tree: pkgs.writeText "dir2dir-${builtins.hashString "sha256" (dest + builtins.toJSON tree)}.json" (builtins.toJSON tree);
in
{
  options.dir2dir = {
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ./package.nix { src = ../.; };
      description = "The dir2dir package to use.";
    };

    prune = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to delete paths in destination trees that are absent from the declared model.";
    };

    trees = lib.mkOption {
      type = lib.types.attrsOf lib.types.anything;
      default = { };
      description = ''
        Declarative trees: attribute set of destination directory → tree in
        the json2dir scheme (strings are files, ["link" t] symlinks,
        ["script" c] executables, attrsets directories). Tildes in
        destination paths are expanded against $HOME.
      '';
    };
  };

  config = lib.mkIf (cfg.trees != { }) {
    home.activation.dir2dir = lib.hm.dag.entryAfter [ "writeBoundary" ] (
      lib.concatStringsSep "\n" (
        lib.mapAttrsToList (
          dest: tree:
          "$DRY_RUN_CMD ${cfg.package}/bin/dir2dir from-json ${lib.optionalString cfg.prune "--prune"} \"${expandTilde dest}\" < ${jsonFile dest tree}"
        ) cfg.trees
      )
    );
  };
}
