//! The guard's Bash checks: could this command write a protected path, or
//! bless expected output?
//!
//! It follows `cd` (and subshells), literal variables, globs, heredocs,
//! `sh -c` code, wrappers such as `env` and `timeout`, and git's global
//! options. What it cannot resolve inside the repo, it refuses.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::glob;
use crate::guard::{Ctx, Decision, expand_home, normalize};
use crate::guard_words::{Var, assignments, bless, command_words, plain, unwrap};
use crate::shell::{self, Piece, Word};

/// Programs that write every path argument.
const WRITE_ALL: [&str; 11] = [
    "tee", "rm", "unlink", "shred", "truncate", "chmod", "chown", "mv", "ln", "touch", "mkfifo",
];
/// Programs that write their last path argument.
const WRITE_LAST: [&str; 3] = ["cp", "install", "rsync"];
const SHELLS: [&str; 6] = ["sh", "bash", "zsh", "dash", "ksh", "fish"];
const SCRIPTS: [&str; 12] = [
    "python",
    "python3",
    "node",
    "deno",
    "perl",
    "ruby",
    "php",
    "osascript",
    "awk",
    "gawk",
    "mawk",
    "lua",
];
/// Signs that unparsed code writes files.
const WRITE_HINTS: [&str; 20] = [
    "write",
    "open(",
    "'w'",
    "\"w\"",
    "'a'",
    "\"a\"",
    "unlink",
    "remove",
    "rename",
    "truncate",
    "chmod",
    "copy",
    "move",
    "rmtree",
    "system",
    "popen",
    "subprocess",
    "spawn",
    ">",
    "sed",
];

pub(crate) struct Check<'a> {
    pub ctx: &'a Ctx<'a>,
    pub cwd: Option<PathBuf>,
    pub vars: &'a BTreeMap<String, Var>,
    pub depth: u32,
}

impl Ctx<'_> {
    pub fn bash(&self, command: &str) -> Decision {
        self.bash_at(command, Some(self.cwd.to_path_buf()), 0)
    }

    fn bash_at(&self, command: &str, cwd: Option<PathBuf>, depth: u32) -> Decision {
        let pieces = shell::split(command);
        if let Some(found) = bless(&pieces) {
            if self.unlocked.iter().any(|l| l == "bless") {
                return Decision::Allow;
            }
            return Decision::Block(format!(
                "refusing a bless/accept command ({found}): updating snapshots, goldens or expected output"
            ));
        }
        if self.all_unlocked() {
            return Decision::Allow;
        }
        if depth > 4 {
            return Decision::Block("the command nests too deeply to check".into());
        }
        for inner in shell::substitutions(command) {
            if let block @ Decision::Block(_) = self.bash_at(&inner, cwd.clone(), depth + 1) {
                return block;
            }
        }
        let vars = assignments(&pieces);
        let mut dirs: Vec<Option<PathBuf>> = vec![cwd];
        for piece in pieces {
            match piece {
                Piece::Open => dirs.push(dirs.last().cloned().flatten()),
                Piece::Close => {
                    if dirs.len() > 1 {
                        dirs.pop();
                    }
                }
                Piece::Command { words, heredoc } => {
                    let words = command_words(words);
                    let check = Check {
                        ctx: self,
                        cwd: dirs.last().cloned().flatten(),
                        vars: &vars,
                        depth,
                    };
                    if let Some(first) = words.first()
                        && matches!(first.text.as_str(), "cd" | "pushd" | "popd")
                    {
                        let next = match (first.text.as_str(), words.get(1)) {
                            ("popd", _) => None,
                            (_, None) => std::env::var_os("HOME").map(PathBuf::from),
                            (_, Some(w)) if w.text == "-" => None,
                            (_, Some(w)) => match (check.expand(w), &check.cwd) {
                                (Some(dir), Some(cwd)) => Some(normalize(&cwd.join(dir))),
                                (Some(dir), None) if Path::new(&dir).is_absolute() => {
                                    Some(PathBuf::from(dir))
                                }
                                _ => None,
                            },
                        };
                        *dirs.last_mut().expect("never empty") = next;
                        continue;
                    }
                    if let Some(decision) = check.command(&words, &heredoc) {
                        return decision;
                    }
                }
            }
        }
        Decision::Allow
    }
}

