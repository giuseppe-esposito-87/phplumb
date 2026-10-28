//! Route names written in attributes, as both Drupal and Symfony read them.

use crate::metadata::{MetaValue, Metadata};

/// Route attribute classes, whose `name` argument (named or at position 1) names a
/// route.
const ROUTE_ATTRIBUTES: [&str; 2] = [
    "Symfony\\Component\\Routing\\Attribute\\Route",
    "Symfony\\Component\\Routing\\Annotation\\Route",
];

/// The name a route attribute gives.
pub(crate) enum RouteName {
    Literal(String),
    /// A constant or an expression.
    Unreadable,
    Missing,
}

/// The names of the route attributes in `metadata`: the `name` argument, named or at
/// position 1.
pub(crate) fn route_names(metadata: &[Metadata]) -> Vec<RouteName> {
    metadata
        .iter()
        .filter(|m| {
            !m.annotation
                && ROUTE_ATTRIBUTES
                    .iter()
                    .any(|a| m.name.eq_ignore_ascii_case(a))
        })
        .map(|attribute| {
            let name = attribute.argument("name").or_else(|| {
                attribute
                    .arguments
                    .get(1)
                    .filter(|(key, _)| key.is_none())
                    .map(|(_, value)| value)
            });
            match name {
                Some(MetaValue::String(name)) => RouteName::Literal(name.clone()),
                Some(_) => RouteName::Unreadable,
                None => RouteName::Missing,
            }
        })
        .collect()
}
