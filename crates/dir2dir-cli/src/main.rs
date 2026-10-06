//! CLI wiring: parse, compose adapters with use cases, report the plan.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use dir2dir_application::{
    ApplyOptions, CopyDir, CopyOptions, DiffDirs, ExportTree, ImportTree, UseCaseError,
};
use dir2dir_domain::ChangeSet;
use dir2dir_infrastructure::{FsSink, FsSource, JsonCodec};

#[derive(Debug, Parser)]
#[command(
    name = "dir2dir",
    version,
    about = "Directory → directory, through a typed tree model",
    long_about = "Copies directory trees through an explicit, inspectable plan: \
                  capture the source into a model, diff the destination against it, \
                  apply only what changed. Dry-run first, always."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Copy a directory tree (only changed nodes are written)
    #[command(alias = "cp")]
    Copy {
        /// Source directory (captured into the model)
        source: PathBuf,
        /// Destination directory (planned, then mutated)
        destination: PathBuf,
        /// Print the plan without touching the destination
        #[arg(long)]
        dry_run: bool,
        /// Delete destination paths that are absent from the source
        #[arg(long)]
        prune: bool,
    },

    /// Serialize a directory tree to a JSON document on stdout
    #[command(alias = "dump")]
    ToJson {
        /// Directory to capture
        directory: PathBuf,
    },

    /// Materialize a JSON document from stdin into a directory
    #[command(alias = "load")]
    FromJson {
        /// Destination directory
        destination: PathBuf,
        /// Delete destination paths that are absent from the document
        #[arg(long)]
        prune: bool,
    },

    /// Compare two directory trees (exit 0 if identical, 1 if different)
    Diff { left: PathBuf, right: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let source = FsSource;
    let sink = FsSink;
    let codec = JsonCodec;

    match cli.command {
        Command::Copy {
            source: source_root,
            destination,
            dry_run,
            prune,
        } => {
            let options = CopyOptions { dry_run, prune };
            match CopyDir::new(&source, &sink).execute(&source_root, &destination, &options) {
                Ok(plan) => {
                    print_plan(&plan, prune);
                    ExitCode::SUCCESS
                }
                Err(error) => fail(&error),
            }
        }
        Command::ToJson { directory } => {
            match ExportTree::new(&source, &codec).execute(&directory) {
                Ok(document) => {
                    println!("{document}");
                    ExitCode::SUCCESS
                }
                Err(error) => fail(&error),
            }
        }
        Command::FromJson { destination, prune } => {
            let mut document = String::new();
            if let Err(error) = std::io::stdin().read_to_string(&mut document) {
                eprintln!("error: cannot read stdin: {error}");
                return ExitCode::FAILURE;
            }
            let options = ApplyOptions { prune };
            match ImportTree::new(&codec, &sink).execute(&destination, &document, &options) {
                Ok(plan) => {
                    print_plan(&plan, prune);
                    ExitCode::SUCCESS
                }
                Err(error) => fail(&error),
            }
        }
        Command::Diff { left, right } => match DiffDirs::new(&source).execute(&left, &right) {
            Ok(plan) => {
                print_plan(&plan, false);
                if plan.is_empty() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                }
            }
            Err(error) => fail(&error),
        },
    }
}

fn print_plan(plan: &ChangeSet, pruning: bool) {
    for path in plan.created() {
        println!("create     {path}");
    }
    for path in plan.overwritten() {
        println!("overwrite  {path}");
    }
    for path in plan.extraneous() {
        let verb = if pruning { "prune" } else { "extraneous" };
        println!("{verb:<10} {path}");
    }

    let (created, overwritten, extraneous) = plan.counts();
    println!("{created} created, {overwritten} overwritten, {extraneous} extraneous");

    if extraneous > 0 && !pruning {
        eprintln!("note: extraneous paths were left in place; pass --prune to delete them");
    }
}

fn fail(error: &UseCaseError) -> ExitCode {
    eprintln!("error: {error}");
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        eprintln!("  caused by: {cause}");
        source = cause.source();
    }
    ExitCode::FAILURE
}