impl Check<'_> {
    /// The word's value with literal variables substituted; `None` if it
    /// depends on something the guard cannot know.
    fn expand(&self, w: &Word) -> Option<String> {
        let text = expand_home(&w.text);
        if !w.dynamic || !text.contains('$') {
            return Some(text);
        }
        if text.contains("$(") || text.contains('`') {
            return None;
        }
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '$' {
                out.push(c);
                continue;
            }
            let braced = chars.peek() == Some(&'{');
            if braced {
                chars.next();
            }
            let mut name = String::new();
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphanumeric() || n == '_' {
                    name.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            if braced && chars.next() != Some('}') {
                return None;
            }
            if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
                return None;
            }
            match self.vars.get(&name) {
                Some(Var::Value(v)) => out.push_str(v),
                Some(Var::Temp) => {
                    out.push_str(&std::env::temp_dir().join("mktemp").to_string_lossy())
                }
                Some(Var::Unknown) => return None,
                None => out.push_str(&std::env::var(&name).unwrap_or_default()),
            }
        }
        Some(out)
    }

    /// A description of the guarded path `w` may name, if any.
    pub(crate) fn target(&self, w: &Word) -> Option<String> {
        let Some(text) = self.expand(w) else {
            return self.unknown(&w.text);
        };
        let absolute = Path::new(&text).is_absolute();
        let base = match (&self.cwd, absolute) {
            (_, true) => PathBuf::from("/"),
            (Some(cwd), false) => cwd.clone(),
            (None, false) => return self.unknown(&text),
        };
        if w.dynamic && text.contains(['*', '?', '[']) {
            if text.contains('[') {
                return self.unknown(&text);
            }
            let matches = expand_glob(&base, &text);
            if let Some(hit) = matches
                .iter()
                .find_map(|p| self.ctx.guarded_path(&base, &p.to_string_lossy()))
            {
                return Some(hit);
            }
            if !matches.is_empty() {
                return None;
            }
        }
        self.ctx.guarded_path(&base, &text)
    }

    /// A target the guard cannot resolve: refused if it may land in the repo.
    fn unknown(&self, text: &str) -> Option<String> {
        let literal: String = text
            .chars()
            .take_while(|c| !"$`*?[{".contains(*c))
            .collect();
        let dir = literal
            .rsplit_once('/')
            .map_or(".", |(d, _)| if d.is_empty() { "/" } else { d });
        let inside = match &self.cwd {
            Some(cwd) => self.ctx.resolve_in(cwd, dir).is_some(),
            None => {
                !Path::new(dir).is_absolute() || self.ctx.resolve_in(self.ctx.root, dir).is_some()
            }
        };
        inside.then(|| {
            format!("`{text}` (the guard cannot tell what it names; use an explicit path)")
        })
    }

    pub(crate) fn inside_repo(&self, raw: &str) -> bool {
        self.cwd
            .as_ref()
            .is_none_or(|cwd| self.ctx.resolve_in(cwd, raw).is_some())
    }

    fn block(what: String) -> Option<Decision> {
        Some(Decision::Block(format!(
            "Bash would write protected path {what}"
        )))
    }

    fn command(&self, words: &[Word], heredoc: &str) -> Option<Decision> {
        for (i, w) in words.iter().enumerate() {
            let write = w.op && {
                let op = w
                    .text
                    .trim_start_matches(|c: char| c.is_ascii_digit() || c == '&');
                matches!(op, ">" | ">>" | ">|" | "<>")
            };
            if write && let Some(what) = words.get(i + 1).and_then(|t| self.target(t)) {
                return Self::block(what);
            }
        }
        let words_plain = plain(words);
        let (_, program, args) = unwrap(&words_plain);
        let program = program?;
        let flag = |w: &&Word| w.text.starts_with('-') && w.text != "-";
        let paths: Vec<&Word> = args.iter().copied().filter(|w| !flag(w)).collect();
        let written: Vec<&Word> = match program.as_str() {
            p if WRITE_ALL.contains(&p) => paths.clone(),
            "cp" if args
                .iter()
                .any(|a| matches!(a.text.as_str(), "-l" | "--link" | "-s" | "--symbolic-link")) =>
            {
                paths.clone()
            }
            p if WRITE_LAST.contains(&p) => paths.last().copied().into_iter().collect(),
            "sed" | "gsed" | "perl" | "ruby" if args.iter().any(|a| in_place_flag(&a.text)) => {
                paths.clone()
            }
            _ => Vec::new(),
        };
        if let Some(what) = written.iter().find_map(|w| self.target(w)) {
            return Self::block(what);
        }
        if program == "dd"
            && let Some(what) = args
                .iter()
                .filter_map(|a| a.text.strip_prefix("of="))
                .find_map(|t| {
                    self.target(&Word {
                        text: t.to_string(),
                        quoted: false,
                        dynamic: false,
                        op: false,
                    })
                })
        {
            return Self::block(what);
        }
        match program.as_str() {
            "git" => self.git(&args, heredoc),
            "patch" => self.patch_text(&args, heredoc),
            "find" => self.find(&args),
            "xargs" => self.xargs(&args),
            "eval" => self.nested(
                &args
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            "source" | "." => self.script_file(paths.first().copied()),
            p if SHELLS.contains(&p) => {
                if let Some(code) = inline_code(&args, 'c') {
                    return self.nested(&code);
                }
                if !heredoc.is_empty() {
                    return self.nested(heredoc);
                }
                self.script_file(paths.first().copied())
            }
            p if SCRIPTS.contains(&p) => {
                let mut code = String::from(heredoc);
                if p.ends_with("awk") {
                    code.push_str(paths.first().map_or("", |w| w.text.as_str()));
                } else if let Some(inline) =
                    inline_code(&args, if p.starts_with("python") { 'c' } else { 'e' })
                {
                    code.push_str(&inline);
                } else if let Some(file) = paths.first().and_then(|w| self.read_file(&w.text)) {
                    code.push_str(&file);
                }
                self.opaque(&program, &code)
            }
            _ => None,
        }
    }

    fn nested(&self, code: &str) -> Option<Decision> {
        match self.ctx.bash_at(code, self.cwd.clone(), self.depth + 1) {
            Decision::Allow => None,
            block => Some(block),
        }
    }

    pub(crate) fn read_file(&self, raw: &str) -> Option<String> {
        let path = self.cwd.as_ref()?.join(expand_home(raw));
        fs::read_to_string(path).ok()
    }

    fn script_file(&self, word: Option<&Word>) -> Option<Decision> {
        let text = self.read_file(&word?.text)?;
        self.nested(&text)
    }

    /// Code in another language: refused when it both names a protected path
    /// and looks like it writes files.
    fn opaque(&self, program: &str, code: &str) -> Option<Decision> {
        let code = code.to_lowercase();
        let fragment = self
            .ctx
            .rules
            .fragments()
            .into_iter()
            .find(|f| code.contains(f.as_str()))?;
        WRITE_HINTS.iter().any(|h| code.contains(h)).then(|| {
            Decision::Block(format!(
                "{program} code writes and names a protected path ({fragment})"
            ))
        })
    }
}

