{
  mkShell,
  rustc,
  cargo,
  rustfmt,
  clippy,
  rust-analyzer,
}:

mkShell {
  packages = [
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer
  ];

  shellHook = ''
    echo "dir2dir devshell"
    echo "  cargo build / cargo test / cargo clippy -- -D warnings / cargo fmt"
  '';
}
