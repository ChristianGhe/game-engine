//! The guard's checks for programs that run code or write files in ways of
//! their own: shells and interpreters, scripts run by their path, downloads,
//! `sort -o`, and archive tools (ADR-0015).
//!
//! Code the guard cannot see could push or change GitHub, which the second
//! line (`guard_snapshot`) cannot undo, so it is refused: code from a pipe or
//! stdin, and a script written earlier in the same command. Heredoc data
//! read as commands skips these refusals.

use crate::guard::{Decision, expand_home, normalize};
use crate::guard_bash::{Check, SCRIPTS, SHELLS};
use crate::guard_vcs::value;
use crate::shell::Word;

/// Programs that write the file a flag names.
const OUTPUT_FLAGS: [(&str, &[&str]); 3] = [
    ("curl", &["-o", "--output"]),
    ("wget", &["-O", "--output-document"]),
    ("sort", &["-o", "--output"]),
];
/// Script names, so a missing one is not taken for a subcommand.
const SCRIPT_EXTENSIONS: [&str; 11] = [
    ".sh", ".bash", ".zsh", ".py", ".js", ".mjs", ".ts", ".rb", ".pl", ".php", ".lua",
];

/// `(flags that take a value, flags that only print a version or help)`.
fn code_flags(program: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match program {
        p if SHELLS.contains(&p) => (
            &["-o", "-O", "+o", "+O", "--rcfile", "--init-file"],
            &["--version", "--help"],
        ),
        p if p.ends_with("awk") => (&["-F", "-v", "-f"], &["--version", "-V", "--help"]),
        p if p.starts_with("python") => (&["-W", "-X"], &["--version", "-V", "--help", "-h"]),
        "node" | "deno" => (
            &["-r", "--require", "--import", "--loader"],
            &["--version", "-v", "--help", "-h"],
        ),
        _ => (&[], &["--version", "-v", "-V", "--help", "-h"]),
    }
}

