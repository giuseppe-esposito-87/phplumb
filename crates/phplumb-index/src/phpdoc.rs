//! PHPDoc tags and type expressions, reduced to what navigation needs.

use serde::{Deserialize, Serialize};

/// A type as navigation needs it. Names are fully qualified, without a leading
/// backslash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    Named(String),
    /// `self`
    SelfType,
    /// `static` and `$this`
    StaticType,
    /// `parent`
    ParentType,
    /// `X[]`, `array<X>`, `array<K, X>`, `iterable<X>`, `list<X>`
    ArrayOf(Box<TypeExpr>),
    /// Unions and intersections: navigation treats both as "any of these".
    Union(Vec<TypeExpr>),
    /// A class carrying a framework tag, such as a storage that knows its entity
    /// type. Everything that reads classes from a type treats it as its class.
    Tagged {
        class: String,
        tag: String,
    },
    /// Scalars, `mixed`, `null`, shapes, callables and anything unknown.
    Other,
}

/// Resolves a class name as written in a file (not fully qualified) to its fully
/// qualified form.
pub trait NameContext {
    fn resolve_class(&self, name: &str) -> String;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocBlock {
    /// Text before the first tag, lines joined with spaces.
    pub summary: String,
    /// `@var Type [$name]`, names without `$`.
    pub var: Vec<(Option<String>, TypeExpr)>,
    /// `@param Type $name`, names without `$`.
    pub params: Vec<(String, TypeExpr)>,
    /// `@return Type`
    pub returns: Option<TypeExpr>,
}

const NON_CLASS: &[&str] = &[
    "int",
    "integer",
    "string",
    "bool",
    "boolean",
    "float",
    "double",
    "array",
    "mixed",
    "void",
    "false",
    "true",
    "object",
    "callable",
    "iterable",
    "never",
    "resource",
    "scalar",
    "numeric",
    "list",
    "non-empty-array",
    "non-empty-list",
    "non-empty-string",
    "positive-int",
    "negative-int",
    "class-string",
    "array-key",
    "key-of",
    "value-of",
    "closed-resource",
    "literal-string",
    "callable-string",
    "numeric-string",
    "lowercase-string",
    "noreturn",
];

const ARRAY_LIKE: &[&str] = &[
    "array",
    "iterable",
    "list",
    "non-empty-array",
    "non-empty-list",
];

pub fn parse_type(text: &str, names: &dyn NameContext) -> TypeExpr {
    let mut parser = TypeParser {
        text: text.trim().as_bytes(),
        pos: 0,
        names,
    };
    finish(parser.union())
}

/// Parsed form before `null` is folded away.
enum Parsed {
    Type(TypeExpr),
    Null,
}

fn finish(parsed: Parsed) -> TypeExpr {
    match parsed {
        Parsed::Type(ty) => ty,
        Parsed::Null => TypeExpr::Other,
    }
}

struct TypeParser<'a> {
    text: &'a [u8],
    pos: usize,
    names: &'a dyn NameContext,
}

