//! Just enough POSIX shell parsing for the guard: simple commands, quotes,
//! heredocs, subshells and command substitutions. It does not need to run
//! shell, only to find what a command could write.

/// One shell word.
pub struct Word {
    pub text: String,
    /// Any part was quoted or escaped (a quoted `>` is not a redirection).
    pub quoted: bool,
    /// Its value is decided at run time: a variable, a substitution or an
    /// unquoted glob. The guard cannot tell what it names.
    pub dynamic: bool,
    /// A redirection operator such as `>`, `2>>`, `&>` or `<<`; the next
    /// word is its target.
    pub op: bool,
}

pub enum Piece {
    /// A simple command, with any heredoc bodies it reads.
    Command { words: Vec<Word>, heredoc: String },
    /// `(`: a subshell starts; `cd` inside it does not leak out.
    Open,
    /// `)`: the subshell ends.
    Close,
}

/// Splits on `;`, `&&`, `||`, `|`, `&`, newlines and subshell parentheses,
/// outside quotes. Heredoc bodies stay with the command that reads them.
pub fn split(command: &str) -> Vec<Piece> {
    let chars: Vec<char> = command.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    let mut heredoc = String::new();
    let mut pending: Vec<(String, bool)> = Vec::new();
    let mut quote: Option<char> = None;
    let mut depth = 0usize; // inside $( ... )
    let mut i = 0;
    let flush = |current: &mut String, heredoc: &mut String, out: &mut Vec<Piece>| {
        if !current.trim().is_empty() {
            out.push(Piece::Command {
                words: words(current),
                heredoc: std::mem::take(heredoc),
            });
        }
        current.clear();
    };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match (quote, c) {
            (Some(q), c) if c == q => {
                quote = None;
                current.push(c);
            }
            (Some('"'), '\\') | (None, '\\') => {
                current.push(c);
                if let Some(n) = next {
                    current.push(n);
                    i += 1;
                }
            }
            (Some(_), c) => current.push(c),
            (None, '\'' | '"' | '`') => {
                quote = Some(c);
                current.push(c);
            }
            (None, '$') if next == Some('(') => {
                depth += 1;
                current.push_str("$(");
                i += 1;
            }
            (None, ')') if depth > 0 => {
                depth -= 1;
                current.push(c);
            }
            (None, '<') if next == Some('<') && chars.get(i + 2) != Some(&'<') => {
                // A heredoc: remember its delimiter; the body follows the line.
                let mut j = i + 2;
                let strip_tabs = chars.get(j) == Some(&'-');
                if strip_tabs {
                    j += 1;
                }
                while chars.get(j) == Some(&' ') {
                    j += 1;
                }
                let mut delim = String::new();
                while let Some(&d) = chars.get(j) {
                    if d.is_whitespace() || matches!(d, ';' | '&' | '|' | '<' | '>' | ')') {
                        break;
                    }
                    if !matches!(d, '\'' | '"' | '\\') {
                        delim.push(d);
                    }
                    j += 1;
                }
                current.push_str("<<");
                pending.push((delim, strip_tabs));
                i = j;
                continue;
            }
            (None, '\n') if depth == 0 => {
                // Consume heredoc bodies before ending the command.
                for (delim, strip_tabs) in pending.drain(..) {
                    loop {
                        let start = i + 1;
                        let end = chars[start..]
                            .iter()
                            .position(|&c| c == '\n')
                            .map_or(chars.len(), |p| start + p);
                        let line: String = chars[start.min(chars.len())..end].iter().collect();
                        i = end;
                        let check = if strip_tabs {
                            line.trim_start_matches('\t')
                        } else {
                            line.as_str()
                        };
                        if check == delim || start >= chars.len() {
                            break;
                        }
                        heredoc.push_str(&line);
                        heredoc.push('\n');
                    }
                }
                flush(&mut current, &mut heredoc, &mut out);
            }
            (None, ';' | '|' | '&' | '\n') if depth == 0 => {
                if matches!(c, '|' | '&') && next == Some(c) {
                    i += 1;
                }
                if c == '&' && matches!(current.chars().last(), Some('>' | '<'))
                    || next == Some('>') && c == '&'
                {
                    // `>&2`, `&>file`: part of a redirection.
                    current.push(c);
                } else {
                    flush(&mut current, &mut heredoc, &mut out);
                }
            }
            (None, '(') if depth == 0 && current.trim().is_empty() => {
                out.push(Piece::Open);
            }
            (None, ')') if depth == 0 => {
                flush(&mut current, &mut heredoc, &mut out);
                out.push(Piece::Close);
            }
            (None, c) => current.push(c),
        }
        i += 1;
    }
    flush(&mut current, &mut heredoc, &mut out);
    out
}