impl Check<'_> {
    /// Shells and interpreters: the code they run, from `-c`/`-e`, a heredoc
    /// or a file.
    pub(crate) fn code(&self, program: &str, args: &[&Word], heredoc: &str) -> Option<Decision> {
        let (with_value, info) = code_flags(program);
        let info = args.iter().any(|w| info.contains(&w.text.as_str()));
        let stdin = args
            .iter()
            .any(|w| w.text == "-" || (w.text == "-s" && SHELLS.contains(&program)));
        let operand = script_operand(args, with_value);
        let unseen = |what: Decision| (!self.lenient).then_some(what);
        if SHELLS.contains(&program) {
            if let Some(code) = inline_code(args, 'c') {
                return self.nested(&code);
            }
            if !heredoc.is_empty() {
                return self.nested(heredoc);
            }
            return match operand {
                Some(w) if !stdin => self.script_file(w),
                _ if info => None,
                _ => unseen(stdin_code(program)),
            };
        }
        let mut code = String::from(heredoc);
        let letter = if program.starts_with("python") {
            'c'
        } else {
            'e'
        };
        if program.ends_with("awk") {
            match value(args, "-f") {
                Some(file) => match self.read_file(&file) {
                    Some(text) => code.push_str(&text),
                    None if self.named_earlier(&file) => return unseen(unreadable(&file)),
                    None => {}
                },
                None => code.push_str(operand.map_or("", |w| w.text.as_str())),
            }
        } else if let Some(inline) = inline_code(args, letter) {
            code.push_str(&inline);
        } else if args.iter().any(|w| w.text == "-m") {
            // A module: out of reach, like any installed program.
        } else if let Some(w) = operand.filter(|_| !stdin) {
            let file = self.expand(w).unwrap_or_else(|| w.text.clone());
            match self.read_file(&file) {
                Some(text) => code.push_str(&text),
                None if looks_like_script(&file) && self.named_earlier(&file) => {
                    return unseen(unreadable(&file));
                }
                None => {}
            }
        } else if heredoc.is_empty() && !info {
            return unseen(stdin_code(program));
        }
        self.opaque(program, &code)
    }

    /// A shell script: checked as commands. One that cannot be read yet but
    /// is named earlier in the same command is being written to run unseen.
    pub(crate) fn script_file(&self, w: &Word) -> Option<Decision> {
        let raw = self.expand(w).unwrap_or_else(|| w.text.clone());
        match self.read_file(&raw) {
            Some(text) => self.nested(&text),
            None if !self.lenient && self.named_earlier(&raw) => Some(unreadable(&raw)),
            None => None,
        }
    }

    /// A program run by its path: a text file is a script, checked with its
    /// `#!` interpreter (`sh` without one). A binary is out of reach, and a
    /// file that does not exist yet is refused only if this command writes
    /// it first.
    pub(crate) fn run_file(&self, w: &Word) -> Option<Decision> {
        if self.lenient {
            return None;
        }
        let raw = self.expand(w)?;
        let Some(text) = self.read_file(&raw) else {
            return self.named_earlier(&raw).then(|| unreadable(&raw));
        };
        let shebang: Vec<&str> = text
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("#!"))
            .map(|l| l.split_whitespace().collect())
            .unwrap_or_default();
        let mut parts = shebang.iter().map(|p| p.rsplit('/').next().unwrap_or(p));
        let mut interpreter = parts.next().unwrap_or("sh");
        if interpreter == "env" {
            interpreter = parts.find(|p| !p.starts_with('-')).unwrap_or("sh");
        }
        let interpreter = interpreter.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        if SHELLS.contains(&interpreter) {
            self.nested(&text)
        } else if SCRIPTS.contains(&interpreter) {
            self.opaque(interpreter, &text)
        } else {
            None
        }
    }

    /// Whether an earlier piece of this command names the path `raw`.
    pub(crate) fn named_earlier(&self, raw: &str) -> bool {
        let Some(cwd) = &self.cwd else {
            return true;
        };
        let path = normalize(&cwd.join(expand_home(raw)));
        self.earlier.iter().any(|e| {
            e.words.iter().any(|w| {
                e.cwd
                    .as_ref()
                    .is_some_and(|c| normalize(&c.join(expand_home(w))) == path)
            })
        })
    }

    /// Whether an earlier piece of this command ran `words`, such as
    /// `git stash`.
    pub(crate) fn ran_earlier(&self, words: &[&str]) -> bool {
        self.earlier.iter().any(|e| {
            e.words
                .windows(words.len())
                .any(|w| w.iter().zip(words).all(|(a, b)| a == b))
        })
    }

    /// Downloads and `sort -o` write the file a flag names; archive tools
    /// extracting inside the repo can overwrite any file in it.
    pub(crate) fn writers(&self, program: &str, args: &[&Word]) -> Option<Decision> {
        let outputs = self.outputs(program, args);
        if let Some(what) = outputs.iter().find_map(|o| self.target(o)) {
            return Some(Decision::Block(format!(
                "{program} would write protected path {what}"
            )));
        }
        if self.lenient || !extracts(program, args) {
            return None;
        }
        let dir = ["-C", "--directory", "-d", "-D"]
            .iter()
            .find_map(|f| value(args, f))
            .or_else(|| {
                args.iter()
                    .find_map(|w| w.text.strip_prefix("-o").map(str::to_string))
            })
            .unwrap_or_default();
        let dir = if dir.is_empty() { ".".to_string() } else { dir };
        let inside = match self.expand(&word(&dir)) {
            Some(dir) => self.inside_repo(&dir),
            None => self.unknown(&dir).is_some(),
        };
        inside.then(|| {
            Decision::Block(format!(
                "{program} extracting inside the repo can overwrite protected files; extract to a temporary directory"
            ))
        })
    }

    /// The files a download or `sort -o` writes.
    fn outputs(&self, program: &str, args: &[&Word]) -> Vec<Word> {
        let mut out: Vec<String> = Vec::new();
        if let Some((_, flags)) = OUTPUT_FLAGS.iter().find(|(p, _)| *p == program) {
            let short: Vec<char> = flags
                .iter()
                .filter(|f| f.len() == 2)
                .filter_map(|f| f.chars().nth(1))
                .collect();
            for (i, w) in args.iter().enumerate() {
                let t = w.text.as_str();
                let next = || args.get(i + 1).map(|v| v.text.clone());
                if flags.contains(&t) {
                    out.extend(next());
                } else if let Some(v) = flags
                    .iter()
                    .filter(|f| f.starts_with("--"))
                    .find_map(|f| t.strip_prefix(f)?.strip_prefix('='))
                {
                    out.push(v.to_string());
                } else if t.starts_with('-') && !t.starts_with("--") {
                    // A cluster such as `-sSLo file` or `-qOfile`.
                    for letter in &short {
                        if let Some(at) = t[1..].find(*letter) {
                            let rest = &t[1 + at + 1..];
                            if rest.is_empty() {
                                out.extend(next());
                            } else {
                                out.push(rest.to_string());
                            }
                        }
                    }
                }
            }
        }
        // `curl -O` and plain `wget` name the file after the URL.
        let by_url = match program {
            "curl" => args.iter().any(|w| {
                matches!(w.text.as_str(), "--remote-name" | "--remote-name-all")
                    || (w.text.starts_with('-')
                        && !w.text.starts_with("--")
                        && w.text.contains('O'))
            }),
            "wget" => out.is_empty(),
            _ => false,
        };
        if by_url {
            let dir = value(args, "--output-dir")
                .or_else(|| value(args, "-P"))
                .or_else(|| value(args, "--directory-prefix"))
                .unwrap_or_else(|| ".".into());
            for url in args.iter().filter(|w| w.text.contains("://")) {
                let path = url.text.split(['?', '#']).next().unwrap_or("");
                let file = path.rsplit('/').next().filter(|f| !f.is_empty());
                out.push(format!("{dir}/{}", file.unwrap_or("index.html")));
            }
        }
        out.iter().map(|o| word(o)).collect()
    }
}

