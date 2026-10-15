#![allow(unused)]
#![allow(test_only_dead_code)]

// Regression UI: derive/proc-macro expansions must NOT be flagged by
// FULLY_QUALIFIED_PATH_WITHOUT_USE. clap's derive rewrites user-written
// short type tokens (e.g. `String`) into absolute paths while KEEPING the
// original token's span, so `from_expansion()` alone does not fire; the
// syntax-context and snippet-segment-count guards catch this.
//
// Requires `clap = { version = "4", features = ["derive"] }` to compile;
// kept here as a sample since dylint_testing is incompatible with the
// pinned nightly.

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build a dictionary.
    Build {
        /// Prism file.
        prism: String,
        /// Table file.
        table: String,
    },
    /// Query the dictionary.
    Query {
        /// Input text.
        input: String,
    },
}

pub fn direct() {
    std::mem::drop(0);
}
