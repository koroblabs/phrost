//! Turning a declaration's own names into the sentences a docblock wants.
//!
//! `getPaidTotal` becomes "Gets the paid total." and `$itemNumber` becomes "The item
//! number." Not prose anybody would call good, and far better than a line saying "describe this":
//! most of it is right, the rest is a word to change, and either way somebody reads it rather than
//! deleting it unread.
//!
//! The phrasing is Drupal core's own, read out of it rather than invented -- `EntityInterface` writes
//! "Gets the entity type definition.", "Sets the original ID.", "Determines whether the entity has a
//! field with the given name." Third person, one line, full stop.
//!
//! # Why not a model
//!
//! Weights are hundreds of megabytes against a whole server that is two hundred and forty; a
//! generation is hundreds of milliseconds against a twelve-millisecond budget; and the answer differs
//! between runs, which every invariant in this project is built on not doing. A model belongs outside
//! the server, where a person asks it a question, not inside a code action that has to give the same
//! answer twice.

/// The line left to overwrite, where nothing can be derived and nothing may be invented.
pub(super) const PLACEHOLDER: &str = "Describe what this does.";

/// The summary line for a method, from its name.
pub(super) fn summary_of(name: &str) -> String {
    let words = words_in(name);
    let Some((verb, rest)) = words.split_first() else {
        return PLACEHOLDER.to_owned();
    };
    let subject = rest.join(" ");

    // The four prefixes Drupal core phrases specially, because a getter's summary reads as a noun
    // phrase and a predicate's reads as a question.
    let sentence = match verb.as_str() {
        "get" if !subject.is_empty() => format!("Gets the {subject}"),
        "set" if !subject.is_empty() => format!("Sets the {subject}"),
        // Two sentences rather than one, because "whether the shared" is not English and "whether it
        // is shared" is. Core's own are longer than anything derivable -- "Determines whether the
        // entity has a field with the given name" -- so this gets the shape right and leaves the
        // detail to be written.
        "is" if !subject.is_empty() => format!("Determines whether it is {subject}"),
        "has" if !subject.is_empty() => format!("Determines whether it has a {subject}"),
        _ => match subject.is_empty() {
            true => third_person(verb),
            false => format!("{} {subject}", third_person(verb)),
        },
    };

    finish(&sentence)
}

/// The description under a `@param`, from the parameter's name.
pub(super) fn param_prose(name: &str) -> String {
    let words = words_in(name);
    match words.is_empty() {
        true => "Describe it.".to_owned(),
        false => finish(&format!("The {}", words.join(" "))),
    }
}

/// The description under a `@return`, from what is returned.
///
/// The type's own last segment, which is the only thing here that knows anything about the value.
/// `bool` gets the sentence Drupal writes for one, because "The bool." says nothing at all.
pub(super) fn return_prose(rendered: &str) -> String {
    let bare = rendered.trim_start_matches(['?', '\\']);
    // A shape has already said what each key holds, and none of that text is a name to derive a
    // sentence from: `rsplit('\\')` over `array{file: ?\App\File}` answers `File}`. What the tag
    // still cannot say is what the array *is*, which is the line left to overwrite.
    if bare.starts_with("array{") {
        return finish("The array");
    }
    let short = bare.rsplit('\\').next().unwrap_or(bare);
    match short {
        "bool" => "TRUE if it does, FALSE otherwise.".to_owned(),
        "void" | "never" | "" => "Describe it.".to_owned(),
        other => finish(&format!("The {}", words_in(other).join(" "))),
    }
}

/// The description under a `@throws`, from the exception's name.
///
/// The standard library's exceptions each mean one thing and get that thing; anything else gets the
/// sentence core writes most often under a `@throws`, which says nothing about *when* -- that is the
/// word to change, and it is one word.
pub(super) fn throws_prose(rendered: &str) -> String {
    let bare = rendered.trim_start_matches('\\');
    let short = bare.rsplit('\\').next().unwrap_or(bare);
    match short {
        "InvalidArgumentException" => "Thrown when an argument is invalid.",
        "OutOfRangeException" | "OutOfBoundsException" => "Thrown when an index is out of range.",
        "LengthException" => "Thrown when a length is invalid.",
        "DomainException" => "Thrown when a value is outside the domain.",
        "UnexpectedValueException" => "Thrown when a value is not what was expected.",
        "BadMethodCallException" | "BadFunctionCallException" => {
            "Thrown when the call is not valid."
        }
        "LogicException" => "Thrown when the call makes no sense here.",
        "JsonException" => "Thrown when the JSON cannot be handled.",
        "TypeError" | "ValueError" => "Thrown when an argument is not acceptable.",
        _ => "Thrown when the operation fails.",
    }
    .to_owned()
}