impl TypeParser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.pos).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        self.skip_spaces();
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn union(&mut self) -> Parsed {
        let mut parts = vec![self.intersection()];
        while self.eat(b'|') {
            parts.push(self.intersection());
        }
        merge(parts)
    }

    fn intersection(&mut self) -> Parsed {
        let mut parts = vec![self.postfix()];
        while self.peek_intersection() {
            self.pos += 1;
            parts.push(self.postfix());
        }
        merge(parts)
    }

    /// `&` followed by a type, not `&$name` or `&...` in a parameter tag.
    fn peek_intersection(&mut self) -> bool {
        self.skip_spaces();
        self.peek() == Some(b'&') && !matches!(self.text.get(self.pos + 1), Some(b'$') | Some(b'.'))
    }

    fn postfix(&mut self) -> Parsed {
        let mut parsed = self.primary();
        loop {
            self.skip_spaces();
            if self.text[self.pos..].starts_with(b"[]") {
                self.pos += 2;
                parsed = match parsed {
                    Parsed::Type(ty) => Parsed::Type(array_of(ty)),
                    Parsed::Null => Parsed::Type(TypeExpr::Other),
                };
            } else {
                return parsed;
            }
        }
    }

    fn primary(&mut self) -> Parsed {
        self.skip_spaces();
        match self.peek() {
            Some(b'?') => {
                self.pos += 1;
                self.postfix()
            }
            Some(b'(') => {
                self.pos += 1;
                let inner = self.union();
                self.eat(b')');
                inner
            }
            Some(b'$') => {
                let word = self.word();
                Parsed::Type(if word == "$this" {
                    TypeExpr::StaticType
                } else {
                    TypeExpr::Other
                })
            }
            Some(b) if b == b'\\' || b == b'_' || b.is_ascii_alphabetic() => self.named(),
            Some(b'"') | Some(b'\'') => {
                self.skip_quoted();
                Parsed::Type(TypeExpr::Other)
            }
            Some(_) => {
                self.word();
                Parsed::Type(TypeExpr::Other)
            }
            None => Parsed::Type(TypeExpr::Other),
        }
    }

    fn named(&mut self) -> Parsed {
        let word = self.word();
        self.skip_spaces();
        let args = match self.peek() {
            Some(b'<') => {
                self.pos += 1;
                Some(self.generic_args())
            }
            Some(b'{') => {
                self.skip_balanced(b'{', b'}');
                return Parsed::Type(TypeExpr::Other);
            }
            Some(b'(') => {
                self.skip_balanced(b'(', b')');
                if self.eat(b':') {
                    self.postfix();
                }
                return Parsed::Type(TypeExpr::Other);
            }
            _ => None,
        };

        let lower = word.to_ascii_lowercase();
        if ARRAY_LIKE.contains(&lower.as_str()) {
            return Parsed::Type(match args.and_then(|mut args| args.pop()) {
                Some(Parsed::Type(element)) => array_of(element),
                _ => TypeExpr::Other,
            });
        }
        Parsed::Type(match lower.as_str() {
            "null" => return Parsed::Null,
            "self" => TypeExpr::SelfType,
            "static" => TypeExpr::StaticType,
            "parent" => TypeExpr::ParentType,
            _ if NON_CLASS.contains(&lower.as_str()) => TypeExpr::Other,
            _ => match word.strip_prefix('\\') {
                Some(qualified) => TypeExpr::Named(qualified.to_string()),
                None => TypeExpr::Named(self.names.resolve_class(&word)),
            },
        })
    }

    fn generic_args(&mut self) -> Vec<Parsed> {
        let mut args = vec![self.union()];
        while self.eat(b',') {
            args.push(self.union());
        }
        self.eat(b'>');
        args
    }

    fn word(&mut self) -> String {
        let start = self.pos;
        if self.peek() == Some(b'$') {
            self.pos += 1;
        }
        while self
            .peek()
            .is_some_and(|b| b == b'\\' || b == b'_' || b == b'-' || b.is_ascii_alphanumeric())
        {
            self.pos += 1;
        }
        if self.pos == start {
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.text[start..self.pos]).into_owned()
    }

    fn skip_balanced(&mut self, open: u8, close: u8) {
        let mut depth = 0;
        while let Some(b) = self.peek() {
            self.pos += 1;
            if b == open {
                depth += 1;
            } else if b == close {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
        }
    }

    fn skip_quoted(&mut self) {
        let quote = self.peek();
        self.pos += 1;
        while let Some(b) = self.peek() {
            self.pos += 1;
            if Some(b) == quote {
                return;
            }
        }
    }
}

fn array_of(element: TypeExpr) -> TypeExpr {
    match element {
        TypeExpr::Other => TypeExpr::Other,
        element => TypeExpr::ArrayOf(Box::new(element)),
    }
}

/// Folds a union: nested unions flatten, `null` and non-class members drop, and a
/// single remaining member stands alone.
fn merge(parts: Vec<Parsed>) -> Parsed {
    if parts.len() == 1 {
        return parts.into_iter().next().expect("one part");
    }
    let mut members: Vec<TypeExpr> = Vec::new();
    for part in parts {
        match part {
            Parsed::Null | Parsed::Type(TypeExpr::Other) => {}
            Parsed::Type(TypeExpr::Union(inner)) => members.extend(inner),
            Parsed::Type(ty) => members.push(ty),
        }
    }
    members.dedup();
    Parsed::Type(match members.len() {
        0 => TypeExpr::Other,
        1 => members.pop().expect("one member"),
        _ => TypeExpr::Union(members),
    })
}

/// Splits the leading type off a tag's text, keeping spaces inside brackets.
fn split_type(text: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, c) in text.char_indices() {
        match c {
            '<' | '{' | '(' | '[' => depth += 1,
            '>' | '}' | ')' | ']' => depth -= 1,
            c if c.is_whitespace() && depth <= 0 => return (&text[..i], text[i..].trim_start()),
            _ => {}
        }
    }
    (text, "")
}

/// A `$name` at the start of `text`, without the `$`, `&` or `...`.
fn variable_name(text: &str) -> Option<String> {
    let text = text.trim_start_matches(['&', '.']);
    let name = text.strip_prefix('$')?;
    let end = name
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(name.len());
    (end > 0).then(|| name[..end].to_string())
}

