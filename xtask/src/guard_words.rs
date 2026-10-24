//! Word-level helpers for the guard: assignments, wrappers, keywords and
//! bless commands.

use std::collections::BTreeMap;

use crate::shell::{Piece, Word};

/// Shell words that may precede a command on the same line.
const KEYWORDS: [&str; 13] = [
    "{", "}", "then", "do", "else", "elif", "if", "while", "until", "!", "fi", "done", "time",
];

#[derive(Clone, PartialEq)]
pub(crate) enum Var {
    Value(String),
    Temp,
    Unknown,
}

pub(crate) fn is_assignment(text: &str) -> Option<(&str, &str)> {
    let (name, value) = text.split_once('=')?;
    let valid = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    valid.then_some((name, value))
}

pub(crate) fn bless_name(name: &str) -> bool {
    name.starts_with("UPDATE_")
        || name.ends_with("_UPDATE")
        || name == "UPDATE"
        || name.starts_with("INSTA_")
        || name == "TRYBUILD"
        || name.contains("BLESS")
}

/// Leading keywords dropped.
pub(crate) fn command_words(words: Vec<Word>) -> Vec<Word> {
    words
        .into_iter()
        .skip_while(|w| !w.quoted && KEYWORDS.contains(&w.text.as_str()))
        .collect()
}

/// Words without redirections and their targets.
pub(crate) fn plain(words: &[Word]) -> Vec<&Word> {
    let mut out = Vec::new();
    let mut skip = false;
    for w in words {
        if skip {
            skip = false;
            continue;
        }
        if w.op {
            skip = !w.text.contains('&') || w.text.ends_with('>');
            continue;
        }
        out.push(w);
    }
    out
}

/// Skips assignments and wrapper programs; returns `(assignments, program, args)`.
pub(crate) fn unwrap<'a>(words: &[&'a Word]) -> (Vec<&'a Word>, Option<String>, Vec<&'a Word>) {
    let mut assigns = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        // `X="a b" cmd` assigns too: quotes in the value do not matter.
        if is_assignment(&w.text).is_some() {
            assigns.push(w);
            i += 1;
            continue;
        }
        // zsh's `=cmd` runs `cmd`.
        let text = w
            .text
            .strip_prefix('=')
            .filter(|t| !t.is_empty())
            .unwrap_or(&w.text);
        let name = text.rsplit('/').next().unwrap_or(text);
        let takes_value: &[&str] = match name {
            "sudo" => &["-u", "-g", "-C"],
            "env" => &["-u", "-C", "-S"],
            "nice" => &["-n"],
            "ionice" => &["-c", "-n"],
            "timeout" => &["-s", "-k"],
            "exec" => &["-a"],
            "stdbuf" | "command" | "nohup" | "time" | "builtin" => &[],
            // zsh's precommand modifiers.
            "noglob" | "nocorrect" | "-" | "repeat" => &[],
            _ => return (assigns, Some(name.to_string()), words[i + 1..].to_vec()),
        };
        i += 1;
        while i < words.len() && words[i].text.starts_with('-') {
            i += if takes_value.contains(&words[i].text.as_str()) {
                2
            } else {
                1
            };
        }
        if matches!(name, "timeout" | "repeat") && i < words.len() {
            i += 1; // the duration, or the count
        }
    }
    (assigns, None, Vec::new())
}

/// The first bless/accept command in `pieces`, if any.
pub(crate) fn bless(pieces: &[Piece]) -> Option<String> {
    for piece in pieces {
        let Piece::Command { words, .. } = piece else {
            continue;
        };
        let words: Vec<&Word> = words.iter().filter(|w| !w.op).collect();
        let (assigns, program, args) = unwrap(&words);
        let assigned = assigns
            .iter()
            .chain(args.iter().filter(|_| program.as_deref() == Some("export")))
            .filter_map(|w| is_assignment(&w.text))
            .find(|(name, _)| bless_name(name));
        if let Some((name, _)) = assigned {
            return Some(format!("{name}="));
        }
        let Some(program) = program else {
            continue;
        };
        let texts: Vec<&str> = args.iter().map(|w| w.text.as_str()).collect();
        let insta = matches!(program.as_str(), "cargo-insta" | "insta")
            || (program == "cargo" && texts.first() == Some(&"insta"));
        let sub = if program == "cargo" {
            texts.get(1)
        } else {
            texts.first()
        };
        if insta && matches!(sub, Some(&("accept" | "review"))) {
            return Some(format!("{program} insta {}", sub.unwrap_or(&"")));
        }
        let quiet = matches!(
            program.as_str(),
            "git" | "gh" | "echo" | "printf" | "grep" | "rg"
        );
        if !quiet && let Some(flag) = texts.iter().find(|t| matches!(**t, "--bless" | "--accept")) {
            return Some(format!("{program} {flag}"));
        }
    }
    None
}

/// Variables the command assigns, with what can be known of their values.
pub(crate) fn assignments(pieces: &[Piece]) -> BTreeMap<String, Var> {
    let mut vars: BTreeMap<String, Var> = BTreeMap::new();
    let mut set = |name: &str, var: Var| {
        let merged = match vars.get(name) {
            Some(old) if *old != var => Var::Unknown,
            _ => var,
        };
        vars.insert(name.to_string(), merged);
    };
    for piece in pieces {
        let Piece::Command { words, .. } = piece else {
            continue;
        };
        let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        for (i, w) in words.iter().enumerate() {
            let leading = words[..i].iter().all(|p| is_assignment(&p.text).is_some());
            let declared = matches!(
                texts.first(),
                Some(&("export" | "declare" | "local" | "readonly" | "typeset"))
            );
            if let Some((name, value)) = is_assignment(&w.text)
                && (leading || declared)
            {
                let var = if value.starts_with("$(mktemp") || value.starts_with("`mktemp") {
                    Var::Temp
                } else if w.dynamic {
                    Var::Unknown
                } else {
                    Var::Value(value.to_string())
                };
                set(name, var);
            }
        }
        match texts.as_slice() {
            ["for", name, ..] => set(name, Var::Unknown),
            ["read", rest @ ..] | ["mapfile", rest @ ..] => {
                for name in rest.iter().filter(|t| !t.starts_with('-')) {
                    set(name, Var::Unknown);
                }
            }
            _ => {}
        }
    }
    vars
}
