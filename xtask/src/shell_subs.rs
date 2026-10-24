//! Finding the command substitutions in shell text, for `shell`.

use super::{body_lines, delimiter};

/// The inner commands of `$( ... )`, backtick, `<( ... )`, `>( ... )` and
/// zsh's `=( ... )` substitutions, wherever the shell runs them: not in
/// single quotes, comments or quoted heredoc bodies.
pub fn substitutions(command: &str) -> Vec<String> {
    let chars: Vec<char> = command.chars().collect();
    let mut out = Vec::new();
    scan(&chars, 0, false, &mut out);
    out
}

/// Collects the substitutions in shell text from `i`. With `closes`, the
/// text is inside `$(`: it stops at the closing `)` and returns its index.
fn scan(chars: &[char], mut i: usize, closes: bool, out: &mut Vec<String>) -> usize {
    // `'` single quotes, `"` double quotes (substitutions still run), `$`
    // for `$'...'` (backslash escapes).
    let mut quote: Option<char> = None;
    let mut parens = 0usize;
    let mut pending: Vec<(String, bool, bool)> = Vec::new();
    let mut word_start = true;
    let mut dollar = false;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let at_word_start = std::mem::take(&mut word_start);
        let after_dollar = std::mem::take(&mut dollar);
        match (quote, c) {
            (Some('\'' | '$'), '\'') | (Some('"'), '"') => quote = None,
            (Some('$' | '"') | None, '\\') => i += 1,
            (Some('\'' | '$'), _) => {}
            (None, '\'') => quote = Some(if after_dollar { '$' } else { '\'' }),
            (None, '"') => quote = Some('"'),
            (None, '#') if at_word_start => {
                while chars.get(i + 1).is_some_and(|&n| n != '\n') {
                    i += 1;
                }
            }
            (None, '<') if next == Some('<') && chars.get(i + 2) == Some(&'<') => i += 2,
            (None, '<') if next == Some('<') => {
                let (delim, strip_tabs, quoted, end) = delimiter(chars, i + 2);
                pending.push((delim, strip_tabs, quoted));
                i = end;
                continue;
            }
            (None, '\n') if !pending.is_empty() => {
                for (delim, strip_tabs, quoted) in pending.drain(..) {
                    let (lines, end) = body_lines(chars, i, &delim, strip_tabs);
                    i = end;
                    for (start, stop) in lines.into_iter().filter(|_| !quoted) {
                        body(chars, start, stop, out);
                    }
                }
                word_start = true;
            }
            (_, '$') if next == Some('(') => i = inner(chars, i + 2, out),
            (None, '<' | '>' | '=') if next == Some('(') => i = inner(chars, i + 2, out),
            (_, '`') => {
                let end = backtick_end(chars, i + 1);
                out.push(chars[i + 1..end].iter().collect());
                i = end;
            }
            (None, '(') => parens += 1,
            (None, ')') if parens > 0 => parens -= 1,
            (None, ')') if closes => return i,
            (None, c) => {
                dollar = c == '$' && !after_dollar;
                word_start = c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')');
            }
            _ => {}
        }
        i += 1;
    }
    i
}

/// Records the substitution whose text starts at `start`; returns the index
/// of its closing `)`.
fn inner(chars: &[char], start: usize, out: &mut Vec<String>) -> usize {
    let end = scan(chars, start, true, out);
    out.push(chars[start..end.min(chars.len())].iter().collect());
    end
}

/// Substitutions in a heredoc body with an unquoted delimiter, where quotes
/// are plain text but `$( )` and backticks still run.
fn body(chars: &[char], start: usize, end: usize, out: &mut Vec<String>) {
    let mut j = start;
    while j < end {
        match chars[j] {
            '\\' => j += 1,
            '$' if chars.get(j + 1) == Some(&'(') => j = inner(chars, j + 2, out),
            '`' => {
                let close = backtick_end(chars, j + 1);
                out.push(chars[j + 1..close].iter().collect());
                j = close;
            }
            _ => {}
        }
        j += 1;
    }
}

/// The index of the backtick that closes one opened before `start`.
fn backtick_end(chars: &[char], start: usize) -> usize {
    let mut j = start;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 2,
            '`' => return j,
            _ => j += 1,
        }
    }
    chars.len()
}
