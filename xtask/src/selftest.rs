//! `cargo xtask selftest` (rung 1a, done-when 3; rung 1b, criterion 12): each
//! deliberate violation must fail. Cases run against a fresh copy of the
//! workspace under `target/selftest/`, or against the real guard binary, and
//! never change the tree itself. `golden-edit` runs the real shots harness
//! against goldens copied there; like every run of that harness (`quick`'s
//! too), it empties and rewrites `target/golden-candidates/`. A case that
//! needs what this machine lacks (cargo-deny, the pinned lavapipe) says it
//! could not run: a failure only on CI or with `ENG_REQUIRE_GPU=1`, as for
//! GPU tests, and never a pass (`selftest_verdict.rs`).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::arch;
use crate::util::{self, Steps};

#[path = "selftest_verdict.rs"]
mod verdict;
pub use verdict::NOT_RUN;
use verdict::{judged, last_line, outcome, status, strict};

pub const CASES: [&str; 22] = [
    "float-sin",
    "float-min",
    "allow-escape",
    "core-std-dep",
    "extern-std",
    "crate-clippy-toml",
    "no-std-removed",
    "protected-edit",
    "guard-cd-bypass",
    "guard-heredoc",
    "guard-herestring",
    "guard-quoted-substitution",
    "guard-unlock-write",
    "guard-push",
    "guard-jj",
    "bless",
    "guard-allows-ordinary-edits",
    // Rung 1b, criterion 12 (`violations-1b`).
    "wgpu-outside-gpu",
    "winit-outside-platform",
    "rwh-outside-seam",
    "core-depends-on-gpu",
    "golden-edit",
];

/// `cargo xtask selftest [CASE]`: one line per case, then the counts.
pub fn main(args: &[&str]) -> Result<String, String> {
    let root = util::root()?;
    let cases: Vec<&str> = match args {
        [] => CASES.to_vec(),
        [case] if CASES.contains(case) => vec![*case],
        _ => return Err(format!("selftest cases: {}", CASES.join(" "))),
    };
    let strict = strict();
    let mut outcomes = Vec::new();
    for case in cases {
        let name = format!("violation:{case}");
        let started = Instant::now();
        let result = outcome(run_case(&root, case), strict);
        let seconds = started.elapsed().as_secs_f64();
        println!("{name:.<40} {} ({seconds:.1}s)", status(&result));
        if let Err(detail) = &result {
            println!("{detail}");
        }
        outcomes.push((name, result));
    }
    last_line(&outcomes, strict)
}

/// `ci --job selftest` and `all`: each case is a step of the job's report.
/// A case not run here is pending: never ok, so acceptance never counts
/// it, and it fails no job. In a strict run it fails the job.
pub fn steps(root: &Path, steps: &mut Steps) {
    let strict = strict();
    for case in CASES {
        steps.run_judged(&format!("violation:{case}"), || {
            judged(outcome(run_case(root, case), strict))
        });
    }
}

