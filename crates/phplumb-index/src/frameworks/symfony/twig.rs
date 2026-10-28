//! Names written in Twig templates: string literals inside `{{ }}` and `{% %}`.

/// Tags whose first argument is a template name.
const TEMPLATE_TAGS: [&str; 6] = ["extends", "include", "embed", "import", "from", "use"];

/// Tags whose first argument may be a list of template names, the first existing
/// one being used.
const LIST_TAGS: [&str; 2] = ["extends", "include"];

/// What a name written in a template stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TwigName {
    Route,
    Template,
}

/// Tags whose content Twig outputs as written.
const VERBATIM_TAGS: [(&str, &str); 2] = [("verbatim", "endverbatim"), ("raw", "endraw")];

/// A string literal inside a Twig expression or tag.
struct Literal {
    /// The range between the delimiters of the `{{ }}` or `{% %}` around it.
    block: (usize, usize),
    /// Whether that block is a `{% %}` tag.
    tag: bool,
    /// The opening quote, and the range inside the quotes.
    quote: usize,
    content: usize,
    close: usize,
}

impl Literal {
    fn value(&self, source: &[u8]) -> (String, u32, u32) {
        (
            String::from_utf8_lossy(&source[self.content..self.close]).into_owned(),
            self.content as u32,
            self.close as u32,
        )
    }
}

/// The route name under `offset`: a string literal that is the whole first
/// argument of `path(` or `url(`, with its range inside the quotes.
pub(crate) fn route_name_at(source: &[u8], offset: u32) -> Option<(String, u32, u32)> {
    let literal = literal_at(source, offset as usize)?;
    is_function_argument(source, &literal, &["path", "url"]).then(|| literal.value(source))
}

/// The template name under `offset`: the whole first argument of `include(` or
/// `source(`, or the template a tag names, as an element of a list for `extends`
/// and `include`.
pub(crate) fn template_name_at(source: &[u8], offset: u32) -> Option<(String, u32, u32)> {
    let literal = literal_at(source, offset as usize)?;
    (is_function_argument(source, &literal, &["include", "source"])
        || is_tag_argument(source, &literal))
    .then(|| literal.value(source))
}

/// Every route name and template name `source` writes as a whole literal argument,
/// with its range inside the quotes. Names a template may lack, in an element of a
/// list or with `ignore missing`, are left out, as is text inside `verbatim`.
pub(crate) fn names(source: &[u8]) -> Vec<(TwigName, String, u32, u32)> {
    let mut found = Vec::new();
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
        i = end + 2;
        if close == b"#}" {
            continue;
        }
        let tag = close == b"%}";
        let text = String::from_utf8_lossy(&source[start..end]);
        let words: Vec<&str> = text.split_whitespace().collect();
        if tag
            && let Some((_, closing)) = VERBATIM_TAGS.iter().find(|(opening, _)| {
                words.first().map(|w| w.trim_start_matches(['-', '~'])) == Some(*opening)
            })
        {
            // Up to the closing tag, whatever its spacing.
            while let Some(next) = find(source, i, b"{%") {
                let after = find(source, next + 2, b"%}").unwrap_or(source.len());
                let inner = String::from_utf8_lossy(&source[next + 2..after]);
                i = after + 2;
                if inner.trim().trim_matches(['-', '~']).trim() == *closing {
                    break;
                }
            }
            continue;
        }
        let optional =
            words.join(" ").contains("ignore missing") || text.contains("ignore_missing");
        for literal in block_literals(source, start, end, tag) {
            // `#{...}` interpolates an expression into a double-quoted string.
            let interpolated = source[literal.quote] == b'"'
                && find(source, literal.content, b"#{").is_some_and(|at| at < literal.close);
            if interpolated {
                continue;
            }
            // `ignore_missing` is the 4th argument of `include()`, the 2nd of `source()`.
            let lenient = |function: &str, arguments: usize| {
                is_function_argument(source, &literal, &[function])
                    && argument_count(source, &literal) >= arguments
            };
            let kind = if is_function_argument(source, &literal, &["path", "url"]) {
                TwigName::Route
            } else if optional || lenient("include", 4) || lenient("source", 2) {
                continue;
            } else if is_function_argument(source, &literal, &["include", "source"])
                || (is_tag_argument(source, &literal) && !in_list(source, &literal))
            {
                TwigName::Template
            } else {
                continue;
            };
            let (name, start, end) = literal.value(source);
            found.push((kind, name, start, end));
        }
    }
    found
}