pub fn parse_docblock(text: &str, names: &dyn NameContext) -> DocBlock {
    let body = text.trim().trim_start_matches("/**").trim_end_matches("*/");
    let lines = body.lines().map(|line| {
        let line = line.trim();
        line.strip_prefix('*').map(str::trim_start).unwrap_or(line)
    });

    let mut summary: Vec<&str> = Vec::new();
    let mut in_summary = true;
    // Plain tags first, then the analyzer-specific ones, which win.
    let mut plain = DocBlock::default();
    let mut specific = DocBlock::default();
    for line in lines {
        let Some(tag_line) = line.strip_prefix('@') else {
            if in_summary && !line.is_empty() {
                summary.push(line);
            } else if in_summary && !summary.is_empty() {
                in_summary = false;
            }
            continue;
        };
        in_summary = false;
        let (tag, rest) = tag_line
            .split_once(char::is_whitespace)
            .unwrap_or((tag_line, ""));
        let (target, tag) = match tag
            .strip_prefix("phpstan-")
            .or_else(|| tag.strip_prefix("psalm-"))
        {
            Some(tag) => (&mut specific, tag),
            None => (&mut plain, tag),
        };
        let (type_text, after) = split_type(rest.trim());
        match tag {
            "var" => target
                .var
                .push((variable_name(after), parse_type(type_text, names))),
            "param" => {
                if let Some(name) = variable_name(after) {
                    target.params.push((name, parse_type(type_text, names)));
                }
            }
            "return" => target.returns = Some(parse_type(type_text, names)),
            _ => {}
        }
    }

    let mut params = plain.params;
    for (name, ty) in specific.params {
        params.retain(|(existing, _)| *existing != name);
        params.push((name, ty));
    }
    DocBlock {
        summary: summary.join(" "),
        var: if specific.var.is_empty() {
            plain.var
        } else {
            specific.var
        },
        params,
        returns: specific.returns.or(plain.returns),
    }
}

#[cfg(test)]
mod tests {
    use super::{DocBlock, NameContext, TypeExpr, parse_docblock, parse_type};

    struct Names;

    impl NameContext for Names {
        fn resolve_class(&self, name: &str) -> String {
            match name {
                "Alias" => "Lib\\Thing".to_string(),
                other => format!("App\\{other}"),
            }
        }
    }

    fn ty(text: &str) -> TypeExpr {
        parse_type(text, &Names)
    }

    fn named(fqn: &str) -> TypeExpr {
        TypeExpr::Named(fqn.to_string())
    }

    #[test]
    fn named_and_qualified() {
        assert_eq!(ty("Foo"), named("App\\Foo"));
        assert_eq!(ty("\\Lib\\Bar"), named("Lib\\Bar"));
        assert_eq!(ty("Alias"), named("Lib\\Thing"));
    }

    #[test]
    fn nullable_and_null_unions() {
        assert_eq!(ty("?Foo"), named("App\\Foo"));
        assert_eq!(ty("Foo|null"), named("App\\Foo"));
        assert_eq!(ty("null|Foo"), named("App\\Foo"));
    }

    #[test]
    fn unions_and_intersections() {
        let both = TypeExpr::Union(vec![named("App\\Foo"), named("Lib\\Bar")]);
        assert_eq!(ty("Foo|\\Lib\\Bar"), both);
        assert_eq!(ty("Foo&\\Lib\\Bar"), both);
        assert_eq!(ty("(Foo&\\Lib\\Bar)|null"), both);
    }

    #[test]
    fn arrays_of_classes() {
        let array = TypeExpr::ArrayOf(Box::new(named("App\\Foo")));
        for text in [
            "Foo[]",
            "array<Foo>",
            "array<int, Foo>",
            "iterable<Foo>",
            "list<Foo>",
            "?Foo[]",
        ] {
            assert_eq!(ty(text), array, "{text}");
        }
    }

    #[test]
    fn self_static_this_parent() {
        assert_eq!(ty("self"), TypeExpr::SelfType);
        assert_eq!(ty("static"), TypeExpr::StaticType);
        assert_eq!(ty("$this"), TypeExpr::StaticType);
        assert_eq!(ty("parent"), TypeExpr::ParentType);
    }

    #[test]
    fn scalars_and_shapes_are_other() {
        for text in [
            "int",
            "array",
            "array{a: int}",
            "callable(int): void",
            "mixed",
            "string|int",
            "class-string<Foo>",
            "",
        ] {
            assert_eq!(ty(text), TypeExpr::Other, "{text}");
        }
    }

    #[test]
    fn generics_keep_the_base() {
        assert_eq!(ty("Collection<Foo>"), named("App\\Collection"));
        assert_eq!(ty("\\Lib\\Map<string, Foo>"), named("Lib\\Map"));
    }

    #[test]
    fn docblock_tags() {
        let doc = parse_docblock(
            "/**\n * Loads it.\n *\n * @param Foo $a\n * @param int $b\n * @return static\n * @var Foo|null $x\n */",
            &Names,
        );

        assert_eq!(
            doc,
            DocBlock {
                summary: "Loads it.".to_string(),
                var: vec![(Some("x".to_string()), named("App\\Foo"))],
                params: vec![
                    ("a".to_string(), named("App\\Foo")),
                    ("b".to_string(), TypeExpr::Other)
                ],
                returns: Some(TypeExpr::StaticType),
            }
        );
    }

    #[test]
    fn phpstan_tags_win() {
        let doc = parse_docblock(
            "/**\n * @return array\n * @phpstan-return Foo[]\n */",
            &Names,
        );

        assert_eq!(
            doc.returns,
            Some(TypeExpr::ArrayOf(Box::new(named("App\\Foo"))))
        );
    }
}
