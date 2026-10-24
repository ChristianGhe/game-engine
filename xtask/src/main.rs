//! `cargo xtask`: the only entry point for checks, CI jobs and agent hooks
//! (spec 7.5, 8.2). CI workflows only check out, cache and call this.
//!
//! This crate is a protected path: agents may not change it without the
//! owner. Agent hooks run a prebuilt copy built from `main`
//! (`cargo xtask install-hooks`), never the working tree.

mod acceptance;
mod arch;
mod arch_scan;
mod ci;
mod docs;
mod glob;
mod guard;
mod guard_bash;
mod guard_code;
mod guard_owner;
mod guard_settings;
mod guard_snapshot;
mod guard_tools;
mod guard_vcs;
mod guard_words;
mod hooks;
mod oracle;
mod replay;
mod selftest;
mod shell;
mod size;
mod util;

use std::process::ExitCode;

const USAGE: &str = "\
cargo xtask quick                      fmt, clippy, arch, size, docs, tests, replays
cargo xtask ci --job <job>             a CI job: lint test riscv wasm selftest all
cargo xtask arch-check                 layering, core-set purity, crate-root attributes
cargo xtask size-check                 warn over 500 lines, fail over 1000
cargo xtask docs-check                 AGENTS.md at most 150 lines, no CLAUDE.md
cargo xtask replay [--regen] [--target T] [--out DIR]
                                       run the testbed scenarios, compare hash streams
cargo xtask compare-hashes DIR DIR...  cross-architecture equality of hash streams
cargo xtask selftest [CASE]            deliberate violations must fail
cargo xtask acceptance DIR             compute acceptance `passes` from CI job reports
cargo xtask oracle-check --base REV --head REV
                                       CI: protected changes need the owner
cargo xtask hook <guard|verify|fmt|stop|session-start>
                                       Claude Code hooks (run the prebuilt copy)
cargo xtask install-hooks [--if-stale] build the hook binary from main";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    // The guard must fail closed: any error, even a bad command line, blocks.
    if let ["hook", "verify", ..] = args.as_slice() {
        return guard_snapshot::verify();
    }
    if let ["hook", "guard", ..] = args.as_slice() {
        return guard::main();
    }
    let result = match args.as_slice() {
        ["quick"] => ci::quick(),
        ["ci", "--job", job] => ci::job(job),
        ["arch-check"] => util::root().and_then(|root| arch::check(&root)),
        ["size-check"] => util::root().and_then(|root| size::check(&root)),
        ["docs-check"] => util::root().and_then(|root| docs::check(&root)),
        ["replay", rest @ ..] => replay::main(rest),
        ["compare-hashes", dirs @ ..] if dirs.len() >= 2 => {
            util::root().and_then(|root| replay::compare_dirs(&root, dirs))
        }
        ["selftest", rest @ ..] => selftest::main(rest),
        ["acceptance", dir] => util::root().and_then(|root| acceptance::compute(&root, dir)),
        ["oracle-check", "--base", base, "--head", head] => oracle::check(base, head),
        ["hook", "fmt"] => hooks::fmt(),
        ["hook", "stop"] => return hooks::stop(),
        ["hook", "session-start"] => hooks::session_start(),
        ["install-hooks"] => hooks::install(false),
        ["install-hooks", "--if-stale"] => hooks::install(true),
        ["help" | "--help" | "-h"] => {
            println!("{USAGE}");
            Ok(String::new())
        }
        _ => Err(format!("unknown command {args:?}\n\n{USAGE}")),
    };
    match result {
        Ok(message) => {
            if !message.is_empty() {
                println!("{message}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
