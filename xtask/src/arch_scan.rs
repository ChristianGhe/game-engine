//! Source scan for core crates: escapes the type system and clippy cannot
//! see. It tokenizes (skipping comments and literals), so attributes split
//! across lines are still found.
//!
//! Clippy's own escapes (`#[allow(clippy::style)]` and the like) are also
//! stopped by the compiler, because the workspace sets the determinism lints
//! to `forbid`; this scan is the second line.

/// Lints that must never be silenced, downgraded or expected in a core crate.
const NEVER_SILENCED: [&str; 8] = [
    "warnings",
    "unsafe_code",
    "clippy::all",
    "clippy::style",
    "clippy::restriction",
    "clippy::disallowed_methods",
    "clippy::disallowed_types",
    "clippy::disallowed_macros",
];

struct Token {
    text: String,
    line: usize,
}

fn tokens(src: &str) -> Vec<Token> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            c if c.is_whitespace() => i += 1,
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
                let mut depth = 0;
                while i < chars.len() {
                    if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                        depth += 1;
                        i += 2;
                    } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        line += usize::from(chars[i] == '\n');
                        i += 1;
                    }
                }
            }
            '"' => {
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    line += usize::from(chars[i] == '\n');
                    i += if chars[i] == '\\' { 2 } else { 1 };
                }
                i += 1;
                out.push(Token {
                    text: "\"\"".into(),
                    line,
                });
            }
            '\'' => {
                // A char literal ('x', '\n', '\u{1F600}') or a lifetime ('a).
                if next == Some('\\') {
                    i += 2;
                    while i < chars.len() && chars[i] != '\'' {
                        i += 1;
                    }
                    i += 1;
                } else if chars.get(i + 2) == Some(&'\'') {
                    i += 3;
                } else {
                    i += 1;
                }
                out.push(Token {
                    text: "'".into(),
                    line,
                });
            }
            c if c.is_alphanumeric() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                // Raw strings: r"..", r#".."#, br#".."#, cr".."
                if matches!(ident.as_str(), "r" | "br" | "cr")
                    && matches!(chars.get(i), Some('"' | '#'))
                {
                    let mut hashes = 0;
                    while chars.get(i) == Some(&'#') {
                        hashes += 1;
                        i += 1;
                    }
                    if chars.get(i) == Some(&'"') {
                        i += 1;
                        loop {
                            if i >= chars.len() {
                                break;
                            }
                            if chars[i] == '"'
                                && (1..=hashes).all(|k| chars.get(i + k) == Some(&'#'))
                            {
                                i += 1 + hashes;
                                break;
                            }
                            line += usize::from(chars[i] == '\n');
                            i += 1;
                        }
                        out.push(Token {
                            text: "\"\"".into(),
                            line,
                        });
                        continue;
                    }
                }
                out.push(Token { text: ident, line });
            }
            c => {
                out.push(Token {
                    text: c.to_string(),
                    line,
                });
                i += 1;
            }
        }
    }
    out
}

/// Findings for one core-crate source file, each `(line, message)`.
pub fn scan(src: &str) -> Vec<(usize, String)> {
    let toks = tokens(src);
    let text = |i: usize| toks.get(i).map_or("", |t| t.text.as_str());
    let mut out = Vec::new();
    for (i, tok) in toks.iter().enumerate() {
        match tok.text.as_str() {
            "extern" if text(i + 1) == "crate" && text(i + 2) == "std" => out.push((
                tok.line,
                "`extern crate std` brings std into a no_std core crate".to_string(),
            )),
            "#" => {
                // Inside an attribute (also via cfg_attr), `path = ...` moves a module.
                let open = if text(i + 1) == "!" { i + 2 } else { i + 1 };
                if text(open) == "[" {
                    let (mut depth, mut j) = (0, open);
                    while j < toks.len() {
                        match text(j) {
                            "[" => depth += 1,
                            "]" => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            "path" if text(j + 1) == "=" => out.push((
                                toks[j].line,
                                "`path = ...` in an attribute can pull in code from outside src/"
                                    .to_string(),
                            )),
                            _ => {}
                        }
                        j += 1;
                    }
                }
            }
            "include" if text(i + 1) == "!" => out.push((
                tok.line,
                "`include!` can pull in code from outside src/".to_string(),
            )),
            "allow" | "expect" | "warn" if text(i + 1) == "(" => {
                // Collect the lint paths inside the parentheses.
                let (mut depth, mut j, mut inner) = (0, i + 1, String::new());
                while j < toks.len() {
                    match text(j) {
                        "(" => depth += 1,
                        ")" => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        t if depth == 1 => inner.push_str(if t == "," { " " } else { t }),
                        _ => {}
                    }
                    j += 1;
                }
                for lint in inner.split_whitespace() {
                    if NEVER_SILENCED.contains(&lint) {
                        out.push((
                            tok.line,
                            format!("`{}({lint})` silences a determinism lint", tok.text),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The crate-root inner attributes of a `lib.rs`: the `#![...]` lines before
/// the first item, ignoring comments and blank lines.
pub fn root_attributes(lib_rs: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    for line in lib_rs.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("#![") {
            attrs.push(line.to_string());
        } else {
            break;
        }
    }
    attrs
}

#[cfg(test)]
mod tests {
    use super::scan;

    fn messages(src: &str) -> Vec<usize> {
        scan(src).into_iter().map(|(line, _)| line).collect()
    }

    #[test]
    fn finds_escapes_across_lines_and_groups() {
        assert_eq!(messages("extern crate std;"), [1]);
        assert_eq!(messages("extern\n  crate\n  std;"), [1]);
        assert_eq!(
            messages("#[allow(\n    clippy::disallowed_methods\n)]\nfn f() {}"),
            [1]
        );
        assert_eq!(messages("#[allow(clippy::style)] fn f() {}"), [1]);
        assert_eq!(messages("#![cfg_attr(test, expect(warnings))]"), [1]);
        assert_eq!(messages("#[warn(clippy::disallowed_types)] fn f() {}"), [1]);
        assert_eq!(messages("mod a;\n#[path = \"../x.rs\"]\nmod b;"), [2]);
        assert_eq!(messages("include!(\"gen.rs\");"), [1]);
        // Found in review: a char literal or raw string must not hide what follows.
        assert_eq!(messages("const Q: char = '\"';\nextern crate std;"), [2]);
        assert_eq!(
            messages("const R: &str = r#\"a\"b\"#;\nextern crate std;"),
            [2]
        );
        assert_eq!(
            messages("#[cfg_attr(all(), path = \"/abs/x.rs\")]\nmod x;"),
            [1]
        );
        assert_eq!(messages("fn f<'a>(x: &'a str) {}\nextern crate std;"), [2]);
    }

    #[test]
    fn ignores_comments_literals_and_harmless_lints() {
        assert!(messages("// extern crate std;\n/* #[allow(warnings)] */").is_empty());
        assert!(messages("const S: &str = \"extern crate std\";").is_empty());
        assert!(messages("#[allow(clippy::too_many_arguments)] fn f() {}").is_empty());
        assert!(messages("const T: &str = include_str!(\"x.txt\");").is_empty());
        assert!(messages("extern crate alloc;").is_empty());
    }
}
