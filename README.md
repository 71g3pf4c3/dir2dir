# dir2dir

> fuck json2dir, let's do dir2dir

`dir2dir` copies directory trees. Through a typed intermediate representation.
With a plan, a diff, a dry-run, and a clean-architecture complex.

```
$ dir2dir copy ~/.dotfiles/foot ~/.config/foot
create    theme.ini
create    colors/session.ini
1 created, 0 overwritten, 0 extraneous
```

`json2dir` was a good idea wearing a blindfold: a human-readable JSON document that
materializes into a directory tree. This is the same idea with the loop closed —
you can capture a tree, not just materialize one — and with the mutation path
gated behind an explicit, inspectable change set instead of "delete first, ask
never".

---

## Why this is better than json2dir

`json2dir` converts JSON → directory. That's it. Half a tool. Here is the other
half, plus a few things the first half should have had.

| | json2dir | dir2dir |
|---|---|---|
| direction | JSON → dir, one-way | dir → dir · dir → JSON · JSON → dir |
| binary files | ❌ UTF-8 only (documented limitation) | ✅ `["b64", …]` / `["xb64", …]` |
| dry-run / preview | ❌ | ✅ plan is computed before any mutation |
| repeated runs | deletes and rewrites everything | writes only changed nodes |
| extraneous files | silently ignored | reported; deleted with `--prune` |
| tree diff | ❌ | `dir2dir diff`, `diff(1)`-style exit codes |
| TOCTOU | explicitly disclaimed | never writes *through* symlinks; path invariants enforced in the model, not the CLI |
| file modes | exec bit only | full `0777` bits on the native copy path |
| core | — | zero-dependency, zero-I/O domain crate |
| nix | flake | flake + devshell + home-manager module + derivation builder |

The arguments in detail:

**1. The loop is closed.** `json2dir`'s own README pitches "directory archives,
made human-readable" — but the tool can only *extract* an archive. To produce
one you hand-write JSON, or bolt on `jq`. In practice the "human-readable
archive" is a hand-maintained source of truth that drifts from what's on disk.
`dir2dir` captures a tree into an `FsTree` model first; the JSON document
becomes a *checkpoint and transport format*, not a write-only art form.
Round-trip: `dir2dir to-json DIR | dir2dir from-json DST`.

**2. Mutations are planned, not surprised.** Before touching the destination,
the sink computes a `ChangeSet`: what will be created, what overwritten, what
is extraneous. `--dry-run` prints it and exits; the real run executes exactly
that plan. `json2dir` tries to delete-then-write everything on every
invocation — every run rewrites every file, mtime and all. `dir2dir` writes
only the nodes the plan marks dirty; an unchanged tree is a no-op, which makes
it usable from activation scripts and CI without side effects.

**3. Binary files exist.** JSON strings are UTF-8; your trees are not.
Non-UTF-8 content is carried as base64 (`["b64", …]`, `["xb64", …]` for the
executable flavor) and stays raw bytes on the native dir → dir path.

**4. Symlink hygiene.** `json2dir` states it "makes no attempt to guard"
against TOCTOU. `dir2dir` will not write *through* an existing symlink at a
planned path — the node is removed first, then created. It's not a full
TOCTOU hardening (see caveats), but it closes the worst hole: an attacker
pre-seeding `~/.config/whatever → ~/.ssh/authorized_keys` no longer gets your
file contents written through the link.

**5. Invariants live in the model.** Rooted paths, `..`, embedded `/`, NUL
bytes, empty names — rejected by the domain constructors at the model boundary,
so every codec and every adapter inherits the validation instead of each
re-implementing it (or forgetting to).

**6. Diff is a first-class operation.** `dir2dir diff A B` is a pure model
comparison — no I/O in the comparator, `diff(1)`-compatible exit code (0
identical, 1 different). Useful in tests, useful in scripts.

**7. The architecture is the feature.** The domain crate has zero dependencies
and zero I/O. Use cases depend on ports (`TreeSource`, `TreeSink`,
`TreeCodec`), not on the filesystem or on JSON. The binary is one possible
composition; a Nix derivation builder, a home-manager activation hook and your
test suite are three others, all reusing the same core without mocking the
filesystem.

---

## Architecture

Hexagonal, unapologetically. Dependencies point inward; the domain knows
nothing about anybody.

