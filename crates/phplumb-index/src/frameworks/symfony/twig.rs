//! Names written in Twig templates: string literals inside `{{ }}` and `{% %}`.

/// The route name under `offset`: a string literal that is the first argument of
/// `path(` or `url(` inside `{{ }}` or `{% %}`, with its range inside the quotes.
pub(crate) fn route_name_at(source: &[u8], offset: u32) -> Option<(String, u32, u32)> {
    first_argument_at(source, offset, &["path", "url"])
}

/// The string literal under `offset` that is the first argument of one of
/// `functions` inside a Twig tag or expression, with its range inside the quotes.
fn first_argument_at(source: &[u8], offset: u32, functions: &[&str]) -> Option<(String, u32, u32)> {
    let offset = offset as usize;
    let (start, end) = block_at(source, offset)?;
    let mut i = start;
    while i < end {
        let quote = source[i];
        if quote != b'\'' && quote != b'"' {
            i += 1;
            continue;
        }
        let content = i + 1;
        let mut j = content;
        while j < end && source[j] != quote {
            j += if source[j] == b'\\' { 2 } else { 1 };
        }
        let close = j.min(end);
        if content <= offset && offset <= close {
            // The literal must be the whole argument: `'a' ~ b` or `'a'|lower` is not.
            let mut after = close + 1;
            while after < end && source[after].is_ascii_whitespace() {
                after += 1;
            }
            let whole = after < end && matches!(source[after], b',' | b')');
            if !whole || !called_with(source, start, i, functions) {
                return None;
            }
            let value = String::from_utf8_lossy(&source[content..close]).into_owned();
            return Some((value, content as u32, close as u32));
        }
        i = close + 1;
    }
    None
}

/// The `{{ }}` or `{% %}` around `offset`, as the range between its delimiters.
fn block_at(source: &[u8], offset: usize) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + 1 < source.len() {
        let close: &[u8] = match &source[i..i + 2] {
            b"{{" => b"}}",
            b"{%" => b"%}",
            b"{#" => b"#}",
            _ => {
                i += 1;
                continue;
            }
        };
        let start = i + 2;
        let end = find(source, start, close).unwrap_or(source.len());
        if close != b"#}" && start <= offset && offset <= end {
            return Some((start, end));
        }
        i = end + 2;
    }
    None
}

fn find(source: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    source[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

/// Whether the literal opening at `quote` is the first argument of one of
/// `functions`: `name(` right before it, the name not part of a longer name or a
/// method call.
fn called_with(source: &[u8], block: usize, quote: usize, functions: &[&str]) -> bool {
    let mut i = quote;
    while i > block && source[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    if i == block || source[i - 1] != b'(' {
        return false;
    }
    i -= 1;
    while i > block && source[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    let name_end = i;
    while i > block && (source[i - 1].is_ascii_alphanumeric() || source[i - 1] == b'_') {
        i -= 1;
    }
    let name = &source[i..name_end];
    let attached = i > block && source[i - 1] == b'.';
    !attached && functions.iter().any(|f| f.as_bytes() == name)
}

#[cfg(test)]
mod tests {
    use super::route_name_at;

    fn at(source: &str, needle: &str) -> Option<String> {
        let offset = source.find(needle).unwrap() as u32 + 1;
        route_name_at(source.as_bytes(), offset).map(|(name, _, _)| name)
    }

    #[test]
    fn reads_route_names() {
        assert_eq!(at("{{ path('home') }}", "'home'"), Some("home".into()));
        assert_eq!(at("{% if url( \"a\" ) %}", "\"a\""), Some("a".into()));
        assert_eq!(at("{{ x.path('home') }}", "'home'"), None);
        assert_eq!(at("{{ mypath('home') }}", "'home'"), None);
        assert_eq!(at("{# path('home') #}", "'home'"), None);
        assert_eq!(at("path('home')", "'home'"), None);
        assert_eq!(at("{{ path('a', {b: 'c'}) }}", "'c'"), None);
    }
}