/// The inner commands of `$( ... )` and backtick substitutions.
pub fn substitutions(command: &str) -> Vec<String> {
    let chars: Vec<char> = command.chars().collect();
    let mut out = Vec::new();
    let mut quote = None;
    let mut i = 0;
    while i < chars.len() {
        match (quote, chars[i]) {
            (Some('\''), '\'') => quote = None,
            (Some('\''), _) => {}
            (_, '$') if chars.get(i + 1) == Some(&'(') => {
                let (mut depth, start) = (1, i + 2);
                let mut j = start;
                while j < chars.len() && depth > 0 {
                    match chars[j] {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                out.push(
                    chars[start..j.saturating_sub(1).max(start)]
                        .iter()
                        .collect(),
                );
                i = j;
                continue;
            }
            (_, '`') => {
                let start = i + 1;
                let end = chars[start..]
                    .iter()
                    .position(|&c| c == '`')
                    .map_or(chars.len(), |p| start + p);
                out.push(chars[start..end].iter().collect());
                i = end + 1;
                continue;
            }
            (None, '\'') => quote = Some('\''),
            _ => {}
        }
        i += 1;
    }
    out
}

/// Splits one simple command into words, honouring quotes and backslashes.
pub fn words(command: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut text = String::new();
    let (mut quoted, mut dynamic) = (false, false);
    let mut quote = None;
    let mut depth = 0usize; // inside $( ... ), where spaces do not split
    let mut chars = command.chars().peekable();
    let push = |out: &mut Vec<Word>, text: &mut String, quoted: &mut bool, dynamic: &mut bool| {
        if !text.is_empty() || *quoted {
            out.push(Word {
                text: std::mem::take(text),
                quoted: *quoted,
                dynamic: *dynamic,
                op: false,
            });
        }
        (*quoted, *dynamic) = (false, false);
    };
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => text.extend(chars.next()),
            (Some('"'), '$' | '`') => {
                dynamic = true;
                text.push(c);
            }
            (Some(_), c) => text.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                quoted = true;
            }
            (None, '\\') => {
                text.extend(chars.next());
                quoted = true;
            }
            (None, '$') if chars.peek() == Some(&'(') => {
                chars.next();
                depth += 1;
                dynamic = true;
                text.push_str("$(");
            }
            (None, ')') if depth > 0 => {
                depth -= 1;
                text.push(c);
            }
            (None, c) if c.is_whitespace() && depth == 0 => {
                push(&mut out, &mut text, &mut quoted, &mut dynamic)
            }
            (None, '>' | '<') if depth == 0 => {
                // `2>`, `&>`: a file-descriptor prefix belongs to the operator.
                let fd_prefix = !quoted
                    && !text.is_empty()
                    && text.chars().all(|c| c.is_ascii_digit() || c == '&');
                let mut op = if fd_prefix {
                    std::mem::take(&mut text)
                } else {
                    String::new()
                };
                push(&mut out, &mut text, &mut quoted, &mut dynamic);
                op.push(c);
                while let Some(&n) = chars.peek() {
                    let continues = match c {
                        '>' => {
                            matches!(n, '>' | '|' | '&')
                                || (op.ends_with('&') && (n.is_ascii_digit() || n == '-'))
                        }
                        _ => {
                            matches!(n, '<' | '>' | '-' | '&')
                                || (op.ends_with('&') && (n.is_ascii_digit() || n == '-'))
                        }
                    };
                    if !continues {
                        break;
                    }
                    op.push(n);
                    chars.next();
                }
                out.push(Word {
                    text: op,
                    quoted: false,
                    dynamic: false,
                    op: true,
                });
            }
            (None, '$' | '`' | '*' | '?' | '[' | '{') => {
                dynamic = true;
                text.push(c);
            }
            (None, c) => text.push(c),
        }
    }
    push(&mut out, &mut text, &mut quoted, &mut dynamic);
    out
}