```
             ┌───────────────────────────────────────────────┐
             │                    cli                        │
             │   copy · to-json · from-json · diff           │
             └───────────────────────┬───────────────────────┘
                                     │ composes
             ┌───────────────────────▼───────────────────────┐
             │                 application                   │
             │  use cases: CopyDir · ExportTree ·            │
             │             ImportTree · DiffDirs              │
             │  ports:    TreeSource · TreeSink ·           │
             │             TreeCodec                          │
             └──────┬───────────────────────────────┬───────┘
                    ▲ implements                    ▲ implements
     ┌──────────────┴─────────┐         ┌───────────┴──────────────┐
     │      infrastructure    │         │       infrastructure     │
     │  FsSource (scan)       │         │  FsSink (plan → apply)   │
     │  FsSource ⊂ FsSink     │         │  JsonCodec               │
     └──────────────┬─────────┘         └───────────┬──────────────┘
                    │                                │
             ┌──────▼────────────────────────────────▼──────┐
             │                    domain                      │
             │  FsTree · Node · NodeName · NodePath ·        │
             │  FileMode · ChangeSet (pure diff)              │
             │  — zero dependencies · zero I/O —              │
             └───────────────────────────────────────────────┘
```

Crates (cargo workspace):

| crate | contains | deps |
|---|---|---|
| `dir2dir-domain` | tree model, invariants, pure `diff` | none |
| `dir2dir-application` | use cases, ports, port error contracts | `domain` |
| `dir2dir-infrastructure` | `FsSource`, `FsSink`, `JsonCodec` adapters | `application`, serde, base64 |
| `dir2dir-cli` | clap wiring, plan reporting, exit codes | everything above |

Adding a codec (TOML, MessagePack, whatever) is an adapter. It cannot get the
invariants wrong, because it has to build a `FsTree` through the validated
constructors.

## Conversion scheme

The JSON port is `json2dir`-compatible, extended for binary content:

| JSON | filesystem |
|---|---|
| `{}` / object | directory |
| `"string"` | file (UTF-8 content) |
| `["link", "target"]` | symbolic link |
| `["script", "#!/bin/sh …"]` | executable file |
| `["b64", "…"]` | binary file (base64 content) |
| `["xb64", "…"]` | executable binary file |

Object keys are single path segments — `"a/b"`, `"."`, `".."`, rooted and
empty names are rejected, same policy as `json2dir`, enforced in the domain.

## Usage

```console
# copy a tree (only changed nodes are written)
$ dir2dir copy ~/dotfiles/foot ~/.config/foot

# see what would happen; touch nothing
$ dir2dir copy ~/dotfiles ~/.config --dry-run

# rsync --delete semantics, opt-in
$ dir2dir copy ~/dotfiles ~/.config --prune

# checkpoint a tree as human-readable JSON
$ dir2dir to-json ~/.config/foot > foot.json

# ... and back
$ dir2dir from-json ~/.config/foot < foot.json

# pure model diff, diff(1) exit codes
$ dir2dir diff ~/.config/foot ~/dotfiles/foot; echo $?
```

## Nix

Flake outputs: `packages.default`, `devShells.default`, `overlays.default`,
`homeManagerModules.default`.

```console
$ nix build . && ./result/bin/dir2dir --help
$ nix develop   # rustc, cargo, clippy, rustfmt, rust-analyzer
```

home-manager module — declarative trees, materialized on activation with
prune semantics, respecting `home-manager --dry`:

```nix
{
  imports = [ inputs.dir2dir.homeManagerModules.default ];

  dir2dir.trees."~/.config/dir2dir-demo" = {
    greeting = "Hello, world!";
    subdir.message = "Content.\n";
    "link-to-root" = [ "link" "/" ];
    "run.sh" = [ "script" "#!/bin/sh\necho Howdy!" ];
  };
}
```

## Caveats

Honest ones, in the `json2dir` tradition:

- **TOCTOU is reduced, not eliminated.** Files are not written *through*
  pre-existing symlinks and target paths are model-validated, but there is
  still a window between stat and write. Don't run this as root into
  world-writable directories.
- **Non-UTF-8 *names* are rejected.** Non-UTF-8 *content* is fine (raw bytes
  on the copy path, base64 in JSON); non-UTF-8 names are an explicit error.
  This matches the JSON port's fundamental limits.
- **The JSON port drops non-exec mode bits.** Exec vs. non-exec survives;
  `0600` vs. `0644` does not. The native dir → dir path preserves full `0777`
  bits.
- **No hardlinks, FIFOs, sockets, devices, xattrs, ownership.** Directories,
  files, symlinks — like `json2dir`, but with an error instead of a shrug when
  it meets something else.
- **No mtimes are preserved** on rewrite; unchanged files are left untouched,
  which is better anyway.

## When *not* to use this

- You need to copy 4 TB over a network: `rsync`.
- You need an actual archive format: `tar`.
- You need one copy, once: `cp -a`.

You want `dir2dir` when the directory tree is *data* — when you want to diff
it, plan over it, checkpoint it as JSON, materialize it from Nix or CI, and
have the whole pipeline be testable without a filesystem.

## License

GPL-3.0-only — see [LICENSE](LICENSE).
