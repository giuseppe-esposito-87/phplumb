//! A YAML tree that keeps where each scalar is written, for the Drupal registries.

use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::Marker;

pub(crate) enum Yaml {
    /// A scalar and the byte range it is written at, quotes included.
    Scalar {
        value: String,
        start: u32,
        end: u32,
    },
    Map(Vec<(Yaml, Yaml)>),
    Sequence,
    Other,
}

impl Yaml {
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Yaml::Scalar { value, .. } => Some(value),
            _ => None,
        }
    }

    /// The value of the key `key`, when this is a map.
    pub(crate) fn get(&self, key: &str) -> Option<&Yaml> {
        match self {
            Yaml::Map(entries) => entries
                .iter()
                .find(|(k, _)| k.text() == Some(key))
                .map(|(_, value)| value),
            _ => None,
        }
    }
}

/// The tree of `text`; nothing when it does not parse.
pub(crate) fn parse(text: &str) -> Option<Yaml> {
    let mut builder = Builder {
        text,
        lines: std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect(),
        stack: Vec::new(),
        root: None,
    };
    Parser::new_from_str(text).load(&mut builder, false).ok()?;
    builder.root
}

/// The byte range of the text `value` inside the scalar written at `start..end`:
/// the whole range for a plain scalar, inside the quotes for a quoted one; an empty
/// range at `start` when the written text differs from the value (escapes).
pub(crate) fn value_range(text: &str, value: &str, start: u32, end: u32) -> (u32, u32) {
    let (start, end) = (start as usize, end as usize);
    for inner in [start, start + 1] {
        if text.get(inner..inner + value.len()) == Some(value) && inner + value.len() <= end {
            return (inner as u32, (inner + value.len()) as u32);
        }
    }
    (start as u32, start as u32)
}

enum Frame {
    Map(Vec<(Yaml, Yaml)>, Option<Yaml>),
    Sequence,
}

struct Builder<'t> {
    text: &'t str,
    /// Byte offsets of line starts.
    lines: Vec<usize>,
    stack: Vec<Frame>,
    root: Option<Yaml>,
}

impl Builder<'_> {
    fn push(&mut self, node: Yaml) {
        match self.stack.last_mut() {
            Some(Frame::Map(entries, key)) => match key.take() {
                Some(key) => entries.push((key, node)),
                None => *key = Some(node),
            },
            Some(Frame::Sequence) => {}
            None => self.root = Some(node),
        }
    }

    /// The byte offset of a marker, from its line and its character column (the
    /// marker's own index counts characters).
    fn offset(&self, mark: &Marker) -> usize {
        let Some(&line_start) = self.lines.get(mark.line().saturating_sub(1)) else {
            return self.text.len();
        };
        let line = &self.text[line_start..];
        let line = &line[..line.find('\n').unwrap_or(line.len())];
        line_start
            + line
                .char_indices()
                .nth(mark.col())
                .map_or(line.len(), |(i, _)| i)
    }

    /// Where the scalar written at `start` ends: after its closing quote, or after
    /// its text for a plain scalar, or at the end of the line.
    fn scalar_end(&self, start: usize, value: &str) -> usize {
        let rest = &self.text[start..];
        let line_end = rest.find('\n').map_or(self.text.len(), |i| start + i);
        let bytes = rest.as_bytes();
        match bytes.first() {
            Some(b'\'') => {
                let mut i = 1;
                while i < bytes.len() {
                    if bytes[i] == b'\'' {
                        if bytes.get(i + 1) == Some(&b'\'') {
                            i += 2;
                            continue;
                        }
                        return start + i + 1;
                    }
                    i += 1;
                }
                line_end
            }
            Some(b'"') => {
                let mut i = 1;
                while i < bytes.len() {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => return start + i + 1,
                        _ => i += 1,
                    }
                }
                line_end
            }
            _ if rest.starts_with(value) => start + value.len(),
            _ => line_end,
        }
    }
}

impl MarkedEventReceiver for Builder<'_> {
    fn on_event(&mut self, event: Event, mark: Marker) {
        match event {
            Event::Scalar(value, ..) => {
                let start = self.offset(&mark);
                let end = self.scalar_end(start, &value);
                self.push(Yaml::Scalar {
                    value,
                    start: start as u32,
                    end: end as u32,
                });
            }
            Event::MappingStart(..) => self.stack.push(Frame::Map(Vec::new(), None)),
            Event::MappingEnd => {
                if let Some(Frame::Map(entries, _)) = self.stack.pop() {
                    self.push(Yaml::Map(entries));
                }
            }
            Event::SequenceStart(..) => self.stack.push(Frame::Sequence),
            Event::SequenceEnd => {
                self.stack.pop();
                self.push(Yaml::Sequence);
            }
            Event::Alias(_) => self.push(Yaml::Other),
            _ => {}
        }
    }
}