fn run_case(root: &Path, case: &str) -> Result<String, String> {
    match case {
        "float-sin" => clippy_rejects(
            root,
            case,
            "pub fn selftest(x: f32) -> f32 {\n    x.sin()\n}\n",
            &["no method named `sin`", "disallowed method"],
        ),
        "float-min" => clippy_rejects(
            root,
            case,
            "pub fn selftest(x: f32) -> f32 {\n    x.min(1.0)\n}\n",
            &["disallowed method `f32::min`"],
        ),
        "allow-escape" => clippy_rejects(
            root,
            case,
            "#[allow(clippy::style)]\npub fn selftest(x: f32) -> f32 {\n    x.max(1.0)\n}\n",
            &["incompatible with previous forbid"],
        ),
        "extern-std" => arch_rejects(root, case, "extern crate std", |ws| {
            edit(
                ws,
                "crates/eng-math/src/lib.rs",
                "mod angle;\n",
                "extern crate std;\nmod angle;\n",
            )
        }),
        "crate-clippy-toml" => arch_rejects(root, case, "[E-ARCH-OVERRIDE]", |ws| {
            fs::write(
                ws.join("crates/eng-math/clippy.toml"),
                "disallowed-methods = []\n",
            )
            .map_err(|e| e.to_string())
        }),
        "guard-cd-bypass" => guard_blocks(
            root,
            case,
            "Bash",
            serde_json::json!({ "command": "cd crates && echo x >> ../deny.toml" }),
        ),
        "guard-heredoc" => guard_blocks(
            root,
            case,
            "Bash",
            serde_json::json!({ "command": "python3 - <<'EOF'\nopen('deny.toml', 'a').write('x')\nEOF" }),
        ),
        "guard-herestring" => guard_blocks(
            root,
            case,
            "Bash",
            serde_json::json!({ "command": "cat <<<x\necho x >> deny.toml\nx" }),
        ),
        "guard-quoted-substitution" => guard_blocks(
            root,
            case,
            "Bash",
            serde_json::json!({ "command": "echo \"it's $(echo x >> deny.toml)\"" }),
        ),
        "guard-unlock-write" => guard_blocks(
            root,
            case,
            "Bash",
            serde_json::json!({ "command": "cd .claude && printf '*' > guard-unlock" }),
        ),
        "guard-push" => guard_blocks(
            root,
            case,
            "Bash",
            serde_json::json!({ "command": "git -C . push origin HEAD:main" }),
        ),
        "guard-jj" => guard_blocks(
            root,
            case,
            "Bash",
            serde_json::json!({ "command": "jj restore --from 'root()' deny.toml" }),
        ),
        "core-std-dep" => arch_rejects(root, case, "[E-ARCH-CORE-DEP]", |ws| {
            edit(
                ws,
                "crates/eng-core/Cargo.toml",
                "[dependencies]\n",
                "[dependencies]\neng-schema-json.workspace = true\n",
            )
        }),
        "no-std-removed" => arch_rejects(root, case, "[E-ARCH-ROOT-ATTR]", |ws| {
            edit(ws, "crates/eng-ecs/src/lib.rs", "#![no_std]\n", "")
        }),
        "protected-edit" => {
            guard_blocks(
                root,
                case,
                "Edit",
                serde_json::json!({ "file_path": "deny.toml", "old_string": "a", "new_string": "b" }),
            )?;
            guard_blocks(
                root,
                case,
                "Bash",
                serde_json::json!({ "command": "echo '[bans]' >> clippy.toml" }),
            )
        }
        "bless" => guard_blocks(
            root,
            case,
            "Bash",
            serde_json::json!({ "command": "cargo insta accept" }),
        ),
        // Rung 1b, criterion 12. A crate can name a floor crate only by
        // depending on it, so each case adds the dependency; arch-check's
        // `[floor]` and core-set rules and cargo-deny's wrapper ban
        // (ADR-0016) read dependencies.
        "wgpu-outside-gpu" => arch_rejects(
            root,
            case,
            "[E-ARCH-FLOOR] eng-render depends on wgpu; only eng-gpu may",
            |ws| add_dependency(ws, "eng-render", "wgpu"),
        ),
        "winit-outside-platform" => arch_rejects(
            root,
            case,
            "[E-ARCH-FLOOR] eng-app depends on winit; only eng-platform-winit may",
            |ws| add_dependency(ws, "eng-app", "winit"),
        ),
        "rwh-outside-seam" => deny_rejects(root, case, ("raw-window-handle", "eng-render"), |ws| {
            add_dependency(ws, "eng-render", "raw-window-handle")
        }),
        "core-depends-on-gpu" => arch_rejects(
            root,
            case,
            "[E-ARCH-CORE-DEP] core crate eng-sim reaches eng-gpu (via eng-sim)",
            |ws| add_dependency(ws, "eng-sim", "eng-gpu"),
        ),
        "golden-edit" => crate::selftest_golden::run(root),
        "guard-allows-ordinary-edits" => {
            let ws = copy_workspace(root, case)?;
            let input = serde_json::json!({
                "tool_name": "Edit",
                "tool_input": { "file_path": "crates/eng-core/src/lib.rs", "old_string": "a", "new_string": "b" },
                "cwd": ws,
            });
            let (code, stderr) = run_guard(&ws, &input)?;
            if code == 0 {
                Ok("ordinary edits pass".into())
            } else {
                Err(format!(
                    "guard blocked an ordinary edit (exit {code}): {stderr}"
                ))
            }
        }
        _ => Err(format!("unknown case {case}")),
    }
}

/// A fresh copy of the workspace sources (no `target`, no `.git`).
fn copy_workspace(root: &Path, case: &str) -> Result<PathBuf, String> {
    let ws = root.join("target/selftest").join(case);
    let _ = fs::remove_dir_all(&ws);
    for dir in ["crates", "xtask"] {
        for file in util::walk(&root.join(dir)) {
            copy(root, &ws, &file)?;
        }
    }
    for file in [
        "Cargo.toml",
        "Cargo.lock",
        "clippy.toml",
        "rust-toolchain.toml",
        "rustfmt.toml",
        "architecture.toml",
        "protected.toml",
        "budgets.toml",
        "deny.toml",
    ] {
        if root.join(file).exists() {
            copy(root, &ws, &root.join(file))?;
        }
    }
    // The xtask alias lives in a hidden directory.
    if root.join(".cargo/config.toml").exists() {
        copy(root, &ws, &root.join(".cargo/config.toml"))?;
    }
    Ok(ws)
}

fn copy(root: &Path, ws: &Path, file: &Path) -> Result<(), String> {
    let dest = ws.join(file.strip_prefix(root).map_err(|e| e.to_string())?);
    fs::create_dir_all(dest.parent().expect("has parent")).map_err(|e| e.to_string())?;
    fs::copy(file, &dest)
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", file.display()))
}

fn edit(ws: &Path, rel: &str, from: &str, to: &str) -> Result<(), String> {
    let path = ws.join(rel);
    let text = fs::read_to_string(&path).map_err(|e| format!("{rel}: {e}"))?;
    if !text.contains(from) {
        return Err(format!("selftest setup: {rel} does not contain {from:?}"));
    }
    fs::write(&path, text.replacen(from, to, 1)).map_err(|e| format!("{rel}: {e}"))
}