/// How many arguments the call whose first argument is the literal has, counting
/// the commas at its own depth up to its closing parenthesis.
fn argument_count(source: &[u8], literal: &Literal) -> usize {
    let mut count = 1;
    let mut depth = 0;
    let mut i = literal.close + 1;
    while i < literal.block.1 {
        match source[i] {
            quote @ (b'\'' | b'"') => {
                i += 1;
                while i < literal.block.1 && source[i] != quote {
                    i += if source[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth == 0 => break,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => count += 1,
            _ => {}
        }
        i += 1;
    }
    count
}

/// Whether the literal is an element of a list rather than the tag's argument.
fn in_list(source: &[u8], literal: &Literal) -> bool {
    let mut i = literal.quote;
    while i > literal.block.0 && source[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    i > literal.block.0 && matches!(source[i - 1], b'[' | b',')
}

/// The string literal under `offset` inside a `{{ }}` or `{% %}`.
fn literal_at(source: &[u8], offset: usize) -> Option<Literal> {
    let (start, end, tag) = block_at(source, offset)?;
    block_literals(source, start, end, tag)
        .into_iter()
        .find(|literal| literal.content <= offset && offset <= literal.close)
}

/// The string literals of the block between `start` and `end`.
fn block_literals(source: &[u8], start: usize, end: usize, tag: bool) -> Vec<Literal> {
    let mut found = Vec::new();
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
        found.push(Literal {
            block: (start, end),
            tag,
            quote: i,
            content,
            close,
        });
        i = close + 1;
    }
    found
}

/// The `{{ }}` or `{% %}` around `offset`: the range between its delimiters, and
/// whether it is a tag. Comments hold no names.
fn block_at(source: &[u8], offset: usize) -> Option<(usize, usize, bool)> {
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
            return Some((start, end, close == b"%}"));
        }
        if start > offset {
            return None;
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

fn skip_spaces(source: &[u8], mut i: usize, end: usize) -> usize {
    while i < end && source[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Whether the next character after the literal, past spaces, is one of `allowed`.
fn followed_by(source: &[u8], literal: &Literal, allowed: &[u8]) -> bool {
    let after = skip_spaces(source, literal.close + 1, literal.block.1);
    after < literal.block.1 && allowed.contains(&source[after])
}

/// Whether the literal is the whole first argument of one of `functions`: `name(`
/// right before it, the name not part of a longer name or a method call, and `,` or
/// `)` right after it.
fn is_function_argument(source: &[u8], literal: &Literal, functions: &[&str]) -> bool {
    let block = literal.block.0;
    let mut i = literal.quote;
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
    !attached
        && functions.iter().any(|f| f.as_bytes() == name)
        && followed_by(source, literal, b",)")
}

/// Whether the literal is the template a tag names: the first argument of one of
/// the template tags, or an element of the list right after `extends` or
/// `include`. A literal that is part of a larger expression is not.
fn is_tag_argument(source: &[u8], literal: &Literal) -> bool {
    if !literal.tag {
        return false;
    }
    let (start, end) = literal.block;
    let mut i = skip_spaces(source, start, end);
    if i < end && matches!(source[i], b'-' | b'~') {
        i = skip_spaces(source, i + 1, end);
    }
    let word_start = i;
    while i < end && source[i].is_ascii_alphabetic() {
        i += 1;
    }
    let Ok(tag) = std::str::from_utf8(&source[word_start..i]) else {
        return false;
    };
    if !TEMPLATE_TAGS.contains(&tag) {
        return false;
    }
    let argument = skip_spaces(source, i, end);
    if argument == literal.quote {
        return !followed_by(source, literal, b"~|.+[(");
    }
    if LIST_TAGS.contains(&tag) && argument < end && source[argument] == b'[' {
        // Only a direct element of that first list, not of a later one.
        let mut depth = 0;
        let mut k = argument + 1;
        while k < literal.quote {
            match source[k] {
                quote @ (b'\'' | b'"') => {
                    k += 1;
                    while k < literal.quote && source[k] != quote {
                        k += if source[k] == b'\\' { 2 } else { 1 };
                    }
                }
                b'[' | b'{' | b'(' => depth += 1,
                b']' | b'}' | b')' => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            k += 1;
        }
        if depth != 0 {
            return false;
        }
        let mut before = literal.quote;
        while before > argument && source[before - 1].is_ascii_whitespace() {
            before -= 1;
        }
        let in_list = before > argument && matches!(source[before - 1], b'[' | b',');
        return in_list && followed_by(source, literal, b",]");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{route_name_at, template_name_at};

    fn route(source: &str, needle: &str) -> Option<String> {
        let offset = source.find(needle).unwrap() as u32 + 1;
        route_name_at(source.as_bytes(), offset).map(|(name, _, _)| name)
    }

    fn template(source: &str, needle: &str) -> Option<String> {
        let offset = source.find(needle).unwrap() as u32 + 1;
        template_name_at(source.as_bytes(), offset).map(|(name, _, _)| name)
    }

    #[test]
    fn reads_route_names() {
        assert_eq!(route("{{ path('home') }}", "'home'"), Some("home".into()));
        assert_eq!(route("{% if url( \"a\" ) %}", "\"a\""), Some("a".into()));
        assert_eq!(route("{{ x.path('home') }}", "'home'"), None);
        assert_eq!(route("{{ mypath('home') }}", "'home'"), None);
        assert_eq!(route("{# path('home') #}", "'home'"), None);
        assert_eq!(route("path('home')", "'home'"), None);
        assert_eq!(route("{{ path('a', {b: 'c'}) }}", "'c'"), None);
        assert_eq!(route("{{ path('a' ~ b) }}", "'a'"), None);
    }

    #[test]
    fn reads_template_names() {
        assert_eq!(template("{% extends 'a' %}", "'a'"), Some("a".into()));
        assert_eq!(
            template("{%- include \"a\" with {} -%}", "\"a\""),
            Some("a".into())
        );
        assert_eq!(template("{% from 'a' import b %}", "'a'"), Some("a".into()));
        assert_eq!(
            template("{% extends ['x', 'a'] %}", "'a'"),
            Some("a".into())
        );
        assert_eq!(template("{{ include('a') }}", "'a'"), Some("a".into()));
        assert_eq!(template("{{ source( 'a' ) }}", "'a'"), Some("a".into()));
        assert_eq!(template("{% include 'a' ~ b %}", "'a'"), None);
        assert_eq!(template("{% set x = 'a' %}", "'a'"), None);
        assert_eq!(template("{% embed 'a' with {b: 'c'} %}", "'c'"), None);
        assert_eq!(template("{% import ['a'] as x %}", "'a'"), None);
        assert_eq!(template("{{ 'a' }}", "'a'"), None);
        assert_eq!(
            template("{% include ['a'] with {items: ['b']} %}", "'b'"),
            None
        );
        assert_eq!(template("{% include ['a'] with ['x'] %}", "'x'"), None);
    }
}
