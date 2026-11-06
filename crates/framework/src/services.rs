//! Reading service definitions out of a container's YAML.
//!
//! A line scanner rather than a YAML parser, and deliberately: what is needed here is two keys of a
//! two-level mapping, the files are machine-written and uniform, and a full YAML implementation is a
//! large dependency plus a parse of every list, tag and anchor in a file to reach `class:`. The
//! previous attempt did the same and the choice was never the thing that went wrong.
//!
//! Shared by every framework whose container is configured this way -- Drupal and Symfony write the
//! same mapping -- so the shapes live here and the file locations live with each provider.

/// One service, as its definition names it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ServiceDefinition {
    pub id: String,
    pub names: ServiceTarget,
}

/// What a definition says the service is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ServiceTarget {
    /// A class, fully qualified and without a leading separator.
    Class(String),
    /// Another service id, which may be defined in a different file -- so this is resolved by the
    /// caller, once every file has been read.
    Alias(String),
    /// The id is registered and its class is not knowable from the configuration.
    ///
    /// A `factory:`, a class named by a `%parameter%`, a `parent:` whose own class is one of those.
    /// Kept rather than dropped, because "this id exists" and "this id builds that class" are two
    /// facts and only the second one is missing. Dropping the definition made a registered id look
    /// unregistered, which is a report on correct code -- found exactly that way, by the report.
    Unknown,
}

/// One container parameter, and the shape of the value its configuration gives it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParameterDefinition {
    pub name: String,
    pub shape: phrost_model::BuiltinTy,
}

/// Every parameter one configuration file declares, with the shape of its value.
///
/// The other half of the same file, and the reason it is worth reading: `getParameter()` is declared
/// `array|bool|string|int|float|\UnitEnum|null` -- every kind a configuration value can be, because
/// the signature cannot say which this one is -- so a project passing `getParameter('serializer.
/// formats')` to an `array` parameter reads as unchecked. The YAML says which: a sequence or a
/// mapping is an array, `true` is a bool, a number is an int or a float, and anything else is a
/// string.
///
/// Only the *shape*, never the value. What a parameter holds is configuration and changes per
/// environment; that it holds a list does not.
pub fn parameters_in(text: &str) -> Vec<ParameterDefinition> {
    let mut found: Vec<ParameterDefinition> = Vec::new();
    let mut in_parameters = false;
    let mut name_indent = None;

    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((indent, content)) = key_line(line) else {
            continue;
        };

        if indent == 0 {
            in_parameters = content.starts_with("parameters:");
            name_indent = None;
            continue;
        }
        if !in_parameters {
            continue;
        }
        // The names are the block's own first level; anything deeper belongs to a value that is
        // already known to be a mapping.
        let depth = *name_indent.get_or_insert(indent);
        if indent > depth {
            continue;
        }
        let Some((name, rest)) = content.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.contains(' ') {
            continue;
        }
        // A value on the same line says its own shape; an empty one is a block, and the next
        // non-blank line says which kind of block.
        let shape = match rest.trim() {
            // A block under the name, whether a sequence or a mapping: both are arrays in PHP.
            "" => match lines.peek().and_then(|next| key_line(next)) {
                Some((deeper, _)) if deeper > depth => phrost_model::BuiltinTy::Array,
                // Nothing under it and nothing beside it: a name with no value at all.
                _ => continue,
            },
            value => scalar_shape(value),
        };
        found.push(ParameterDefinition {
            name: name.to_owned(),
            shape,
        });
    }

    found
}

/// The shape of a scalar as YAML writes it.
///
/// A flow sequence or mapping first, because both are written on the value's own line: `[json, xml]`
/// and `{a: 1}` are arrays as surely as an indented block is. Then the two literals PHP and YAML
/// spell the same way, then a number, and a string for everything else -- which is where a quoted
/// value, a `%parameter%` reference and an unquoted word all land.
fn scalar_shape(value: &str) -> phrost_model::BuiltinTy {
    use phrost_model::BuiltinTy;

    if value.starts_with('[') || value.starts_with('{') {
        return BuiltinTy::Array;
    }
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "on" | "off"
    ) {
        return BuiltinTy::Bool;
    }
    if value.parse::<i64>().is_ok() {
        return BuiltinTy::Int;
    }
    if value.parse::<f64>().is_ok() {
        return BuiltinTy::Float;
    }
    BuiltinTy::String
}

