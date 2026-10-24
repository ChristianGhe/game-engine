//! Just enough POSIX shell parsing for the guard: simple commands, quotes,
//! heredocs, subshells and command substitutions. It does not need to run
//! shell, only to find what a command could write.

#[path = "shell_subs.rs"]
mod subs;

pub use subs::substitutions;

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
    let mut subshells = 0usize; // open `(`; a `)` without one ends a case pattern
    let mut ansi = false; // inside $'...', where a backslash escapes the quote
    let mut dollar = false; // the previous character was an unescaped `$`
    let mut word_start = true; // where a `#` starts a comment
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
        let after_dollar = std::mem::take(&mut dollar);
        let at_word_start = std::mem::take(&mut word_start);
        // `$(( ))`, and `(( ))` where a command starts, are arithmetic: `<<`
        // there is a shift, not a heredoc.
        let arithmetic = match (quote, c) {
            (None, '$') if next == Some('(') => arithmetic_end(&chars, i + 1),
            (None, '(') if matches!(current.trim(), "" | "for") => arithmetic_end(&chars, i),
            _ => None,
        };
        if let Some(end) = arithmetic {
            current.extend(&chars[i..=end]);
            i = end + 1;
            continue;
        }
        match (quote, c) {
            (Some('\''), '\\') if ansi => {
                current.push(c);
                if let Some(n) = next {
                    current.push(n);
                    i += 1;
                }
            }
            (Some(q), c) if c == q => {
                quote = None;
                ansi = false;
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
                ansi = c == '\'' && after_dollar;
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
            (None, '<') if next == Some('<') && chars.get(i + 2) == Some(&'<') => {
                // A here-string: its word is input, not a heredoc delimiter.
                current.push_str("<<<");
                i += 3;
                continue;
            }
            (None, '<') if next == Some('<') => {
                // A heredoc: remember its delimiter; the body follows the line.
                let (delim, strip_tabs, _, end) = delimiter(&chars, i + 2);
                current.push_str("<<");
                pending.push((delim, strip_tabs));
                i = end;
                continue;
            }
            (None, '\n') if depth == 0 || !pending.is_empty() => {
                // Heredoc bodies come first. Inside `$( )` a body belongs to
                // the substitution, which is checked on its own.
                for (delim, strip_tabs) in pending.drain(..) {
                    let (lines, end) = body_lines(&chars, i, &delim, strip_tabs);
                    i = end;
                    for (start, stop) in lines.into_iter().filter(|_| depth == 0) {
                        heredoc.extend(&chars[start..stop]);
                        heredoc.push('\n');
                    }
                }
                if depth == 0 {
                    flush(&mut current, &mut heredoc, &mut out);
                } else {
                    current.push('\n');
                }
                word_start = true;
            }
            (None, ';' | '|' | '&' | '\n') if depth == 0 => {
                if c == '|' && current.ends_with('>') {
                    // `>|`: a redirection that overwrites.
                    current.push(c);
                } else if c == '&' && matches!(current.chars().last(), Some('>' | '<'))
                    || next == Some('>') && c == '&'
                {
                    // `>&2`, `&>file`: part of a redirection.
                    current.push(c);
                } else {
                    if matches!(c, '|' | '&') && next == Some(c) {
                        i += 1;
                    }
                    flush(&mut current, &mut heredoc, &mut out);
                    word_start = true;
                }
            }
            (None, '(') if depth == 0 && current.trim().is_empty() => {
                subshells += 1;
                out.push(Piece::Open);
                word_start = true;
            }
            (None, ')') if depth == 0 && subshells > 0 => {
                subshells -= 1;
                flush(&mut current, &mut heredoc, &mut out);
                out.push(Piece::Close);
                word_start = true;
            }
            (None, ')') if depth == 0 => {
                // The end of a `case` pattern, which is not a command.
                current.clear();
                word_start = true;
            }
            (None, '#') if at_word_start => {
                // A comment, to the end of the line.
                while chars.get(i + 1).is_some_and(|&n| n != '\n') {
                    i += 1;
                }
            }
            (None, c) => {
                dollar = c == '$' && !after_dollar;
                word_start = c.is_whitespace();
                current.push(c);
            }
        }
        i += 1;
    }
    flush(&mut current, &mut heredoc, &mut out);
    out
}

