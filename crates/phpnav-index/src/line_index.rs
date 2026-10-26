//! Conversion between byte offsets and LSP positions.

/// Line starts of a text, for converting byte offsets to 0-based lines and UTF-16
/// columns and back. Invalid UTF-8 bytes count as one UTF-16 unit each, matching
/// how editors show them as a single replacement character.
pub struct LineIndex {
    line_starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(text: &[u8]) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.iter()
                .enumerate()
                .filter(|(_, b)| **b == b'\n')
                .map(|(i, _)| i as u32 + 1),
        );
        Self { line_starts }
    }

    /// 0-based line and UTF-16 column of `offset`.
    pub fn position(&self, text: &[u8], offset: u32) -> (u32, u32) {
        let offset = offset.min(text.len() as u32);
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let start = self.line_starts[line] as usize;
        let col = utf16_len(&text[start..offset as usize]);
        (line as u32, col)
    }

    /// Byte offset of a 0-based line and UTF-16 column. None when the line does not
    /// exist or the column is past the line end.
    pub fn offset(&self, text: &[u8], line: u32, utf16_col: u32) -> Option<u32> {
        let start = *self.line_starts.get(line as usize)? as usize;
        let end = self.line_end(text, line as usize);
        let mut units = 0u32;
        let mut byte = start;
        for chunk in text[start..end].utf8_chunks() {
            for ch in chunk.valid().chars() {
                if units == utf16_col {
                    return Some(byte as u32);
                }
                units += ch.len_utf16() as u32;
                byte += ch.len_utf8();
            }
            for _ in chunk.invalid() {
                if units == utf16_col {
                    return Some(byte as u32);
                }
                units += 1;
                byte += 1;
            }
        }
        (units == utf16_col).then_some(end as u32)
    }

    /// Byte offset where the content of `line` ends, before its `\n` or `\r\n`.
    fn line_end(&self, text: &[u8], line: usize) -> usize {
        let mut end = match self.line_starts.get(line + 1) {
            Some(&next) => next as usize - 1,
            None => text.len(),
        };
        if end > self.line_starts[line] as usize && text.get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        end
    }
}

fn utf16_len(bytes: &[u8]) -> u32 {
    bytes
        .utf8_chunks()
        .map(|chunk| {
            chunk.valid().chars().map(char::len_utf16).sum::<usize>() + chunk.invalid().len()
        })
        .sum::<usize>() as u32
}

#[cfg(test)]
mod tests {
    use super::LineIndex;

    fn check(text: &[u8], offset: u32, line: u32, col: u32) {
        let index = LineIndex::new(text);
        assert_eq!(index.position(text, offset), (line, col));
        assert_eq!(index.offset(text, line, col), Some(offset));
    }

    #[test]
    fn ascii() {
        check(b"a\nbc\n", 3, 1, 1);
    }

    #[test]
    fn two_byte_char() {
        check("$x = 'è'; Foo".as_bytes(), 11, 0, 10);
    }

    #[test]
    fn surrogate_pair() {
        check("'😀';Foo".as_bytes(), 7, 0, 5);
    }

    #[test]
    fn crlf() {
        check(b"a\r\nFoo", 3, 1, 0);
    }

    #[test]
    fn past_line_end() {
        let text = b"ab\n";
        let index = LineIndex::new(text);
        assert_eq!(index.offset(text, 0, 99), None);
        assert_eq!(index.offset(text, 5, 0), None);
    }

    #[test]
    fn invalid_utf8() {
        check(b"\xE8\xE8Foo", 2, 0, 2);
    }
}