/// `crates/<krate>` gains `<dep>.workspace = true` as a normal dependency.
fn add_dependency(ws: &Path, krate: &str, dep: &str) -> Result<(), String> {
    edit(
        ws,
        &format!("crates/{krate}/Cargo.toml"),
        "[dependencies]\n",
        &format!("[dependencies]\n{dep}.workspace = true\n"),
    )
}

fn clippy_rejects(
    root: &Path,
    case: &str,
    code: &str,
    expect_any: &[&str],
) -> Result<String, String> {
    let ws = copy_workspace(root, case)?;
    let lib = ws.join("crates/eng-math/src/lib.rs");
    let mut text = fs::read_to_string(&lib).map_err(|e| e.to_string())?;
    text.push('\n');
    text.push_str(code);
    fs::write(&lib, text).map_err(|e| e.to_string())?;
    let out = util::cargo(&ws)
        .args(["clippy", "-p", "eng-math", "--target-dir"])
        .arg(root.join("target/selftest/target"))
        .output()
        .map_err(|e| e.to_string())?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    match (
        out.status.success(),
        expect_any.iter().find(|e| stderr.contains(*e)),
    ) {
        (false, Some(found)) => Ok(format!("rejected: {found}")),
        (false, None) => Err(format!(
            "failed, but not with {expect_any:?}:\n{}",
            util::tail(&stderr, 15)
        )),
        (true, _) => Err("clippy accepted the violation".into()),
    }
}

fn arch_rejects(
    root: &Path,
    case: &str,
    tag: &str,
    mutate: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<String, String> {
    let ws = copy_workspace(root, case)?;
    mutate(&ws)?;
    match arch::check_unlocked(&ws) {
        Err(found) if found.contains(tag) => Ok(format!("rejected: {tag}")),
        Err(found) => Err(format!("failed, but without {tag}:\n{found}")),
        Ok(_) => Err("arch-check accepted the violation".into()),
    }
}

fn run_guard(project: &Path, input: &serde_json::Value) -> Result<(i32, String), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new(exe)
        .args(["hook", "guard"])
        .env("CLAUDE_PROJECT_DIR", project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .expect("piped")
        .write_all(input.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    Ok((
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    ))
}

fn guard_blocks(
    root: &Path,
    case: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    // A copy has no owner unlock file, so the result never depends on it.
    let ws = root.join("target/selftest").join(case);
    let _ = fs::remove_dir_all(&ws);
    fs::create_dir_all(&ws).map_err(|e| e.to_string())?;
    for file in [
        "architecture.toml",
        "Cargo.toml",
        "deny.toml",
        "clippy.toml",
    ] {
        let _ = fs::copy(root.join(file), ws.join(file));
    }
    let input = serde_json::json!({ "tool_name": tool, "tool_input": args, "cwd": ws });
    let (code, stderr) = run_guard(&ws, &input)?;
    if code == 2 && stderr.contains("[E-GUARD]") {
        Ok(format!("{tool} blocked"))
    } else {
        Err(format!(
            "guard let {tool} {args} through (exit {code}): {stderr}"
        ))
    }
}

/// Whether `cargo deny check bans` refused `banned` because `parent`, its
/// direct user, is not one of its wrappers.
fn deny_verdict(
    succeeded: bool,
    output: &str,
    banned: &str,
    parent: &str,
) -> Result<String, String> {
    let ban = format!("error[banned]: crate '{banned} = ");
    let user = format!("direct parent '{parent} = ");
    let of = format!("of banned crate '{banned} = ");
    let named = output
        .lines()
        .any(|line| line.contains(&user) && line.contains(&of));
    if succeeded {
        Err("cargo-deny accepted the violation".into())
    } else if output.contains(&ban) && named {
        Ok(format!(
            "rejected: error[banned] {banned}, used by {parent}"
        ))
    } else {
        Err(format!(
            "failed, but not with error[banned] for {banned} used by {parent}:\n{}",
            util::tail(output, 15)
        ))
    }
}

/// `cargo deny check bans` on a fresh copy changed by `mutate`: it must
/// refuse `banned` because `parent` is not one of its wrappers.
fn deny_rejects(
    root: &Path,
    case: &str,
    (banned, parent): (&str, &str),
    mutate: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<String, String> {
    let ws = copy_workspace(root, case)?;
    mutate(&ws)?;
    let installed = util::cargo(&ws)
        .args(["deny", "--version"])
        .output()
        .is_ok_and(|o| o.status.success());
    if !installed {
        return Err(format!(
            "{NOT_RUN} {case} could not run: cargo-deny is not installed \
             (cargo install --locked cargo-deny@0.20.2, as CI's lint job has it)"
        ));
    }
    let out = util::cargo(&ws)
        .args(["deny", "check", "bans"])
        .output()
        .map_err(|e| format!("cannot run cargo-deny: {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    deny_verdict(out.status.success(), &text, banned, parent)
}

#[cfg(test)]
#[path = "selftest_tests.rs"]
mod tests;