/// A heredoc delimiter that starts at `j`, just after `<<`: the delimiter,
/// whether tabs are stripped (`<<-`), whether it was quoted (the body is
/// then plain text), and the index after it.
fn delimiter(chars: &[char], mut j: usize) -> (String, bool, bool, usize) {
    let strip_tabs = chars.get(j) == Some(&'-');
    if strip_tabs {
        j += 1;
    }
    while chars.get(j) == Some(&' ') {
        j += 1;
    }
    let (mut delim, mut quoted) = (String::new(), false);
    while let Some(&d) = chars.get(j) {
        if d.is_whitespace() || matches!(d, ';' | '&' | '|' | '<' | '>' | ')') {
            break;
        }
        if matches!(d, '\'' | '"' | '\\') {
            quoted = true;
        } else {
            delim.push(d);
        }
        j += 1;
    }
    (delim, strip_tabs, quoted, j)
}

/// The body of a heredoc whose line ends at the newline `i`: its lines as
/// `(start, end)` ranges, and where the delimiter line ends. As in bash, an
/// unterminated body runs to the end.
fn body_lines(
    chars: &[char],
    mut i: usize,
    delim: &str,
    strip_tabs: bool,
) -> (Vec<(usize, usize)>, usize) {
    let mut lines = Vec::new();
    loop {
        let start = i + 1;
        if start >= chars.len() {
            break;
        }
        let end = chars[start..]
            .iter()
            .position(|&c| c == '\n')
            .map_or(chars.len(), |p| start + p);
        i = end;
        let line: String = chars[start..end].iter().collect();
        let check = if strip_tabs {
            line.trim_start_matches('\t')
        } else {
            line.as_str()
        };
        if check == delim {
            break;
        }
        lines.push((start, end));
    }
    (lines, i)
}

/// The index of the `)` that closes the `((` at `open`, if the span ends in
/// `))`; bash reads only such a span as arithmetic, never as commands.
fn arithmetic_end(chars: &[char], open: usize) -> Option<usize> {
    if chars.get(open + 1) != Some(&'(') {
        return None;
    }
    let mut depth = 0usize;
    for (j, &c) in chars.iter().enumerate().skip(open) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return (chars[j - 1] == ')' && j - 1 > open + 1).then_some(j);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits one simple command into words, honouring quotes and backslashes.
pub fn words(command: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut text = String::new();
    let (mut quoted, mut dynamic) = (false, false);
    let mut quote = None;
    let mut depth = 0usize; // inside $( ... ), where spaces do not split
    let mut ansi = false; // inside $'...', whose escapes the guard does not decode
    let mut dollar = false; // the previous character was an unescaped `$`
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
        let after_dollar = std::mem::take(&mut dollar);
        match (quote, c) {
            (None | Some('"'), '\\') if chars.peek() == Some(&'\n') => {
                // A line continuation: bash joins the two lines.
                chars.next();
            }
            (Some('\''), '\\') if ansi => {
                dynamic = true;
                text.push(c);
                text.extend(chars.next());
            }
            (Some(q), c) if c == q => {
                quote = None;
                ansi = false;
            }
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
            (None, '$') if !after_dollar && matches!(chars.peek(), Some('\'' | '"')) => {
                // `$'...'` and `$"..."` quote like `'...'` and `"..."`; in
                // `$$'...'` the `$$` is a number and the quote is plain.
                quote = chars.next();
                ansi = quote == Some('\'');
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
                        // zsh's `>!` and `>>!` overwrite like `>|`.
                        '>' => {
                            matches!(n, '>' | '|' | '&')
                                || (n == '!'
                                    && op.trim_start_matches(char::is_numeric).ends_with('>'))
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
                dollar = c == '$' && !after_dollar;
                dynamic = true;
                text.push(c);
            }
            (None, c) => text.push(c),
        }
    }
    push(&mut out, &mut text, &mut quoted, &mut dynamic);
    out
}
