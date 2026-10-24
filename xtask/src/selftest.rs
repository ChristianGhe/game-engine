//! `cargo xtask selftest` (rung 1a, done-when 3): each deliberate violation
//! must fail. Cases run against a fresh copy of the workspace under
//! `target/selftest/`, or against the real guard binary, never the tree itself.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::arch;
use crate::util::{self, Steps};

pub const CASES: [&str; 17] = [
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
];

pub fn main(args: &[&str]) -> Result<String, String> {
    let root = util::root()?;
    let mut steps = Steps::default();
    match args {
        [] => steps_all(&root, &mut steps),
        [case] if CASES.contains(case) => {
            steps.run(&format!("violation:{case}"), || run_case(&root, case));
        }
        _ => return Err(format!("selftest cases: {}", CASES.join(" "))),
    }
    if steps.ok() {
        Ok(format!("selftest: {} cases ok", steps.results.len()))
    } else {
        Err("selftest: a deliberate violation was not caught".into())
    }
}

pub fn steps(root: &Path, steps: &mut Steps) {
    steps_all(root, steps);
}

fn steps_all(root: &Path, steps: &mut Steps) {
    for case in CASES {
        steps.run(&format!("violation:{case}"), || run_case(root, case));
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
