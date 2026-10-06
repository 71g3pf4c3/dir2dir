{
  lib,
  rustPlatform,
  src ? ../.,
}:

rustPlatform.buildRustPackage {
  pname = "dir2dir";
  version = "0.1.0";

  inherit src;

  cargoLock.lockFile = src + "/Cargo.lock";

  # The workspace has exactly one binary, `dir2dir`, produced by the
  # `crates/dir2dir-cli` member; building the workspace root builds it.

  meta = {
    description = "Directory → directory copying through a typed, plannable tree model";
    license = lib.licenses.gpl3Only;
    mainProgram = "dir2dir";
    maintainers = [ ];
  };
}
