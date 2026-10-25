use toml::Spanned;
use toml::de::{DeTable, DeValue};

/// One step of a path into a TOML document: a table key or an array index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStep<'a> {
    Key(&'a str),
    Index(usize),
}

/// Where the items of an authored TOML text are, for error messages that
/// name a line. [`load_toml`](crate::load_toml) does the loading; this
/// parses the same text again and keeps only positions.
pub struct TomlSpans<'a> {
    text: &'a str,
    root: Option<Spanned<DeTable<'a>>>,
}

/// What a path step can look into.
#[derive(Clone, Copy)]
enum Container<'v, 'a> {
    Table(&'v DeTable<'a>),
    Array(&'v [Spanned<DeValue<'a>>]),
    Leaf,
}

impl<'a> TomlSpans<'a> {
    /// Parses `text`. A text that does not parse has no positions.
    pub fn parse(text: &'a str) -> Self {
        Self {
            text,
            root: DeTable::parse(text).ok(),
        }
    }

    /// The byte offset where the item at `path` starts: the key itself for
    /// a [`PathStep::Key`], the element for a [`PathStep::Index`]. `None`
    /// for an empty path or a path that is not in the text. Keys are exact,
    /// so `x_` extensions are found too.
    pub fn offset(&self, path: &[PathStep<'_>]) -> Option<usize> {
        let mut here = Container::Table(self.root.as_ref()?.get_ref());
        let mut start = None;
        for step in path {
            let found = match (*step, here) {
                (PathStep::Key(name), Container::Table(table)) => {
                    let (key, value) = table.get_key_value(name)?;
                    start = Some(key.span().start);
                    value
                }
                (PathStep::Index(i), Container::Array(items)) => {
                    let value = items.get(i)?;
                    start = Some(value.span().start);
                    value
                }
                _ => return None,
            };
            here = match found.get_ref() {
                DeValue::Table(table) => Container::Table(table),
                DeValue::Array(items) => Container::Array(items),
                _ => Container::Leaf,
            };
        }
        start
    }

    /// The 1-based line of a byte offset.
    pub fn line(&self, offset: usize) -> u32 {
        let before = &self.text.as_bytes()[..offset.min(self.text.len())];
        let newlines = before.iter().filter(|&&b| b == b'\n').count();
        u32::try_from(newlines + 1).unwrap_or(u32::MAX)
    }

    /// The 1-based line where the item at `path` starts; see [`Self::offset`].
    pub fn line_of(&self, path: &[PathStep<'_>]) -> Option<u32> {
        self.offset(path).map(|offset| self.line(offset))
    }
}