/// How many aliases are followed before giving up.
///
/// A bound rather than a guess about depth: real chains are one or two long, and a container written
/// with a cycle must not be walked forever.
pub const ALIAS_DEPTH: u8 = 8;

/// Every service one configuration file defines.
///
/// Only the top-level `services:` block is read. That is not tidiness: Drupal's own
/// `core.services.yml` has a `parameters:` entry whose nested mappings each carry a `class:` key --
/// `core.moved_classes` lists a dozen of them -- and a scanner that looked for `class:` anywhere
/// would register a service for every deprecated class name in core.
pub fn services_in(text: &str) -> Vec<ServiceDefinition> {
    let mut found = Vec::new();
    let mut in_services = false;
    let mut id_indent = None;
    let mut current: Option<Pending> = None;

    for line in text.lines() {
        let Some((indent, content)) = key_line(line) else {
            continue;
        };

        // A top-level key either opens the services block or ends it. `parameters:` after
        // `services:` is the common case, and reading on would treat its contents as services.
        if indent == 0 {
            found.extend(current.take().and_then(Pending::finish));
            in_services = content.starts_with("services:");
            id_indent = None;
            continue;
        }
        if !in_services {
            continue;
        }

        let depth = *id_indent.get_or_insert(indent);
        if indent > depth {
            if let Some(pending) = current.as_mut() {
                pending.property(content);
            }
            continue;
        }
        // A sibling of the previous id, or the first one: whatever was being read is complete.
        found.extend(current.take().and_then(Pending::finish));

        let Some((id, value)) = split_key(content) else {
            continue;
        };
        // `_defaults` and `_instanceof` configure the block rather than declaring a service.
        if id.starts_with('_') {
            continue;
        }
        current = Some(Pending::new(unquote(id).to_owned(), value));
    }

    found.extend(current.and_then(Pending::finish));
    found
}