/// Whether `program` (with `args`) writes files or runs code.
pub(crate) fn writer(program: &str, args: &[&Word]) -> bool {
    let p = program.rsplit('/').next().unwrap_or(program);
    WRITE_ALL.contains(&p)
        || WRITE_LAST.contains(&p)
        || SHELLS.contains(&p)
        || SCRIPTS.contains(&p)
        || matches!(p, "dd" | "patch")
        || (matches!(p, "sed" | "gsed" | "perl" | "ruby")
            && args.iter().any(|a| in_place_flag(&a.text)))
}

/// The code after a short-flag cluster containing `letter` (`-c`, `-ec`, `-Ic`).
fn inline_code(args: &[&Word], letter: char) -> Option<String> {
    let at = args.iter().position(|w| {
        w.text.starts_with('-') && !w.text.starts_with("--") && w.text[1..].contains(letter)
    })?;
    args.get(at + 1).map(|w| w.text.clone())
}

/// `-i`, `-pi`, `-i.bak` or `--in-place`.
fn in_place_flag(arg: &str) -> bool {
    arg.starts_with("--in-place")
        || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains('i'))
}

/// Paths under `base` matching a `*`/`?` glob, at most a few thousand.
fn expand_glob(base: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut found = vec![PathBuf::new()];
    for segment in pattern.split('/').filter(|s| !s.is_empty() && *s != ".") {
        let mut next = Vec::new();
        for prefix in &found {
            if !segment.contains(['*', '?']) {
                next.push(prefix.join(segment));
                continue;
            }
            let Ok(entries) = fs::read_dir(base.join(prefix)) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if glob::matches(segment, &name) {
                    next.push(prefix.join(name));
                }
            }
        }
        next.truncate(4096);
        found = next;
    }
    found
        .into_iter()
        .filter(|p| base.join(p).exists())
        .collect()
}