/// A capital at the front and a full stop at the end, which is the whole of the standard's shape.
fn finish(sentence: &str) -> String {
    let mut chars = sentence.chars();
    let Some(first) = chars.next() else {
        return "Describe it.".to_owned();
    };
    format!("{}{}.", first.to_uppercase(), chars.as_str())
}

/// A name's words, lower-cased: `getPaidTotal` -> `["get", "paid", "total"]`.
///
/// Both spellings a PHP name uses, because a project writes methods in camel case and functions in
/// snake case, and a docblock generator meets each of them.
fn words_in(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();

    for ch in name.chars() {
        if ch == '_' {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        if ch.is_uppercase() && !word.is_empty() {
            // A capital starts a word -- except inside a run of them, which is an acronym and reads
            // worse broken apart. `getHTTPStatus` is three words and not five.
            if !word.chars().last().is_some_and(char::is_uppercase) {
                words.push(std::mem::take(&mut word));
            }
        }
        word.push(ch);
    }
    if !word.is_empty() {
        words.push(word);
    }

    // A run of capitals followed by a lowercase is two words: the acronym, and the word whose own
    // capital was swallowed by the run. `HTTPStatus` is `HTTP` and `Status`.
    let words = words
        .into_iter()
        .flat_map(|word| {
            let letters = word.chars().collect::<Vec<_>>();
            let split = letters
                .iter()
                .position(|ch| ch.is_lowercase())
                .filter(|at| *at >= 2);
            match split {
                Some(at) => vec![
                    letters[..at - 1].iter().collect::<String>(),
                    letters[at - 1..].iter().collect::<String>(),
                ],
                None => vec![word],
            }
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();

    words
        .into_iter()
        .map(|word| match word.chars().all(|ch| ch.is_uppercase()) {
            // An acronym keeps its case: core writes "Gets the ID", not "Gets the id".
            true => word,
            false => word.to_lowercase(),
        })
        .collect()
}

/// The third-person form of a verb, as a summary is written in.
///
/// English's three regular endings and nothing else. An irregular verb comes out slightly wrong and
/// is one word to fix, where a list of exceptions would be a list somebody maintains forever.
fn third_person(verb: &str) -> String {
    let lower = verb.to_lowercase();
    if lower.ends_with('s')
        || lower.ends_with('x')
        || lower.ends_with('z')
        || lower.ends_with("ch")
        || lower.ends_with("sh")
    {
        return format!("{lower}es");
    }
    if lower.len() > 1
        && lower.ends_with('y')
        && !lower
            .chars()
            .nth(lower.len() - 2)
            .is_some_and(|ch| "aeiou".contains(ch))
    {
        return format!("{}ies", &lower[..lower.len() - 1]);
    }
    format!("{lower}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_shapes_drupal_core_phrases_specially() {
        assert_eq!(summary_of("getPaidTotal"), "Gets the paid total.");
        // `Id` and not `ID`: core writes the acronym, and knowing that `Id` is one would take a list
        // of them, which is a list somebody maintains forever for one letter's worth of polish.
        assert_eq!(summary_of("setOriginalId"), "Sets the original id.");
        assert_eq!(summary_of("hasField"), "Determines whether it has a field.");
        assert_eq!(summary_of("isShared"), "Determines whether it is shared.");
    }

    #[test]
    fn anything_else_gets_its_verb_put_in_the_third_person() {
        assert_eq!(summary_of("loadByItemNumber"), "Loads by item number.");
        assert_eq!(summary_of("process"), "Processes.");
        assert_eq!(summary_of("applyTo"), "Applies to.");
        assert_eq!(summary_of("fetchAll"), "Fetches all.");
    }

    #[test]
    fn an_acronym_keeps_its_case_and_stays_one_word() {
        assert_eq!(summary_of("getHTTPStatus"), "Gets the HTTP status.");
        assert_eq!(param_prose("itemNumber"), "The item number.");
        assert_eq!(param_prose("id"), "The id.");
    }

    #[test]
    fn a_return_says_what_the_type_is() {
        assert_eq!(return_prose("?\\Drupal\\node\\Entity\\Node"), "The node.");
        assert_eq!(return_prose("bool"), "TRUE if it does, FALSE otherwise.");
        assert_eq!(return_prose("int"), "The int.");
    }
}