/// Resolves `Alias` targets against the definitions themselves.
///
/// Separate from reading, because an alias may name a service another file defines: Drupal's
/// `plugin.manager.element_info` aliases something registered by a different module, and resolving
/// during the scan would drop it depending on which file was read first.
pub fn resolve_aliases(definitions: Vec<ServiceDefinition>) -> Vec<(String, Option<String>)> {
    let classes = definitions
        .iter()
        .filter_map(|definition| match &definition.names {
            ServiceTarget::Class(class) => Some((definition.id.as_str(), class.as_str())),
            ServiceTarget::Alias(_) | ServiceTarget::Unknown => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    let aliases = definitions
        .iter()
        .filter_map(|definition| match &definition.names {
            ServiceTarget::Alias(target) => Some((definition.id.as_str(), target.as_str())),
            ServiceTarget::Class(_) | ServiceTarget::Unknown => None,
        })
        .collect::<std::collections::HashMap<_, _>>();

    // Every definition comes back, class or no class. An id whose class is not knowable is still a
    // registered id, and collapsing the two made a report say that a declared service does not
    // exist.
    definitions
        .iter()
        .map(|definition| {
            let class = match &definition.names {
                ServiceTarget::Class(class) => Some(class.clone()),
                ServiceTarget::Unknown => None,
                ServiceTarget::Alias(target) => {
                    let mut at = target.as_str();
                    let mut left = ALIAS_DEPTH;
                    loop {
                        if let Some(class) = classes.get(at) {
                            break Some((*class).to_owned());
                        }
                        match left.checked_sub(1).and_then(|next| {
                            left = next;
                            aliases.get(at)
                        }) {
                            Some(next) => at = next,
                            // A chain that leaves the configuration, or goes round: the id is
                            // registered and its class is not to be had.
                            None => break None,
                        }
                    }
                }
            };
            (definition.id.clone(), class)
        })
        .collect()
}

/// One service id whose properties are still being read.
struct Pending {
    id: String,
    class: Option<String>,
    alias: Option<String>,
    /// A `parent:` template's id. Symfony's inheritance: the child takes the parent's class unless it
    /// names its own, which is how every Drupal logger channel is declared.
    parent: Option<String>,
}

impl Pending {
    /// The value written beside the id, which for two shorthands is the whole definition.
    fn new(id: String, value: &str) -> Self {
        let mut pending = Self {
            id,
            class: None,
            alias: None,
            parent: None,
        };
        // `mine.thing: '@other.thing'` -- the alias shorthand.
        if let Some(target) = reference(value) {
            pending.alias = Some(target);
        }
        pending
    }

    fn property(&mut self, content: &str) {
        let Some((key, value)) = split_key(content) else {
            return;
        };
        match key {
            "class" => self.class = class_name(value),
            "alias" => self.alias = reference(value).or_else(|| plain(value)),
            "parent" => self.parent = reference(value).or_else(|| plain(value)),
            _ => {}
        }
    }

    /// What this definition names.
    ///
    /// Always something, because the id is registered whatever else is or is not knowable. A service
    /// built by a `factory:`, or whose class is a `%parameter%`, has no class here -- and that is a
    /// different answer from not being registered at all, which is the distinction a report rests on.
    fn finish(self) -> Option<ServiceDefinition> {
        let names = match (self.class, self.alias, self.parent) {
            (Some(class), _, _) => ServiceTarget::Class(class),
            // A parent is followed like an alias: Symfony gives the child the parent's class unless
            // it names its own, and every Drupal logger channel is declared that way.
            (None, Some(target), _) | (None, None, Some(target)) => ServiceTarget::Alias(target),
            // Autowiring's shorthand: the id *is* the class, written as the whole FQN with the
            // definition saying only how to build it. Most of a modern Drupal module looks like this.
            (None, None, None) => match class_name(&self.id) {
                Some(class) => ServiceTarget::Class(class),
                None => ServiceTarget::Unknown,
            },
        };

        Some(ServiceDefinition { id: self.id, names })
    }
}

/// A line's indentation and content, or nothing when the line says nothing.
fn key_line(line: &str) -> Option<(usize, &str)> {
    let indent = line.len() - line.trim_start().len();
    let content = line.trim();

    // A list item is never an id, and a comment is never anything. Tabs are not legal YAML
    // indentation, so a line indented with one is not something to guess about.
    let uninteresting = content.is_empty()
        || content.starts_with('#')
        || content.starts_with('-')
        || line.starts_with('\t');
    (!uninteresting).then_some((indent, content))
}

/// Splits `key: value`, with an empty value for a key that opens a block.
///
/// Splitting on the first colon rather than the last: a quoted id may contain one, and a value
/// certainly can.
fn split_key(content: &str) -> Option<(&str, &str)> {
    let (key, value) = match content.strip_prefix(['\'', '"']) {
        // A quoted key: the colon that matters is the one after the closing quote.
        Some(rest) => {
            let quote = content.as_bytes()[0] as char;
            let end = rest.find(quote)?;
            (&rest[..end], rest[end + 1..].trim_start_matches(':'))
        }
        None => {
            let colon = content.find(':')?;
            (&content[..colon], &content[colon + 1..])
        }
    };

    let key = key.trim();
    (!key.is_empty()).then_some((key, value.trim()))
}

/// A `@service.id` reference, with Symfony's optional-service `?` allowed.
fn reference(value: &str) -> Option<String> {
    let value = unquote(value);
    let target = value.strip_prefix('@')?.trim_start_matches('?');
    (!target.is_empty()).then(|| target.to_owned())
}

/// A bare word, for an `alias:` written without the `@`.
fn plain(value: &str) -> Option<String> {
    let value = unquote(value);
    let usable =
        !value.is_empty() && !value.starts_with(['~', '{', '[', '%', '!']) && value != "null";
    usable.then(|| value.to_owned())
}

/// A class name a `class:` key or an id can be believed to be.
///
/// Rejects a `%parameter%` placeholder, which names a class only after the container is built, and
/// anything without a namespace separator -- a one-segment `class:` in these files is a YAML tag or a
/// parameter reference far more often than it is a global class.
fn class_name(value: &str) -> Option<String> {
    let value = unquote(value).trim_start_matches('\\');
    let usable = value.contains('\\') && !value.contains('%');
    usable.then(|| value.to_owned())
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    for quote in ['\'', '"'] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

#[cfg(test)]
mod parameter_tests {
    use phrost_model::BuiltinTy;

    use super::{ParameterDefinition, parameters_in};

    /// Every shape a real `core.services.yml` writes, in the order it writes them.
    ///
    /// The values are Drupal's own: two empty strings the kernel fills in at runtime, two lists, a
    /// number, a flag, and a mapping. The `services:` block below them is what says the reader
    /// stops at the end of the block rather than reading the whole file.
    #[test]
    fn every_shape_a_container_file_writes() {
        let read = parameters_in(
            r#"parameters:
  # The app.root parameter is set by the kernel.
  app.root: ''
  site.path: ''
  cache_default_bin_backends: []
  factory.keyvalue: {default: keyvalue.database}
  entity.memory_cache.slots: 1000
  security.enable_super_user: true
  session.storage.options:
    gc_probability: 1
    cookie_samesite: Lax
  http_client_config:
    timeout: 30.5

services:
  cache_factory:
    class: Drupal\Core\Cache\CacheFactory
"#,
        );

        assert_eq!(
            read,
            vec![
                ParameterDefinition {
                    name: "app.root".into(),
                    shape: BuiltinTy::String
                },
                ParameterDefinition {
                    name: "site.path".into(),
                    shape: BuiltinTy::String
                },
                ParameterDefinition {
                    name: "cache_default_bin_backends".into(),
                    shape: BuiltinTy::Array,
                },
                ParameterDefinition {
                    name: "factory.keyvalue".into(),
                    shape: BuiltinTy::Array
                },
                ParameterDefinition {
                    name: "entity.memory_cache.slots".into(),
                    shape: BuiltinTy::Int,
                },
                ParameterDefinition {
                    name: "security.enable_super_user".into(),
                    shape: BuiltinTy::Bool,
                },
                ParameterDefinition {
                    name: "session.storage.options".into(),
                    shape: BuiltinTy::Array,
                },
                ParameterDefinition {
                    name: "http_client_config".into(),
                    shape: BuiltinTy::Array
                },
            ]
        );
    }

    /// A file with no `parameters:` block at all, which most `*.services.yml` are.
    #[test]
    fn a_file_that_declares_none_says_nothing() {
        assert!(
            parameters_in(
                r#"services:
  my.thing:
    class: Drupal\my\Thing
    arguments: ['@other']
"#
            )
            .is_empty()
        );
    }

    /// A name under `parameters:` with nothing beside it and nothing under it declares no value,
    /// and a shape invented for it would be a claim the file did not make.
    #[test]
    fn a_name_with_no_value_is_not_a_parameter() {
        assert!(
            parameters_in(
                r#"parameters:
  nothing.here:
"#
            )
            .is_empty()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the definitions whose class is knowable, which is what most of these tests are about.
    /// See `a_service_with_no_knowable_class_is_still_registered` for the other half.
    fn classes(text: &str) -> Vec<(String, String)> {
        let mut found = resolve_aliases(services_in(text))
            .into_iter()
            .filter_map(|(id, class)| Some((id, class?)))
            .collect::<Vec<_>>();
        found.sort();
        found
    }

    #[test]
    fn a_class_key_names_the_service() {
        assert_eq!(
            classes(
                r#"
services:

  app_core.application_action_manager:
    class: Drupal\app_core\Application\Action\OrderActionManager
    arguments:
      - !tagged_iterator application_action
      - '@Drupal\app_core\Application\Action\Actions\FreezeAction'
"#
            ),
            [(
                "app_core.application_action_manager".to_owned(),
                "Drupal\\app_core\\Application\\Action\\OrderActionManager".to_owned()
            )]
        );
    }

    /// Autowiring's shorthand, which most of a modern Drupal module is written in.
    #[test]
    fn an_id_that_is_a_class_needs_no_class_key() {
        assert_eq!(
            classes(
                r#"
services:
  Drupal\app_core\Application\Action\Actions\FreezeAction:
    arguments: ['@application.manager', '@current_user']
  Drupal\app_core\Thing: ~
"#
            ),
            [
                (
                    "Drupal\\app_core\\Application\\Action\\Actions\\FreezeAction".to_owned(),
                    "Drupal\\app_core\\Application\\Action\\Actions\\FreezeAction".to_owned()
                ),
                (
                    "Drupal\\app_core\\Thing".to_owned(),
                    "Drupal\\app_core\\Thing".to_owned()
                ),
            ]
        );
    }

    /// The trap that made this a block-scoped scanner instead of a search for `class:`.
    ///
    /// Drupal's own `core.services.yml` lists a dozen renamed classes under `parameters:`, each as a
    /// mapping with a `class:` key. A scanner looking for `class:` anywhere would register every one
    /// of them as a service, and `\Drupal::service()` would then answer for ids that do not exist.
    #[test]
    fn a_class_key_under_parameters_is_not_a_service() {
        assert_eq!(
            classes(
                r#"
parameters:
  core.moved_classes:
    'Drupal\Core\StringTranslation\TranslationWrapper':
      class: 'Drupal\Core\StringTranslation\TranslatableMarkup'
    'Doctrine\Common\Annotations\AnnotationException':
      class: 'Drupal\Component\Annotation\Doctrine\AnnotationException'
      deprecation_version: drupal:11.3.0
services:
  real.one:
    class: Drupal\Core\Real
"#
            ),
            [("real.one".to_owned(), "Drupal\\Core\\Real".to_owned())]
        );
    }

    #[test]
    fn parameters_after_services_end_the_block() {
        assert_eq!(
            classes(
                r#"
services:
  real.one:
    class: Drupal\Core\Real
parameters:
  later.thing:
    class: Drupal\Core\NotAService
"#
            ),
            [("real.one".to_owned(), "Drupal\\Core\\Real".to_owned())]
        );
    }

    #[test]
    fn an_alias_takes_the_class_of_what_it_points_at() {
        assert_eq!(
            classes(
                r#"
services:
  real.one:
    class: Drupal\Core\Real
  alias.long:
    alias: real.one
  alias.reference:
    alias: '@real.one'
  alias.shorthand: '@real.one'
  alias.optional:
    alias: '@?real.one'
  alias.of.alias:
    alias: alias.long
"#
            ),
            [
                ("alias.long".to_owned(), "Drupal\\Core\\Real".to_owned()),
                ("alias.of.alias".to_owned(), "Drupal\\Core\\Real".to_owned()),
                ("alias.optional".to_owned(), "Drupal\\Core\\Real".to_owned()),
                (
                    "alias.reference".to_owned(),
                    "Drupal\\Core\\Real".to_owned()
                ),
                (
                    "alias.shorthand".to_owned(),
                    "Drupal\\Core\\Real".to_owned()
                ),
                ("real.one".to_owned(), "Drupal\\Core\\Real".to_owned()),
            ]
        );
    }

    #[test]
    fn an_alias_naming_nothing_and_a_cycle_both_answer_nothing() {
        assert_eq!(
            classes(
                r#"
services:
  dangling:
    alias: never.defined
  loop.a:
    alias: loop.b
  loop.b:
    alias: loop.a
"#
            ),
            []
        );
    }

    /// Everything whose class this cannot know, answering nothing rather than a guess.
    #[test]
    fn a_class_that_is_not_knowable_here_is_not_recorded() {
        assert_eq!(
            classes(
                r#"
services:
  _defaults:
    autowire: true
  _instanceof:
    Drupal\Core\Thing:
      tags: [thing]
  from.parameter:
    class: '%my.class%'
  no.namespace:
    class: ArrayObject
  built.by.parent:
    parent: logger.channel_base
    arguments: ['mine']
  built.by.factory:
    factory: ['@thing.factory', 'create']
  says.nothing: ~
"#
            ),
            []
        );
    }

    /// A leading separator is how a hand-written definition often spells a class, and the index
    /// stores no name with one.
    #[test]
    fn a_leading_separator_is_stripped() {
        assert_eq!(
            classes(
                r#"
services:
  one:
    class: \Drupal\Core\Real
"#
            ),
            [("one".to_owned(), "Drupal\\Core\\Real".to_owned())]
        );
    }
}

#[cfg(test)]
mod registration {
    use super::*;

    /// A registered id with no knowable class comes back with the class missing, not missing itself.
    ///
    /// The distinction a report rests on. Drupal declares every logger channel as
    /// `parent: logger.channel_base` with no class of its own, and dropping those made
    /// `\Drupal::service('logger.channel.mine')` read as an id nobody had registered -- which is
    /// exactly what the report said, on correct code, until this.
    #[test]
    fn a_service_with_no_knowable_class_is_still_registered() {
        let found = resolve_aliases(services_in(
            "services:\n  logger.channel_base:\n    class: Drupal\\Core\\Logger\\LoggerChannel\n  logger.channel.mine:\n    parent: logger.channel_base\n  built.by_factory:\n    factory: ['@some.builder', 'make']\n",
        ));

        // The parent is followed like an alias, because Symfony gives the child the parent's class.
        assert!(found.contains(&(
            "logger.channel.mine".to_owned(),
            Some("Drupal\\Core\\Logger\\LoggerChannel".to_owned())
        )));
        // A factory says nothing about the class, and the id is registered all the same.
        assert!(found.contains(&("built.by_factory".to_owned(), None)));
    }
}
