//! Symfony Messenger: the message handlers `#[AsMessageHandler]` declares, and the
//! messages each one handles.

use std::path::Path;

use crate::filter::PathFilter;
use crate::frameworks::ancestry::is_a;
use crate::index::{Index, Location};
use crate::infer::class_names;
use crate::metadata::{MetaValue, Metadata};
use crate::summary::{Declaration, Member, MemberKind, SymbolKind};

/// The attribute that marks a message handler.
const HANDLER_ATTRIBUTE: &str = "Symfony\\Component\\Messenger\\Attribute\\AsMessageHandler";

/// A handler method and the message classes it handles.
struct Handler {
    location: Location,
    handles: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Handlers {
    list: Vec<Handler>,
}

impl Handlers {
    /// Every handler of the concrete project classes: a class attribute names its
    /// method (`__invoke` by default), a method attribute marks its method.
    pub(crate) fn read(index: &Index, filter: &PathFilter) -> Handlers {
        let mut handlers = Handlers::default();
        for (path, declaration) in index.declarations() {
            if declaration.kind == SymbolKind::Class
                && declares_handlers(declaration)
                && filter.in_scope(path)
                && index.is_project_code(path)
            {
                handlers.read_class(path, declaration);
            }
        }
        handlers
    }

    fn read_class(&mut self, path: &Path, class: &Declaration) {
        let Some(shape) = &class.class else {
            return;
        };
        if shape.is_abstract {
            return;
        }
        let methods = || {
            shape
                .members
                .iter()
                .filter(|m| m.kind == MemberKind::Method)
        };
        for attribute in shape.metadata.iter().filter(|m| is_handler(m)) {
            let name = match argument(attribute, "method", 3) {
                Some(MetaValue::String(method)) => method.as_str(),
                Some(_) => continue,
                None => "__invoke",
            };
            if let Some(method) = methods().find(|m| m.name.eq_ignore_ascii_case(name)) {
                self.add(path, method, attribute);
            }
        }
        for method in methods() {
            for attribute in method.metadata.iter().filter(|m| is_handler(m)) {
                self.add(path, method, attribute);
            }
        }
    }

    /// The handler `method`, handling the attribute's `handles` class or the
    /// classes of its first parameter's type.
    fn add(&mut self, path: &Path, method: &Member, attribute: &Metadata) {
        let handles = match argument(attribute, "handles", 2) {
            Some(MetaValue::Class(class) | MetaValue::String(class)) => {
                vec![class.trim_start_matches('\\').to_string()]
            }
            Some(_) => return,
            None => method
                .params
                .first()
                .and_then(|(_, ty)| ty.as_ref())
                .map(class_names)
                .unwrap_or_default(),
        };
        if handles.is_empty() {
            return;
        }
        self.list.push(Handler {
            location: Location {
                path: path.to_path_buf(),
                start: method.name_start,
                end: method.name_end,
            },
            handles,
        });
    }

    /// The handlers whose handled class `message` is, extends or implements.
    pub(crate) fn of(&self, index: &Index, message: &str) -> Vec<Location> {
        let mut found: Vec<Location> = Vec::new();
        for handler in &self.list {
            let handles = handler
                .handles
                .iter()
                .any(|class| is_a(index, message, &[class]));
            if handles && !found.contains(&handler.location) {
                found.push(handler.location.clone());
            }
        }
        found
    }

    /// The classes the handler method whose name starts at `start` in `path`
    /// handles; none when it is no handler.
    pub(crate) fn handled_by(&self, path: &Path, start: u32) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        for handler in &self.list {
            if handler.location.path == path && handler.location.start == start {
                for class in &handler.handles {
                    if !found.contains(class) {
                        found.push(class.clone());
                    }
                }
            }
        }
        found
    }
}

/// The method a class-level `#[AsMessageHandler]` names, when `attribute` is one.
pub(crate) fn handler_method<'a>(
    class: &'a Declaration,
    attribute: &Metadata,
) -> Option<&'a Member> {
    if !is_handler(attribute) {
        return None;
    }
    let name = match argument(attribute, "method", 3) {
        Some(MetaValue::String(method)) => method.as_str(),
        Some(_) => return None,
        None => "__invoke",
    };
    class
        .class
        .as_ref()?
        .members
        .iter()
        .find(|m| m.kind == MemberKind::Method && m.name.eq_ignore_ascii_case(name))
}

fn is_handler(metadata: &Metadata) -> bool {
    !metadata.annotation && metadata.name.eq_ignore_ascii_case(HANDLER_ATTRIBUTE)
}

/// Whether the class or one of its methods carries `#[AsMessageHandler]`.
fn declares_handlers(class: &Declaration) -> bool {
    class.class.as_ref().is_some_and(|shape| {
        shape.metadata.iter().any(is_handler)
            || shape
                .members
                .iter()
                .any(|m| m.metadata.iter().any(is_handler))
    })
}

/// The argument `name`, given by name or at `position`.
fn argument<'a>(attribute: &'a Metadata, name: &str, position: usize) -> Option<&'a MetaValue> {
    attribute.argument(name).or_else(|| {
        attribute
            .arguments
            .get(position)
            .filter(|(key, _)| key.is_none())
            .map(|(_, value)| value)
    })
}