/// Whether an archive tool's arguments extract files.
fn extracts(program: &str, args: &[&Word]) -> bool {
    let short = |w: &Word, c: char| {
        w.text.starts_with('-') && !w.text.starts_with("--") && w.text.contains(c)
    };
    match program {
        "tar" | "bsdtar" | "gtar" => {
            args.iter()
                .any(|w| short(w, 'x') || matches!(w.text.as_str(), "--extract" | "--get"))
                || args
                    .first()
                    .is_some_and(|w| !w.text.starts_with('-') && w.text.contains('x'))
        }
        "unzip" => !args
            .iter()
            .any(|w| matches!(w.text.as_str(), "-l" | "-t" | "-v" | "-p" | "-Z" | "-z")),
        "cpio" => args.iter().any(|w| short(w, 'i') || w.text == "--extract"),
        "7z" | "7za" | "7zz" => args
            .first()
            .is_some_and(|w| matches!(w.text.as_str(), "x" | "e")),
        _ => false,
    }
}

/// A word made from text the guard pieced together.
fn word(text: &str) -> Word {
    Word {
        text: text.to_string(),
        quoted: false,
        dynamic: text.contains(['$', '`', '*', '?', '[', '{']),
        op: false,
    }
}

/// The script a shell or interpreter runs: its first operand.
fn script_operand<'a>(args: &[&'a Word], with_value: &[&str]) -> Option<&'a Word> {
    let mut i = 0;
    while i < args.len() {
        let t = args[i].text.as_str();
        if t == "--" {
            return args.get(i + 1).copied();
        }
        if !(t.starts_with('-') || t.starts_with('+')) {
            return Some(args[i]);
        }
        i += if with_value.contains(&t) { 2 } else { 1 };
    }
    None
}

/// The code after a short-flag cluster containing `letter` (`-c`, `-ec`, `-Ic`).
fn inline_code(args: &[&Word], letter: char) -> Option<String> {
    let at = args.iter().position(|w| {
        w.text.starts_with('-') && !w.text.starts_with("--") && w.text[1..].contains(letter)
    })?;
    args.get(at + 1).map(|w| w.text.clone())
}

fn looks_like_script(text: &str) -> bool {
    text.contains('/') || SCRIPT_EXTENSIONS.iter().any(|e| text.ends_with(e))
}

fn unreadable(raw: &str) -> Decision {
    Decision::Block(format!(
        "the guard cannot read the script {raw}, which this command writes first; write it in an earlier command, then run it"
    ))
}

fn stdin_code(program: &str) -> Decision {
    Decision::Block(format!(
        "{program} reading code from a pipe or stdin: the guard cannot see it; write the code to a file first"
    ))
}
