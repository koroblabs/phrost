//! Interned PHP identifiers.
//!
//! A symbol name must live once behind one allocation, not in a dozen parallel `String` copies
//! spread across lookup maps. Cloning a [`Name`] shares the allocation; the interner itself is
//! only a reuse table.
//!
//! # PHP case rules
//!
//! PHP is inconsistent here, and the inconsistency is load-bearing, so it is modelled instead of
//! smoothed over. Case-**insensitive**: class / interface / trait / enum names, function names,
//! method names, namespace segments. Case-**sensitive**: variables, property names, global
//! constants, class constants.
//!
//! So a name has two interned forms: the [`Name`] as written (what hover and completion display,
//! what goto reports) and a [`NameKey`] used for case-insensitive lookup. Both are needed —
//! folding at display time loses the author's casing, and comparing display text loses matches.
//!
//! The folding is **ASCII-only**, matching PHP's engine: `zend_str_tolower` maps `A-Z` and leaves
//! every other byte alone, so `Ä` and `ä` are different class names in PHP and here too.

use std::{cmp::Ordering, fmt, hash::Hash, sync::Arc};

use rustc_hash::FxHashSet;

/// One PHP identifier, interned, spelled as the author wrote it.
///
/// Display casing is preserved because it is user-visible in hover, completion, and symbol
/// listings. Use [`NameKey`] when the question is "do these refer to the same thing".
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Name(Arc<str>);

impl Name {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// How many slot names are kept ready, which is what bounds every reader that asks for one.
///
/// A literal past a few dozen entries is a list nobody asks slot questions about, and the readers
/// that do ask are capped well under this already.
const SLOTS: usize = 64;

/// The name PHP gives slot `index` of a positional array literal.
///
/// `[a, b, c]` *is* `[0 => a, 1 => b, 2 => c]`, and a reader that wants to say so needs the `Name`
/// the index knows that key by -- on a path that has no [`Interner`], because interning is a write
/// and a request is a read. So the digits are held here instead, made once for the process.
///
/// Sound because [`Name`] compares by the text rather than by the allocation: a `"0"` from here and
/// a `"0"` a file's lowering interned are the same name to every reader, which is the same property
/// that lets two files' interners agree at all.
pub fn slot_name(index: usize) -> Option<Name> {
    static SLOT_NAMES: std::sync::OnceLock<Vec<Name>> = std::sync::OnceLock::new();
    SLOT_NAMES
        .get_or_init(|| {
            (0..SLOTS)
                .map(|slot| Name(Arc::from(slot.to_string().as_str())))
                .collect()
        })
        .get(index)
        .cloned()
}

/// Whether PHP keys an array by this text as an integer rather than as a string.
///
/// PHP's own canonicalisation, which is narrower than "parses as a number": a decimal integer in
/// range, written the way PHP would print it. `'1'` and `'-1'` become ints; `'01'`, `'-0'`, `'+1'`,
/// `' 1'` and one past `PHP_INT_MAX` stay strings. Asked of PHP itself rather than read off a
/// manual, and the answer decides two things that must not disagree: how a shape key is rendered,
/// and what iterating a literal names each slot by.
pub fn is_an_integer_key(key: &str) -> bool {
    let digits = key.strip_prefix('-').unwrap_or(key);
    let canonical = match digits.as_bytes() {
        // `-0` is not how PHP prints zero, so it stays a string.
        [b'0'] => digits.len() == key.len(),
        [b'1'..=b'9', rest @ ..] => rest.iter().all(u8::is_ascii_digit),
        _ => false,
    };
    canonical && key.parse::<i64>().is_ok()
}

/// A [`Name`] for text that is not a PHP identifier and never enters the index.
///
/// One caller: the call hierarchy names a file as the caller of a call written at a file's top
/// level, and a path is not something any lowering interns. Sound for the same reason [`slot_name`]
/// is -- a `Name` compares by its text rather than by its allocation -- and deliberately awkward to
/// reach for, because a name that *is* a PHP identifier belongs in the table where every other
/// reader will find the same allocation.
pub fn display_name(text: &str) -> Name {
    Name(Arc::from(text))
}

/// Case-folded lookup key for the PHP entities whose names are case-insensitive.
///
/// Deliberately a distinct type from [`Name`]: the compiler then refuses to let a display name be
/// used as a lookup key, which is the mistake that produces "goto works for `Foo` but not `foo`".
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NameKey(Arc<str>);

impl NameKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Reuse table for [`Name`] and [`NameKey`] allocations.
///
/// Not `Sync`: interning happens while building one file's data, and the resulting names are what
/// travel between threads. Sharing the table itself would put a lock on a very hot path.
#[derive(Debug, Default)]
pub struct Interner {
    names: FxHashSet<Arc<str>>,
    keys: FxHashSet<Arc<str>>,
    /// Scratch buffer for folding, so `key()` does not allocate a `String` per call.
    fold_buffer: String,
}

impl lspkit_memsize::MemSize for Interner {
    /// Every interned string, once.
    ///
    /// This is the one place the bytes of a name are counted: everything else in the project holds
    /// an `Arc<str>` pointing here, and a report that counted those would be several times the real
    /// figure.
    fn heap_size(&self) -> usize {
        let strings = |set: &FxHashSet<Arc<str>>| {
            set.iter().map(|name| name.len()).sum::<usize>()
                + lspkit_memsize::table::<Arc<str>, ()>(set.len())
        };

        strings(&self.names) + strings(&self.keys) + self.fold_buffer.capacity()
    }
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns a name exactly as spelled.
    pub fn name(&mut self, text: &str) -> Name {
        Name(Self::intern_into(&mut self.names, text))
    }

    /// A name already interned, without interning one that is not.
    ///
    /// For a reader that has a string and needs the `Name` the index knows it by -- a request
    /// building a subject to compare a stored path against. Read-only on purpose: a name nothing
    /// has interned names nothing in the project, so `None` is the answer to the question that was
    /// really being asked.
    pub fn lookup_name(&self, text: &str) -> Option<Name> {
        self.names.get(text).cloned().map(Name)
    }

    /// Interns the case-folded lookup key for a name.
    ///
    /// Fast path: a name that is already lowercase is by far the common case in real PHP
    /// (`$foo->getName()`, `use App\Repo`), so folding avoids touching the buffer at all then.
    pub fn key(&mut self, text: &str) -> NameKey {
        if !text.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return NameKey(Self::intern_into(&mut self.keys, text));
        }

        self.fold_buffer.clear();
        self.fold_buffer.extend(text.chars().map(|ch| {
            if ch.is_ascii_uppercase() {
                ch.to_ascii_lowercase()
            } else {
                ch
            }
        }));

        // Take the buffer out so the borrow checker sees the set and the buffer as disjoint.
        let folded = std::mem::take(&mut self.fold_buffer);
        let interned = Self::intern_into(&mut self.keys, &folded);
        self.fold_buffer = folded;

        NameKey(interned)
    }

    /// Interns the key for an already-interned name.
    pub fn key_of(&mut self, name: &Name) -> NameKey {
        self.key(name.as_str())
    }

    /// Looks up an existing key without interning a new one.
    ///
    /// For read paths, which only have `&self`. A miss means nothing has ever been interned under
    /// that name — which for a lookup is the same answer as "not declared", so there is no reason
    /// to take a write lock to find out.
    pub fn lookup_key(&self, text: &str) -> Option<NameKey> {
        if !text.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return self.keys.get(text).map(|key| NameKey(Arc::clone(key)));
        }

        // Folded on the stack for any name that fits, which is every class and method name a
        // project writes. This runs for every class a request looks up -- and nearly every class
        // name has an uppercase letter -- so the `String` it used to build was an allocation and a
        // free per lookup, visible as such in a profile of a diagnostics publish. ASCII folding
        // keeps UTF-8 valid, so the buffer reads back as text without a copy.
        const STACK: usize = 256;
        if text.len() <= STACK {
            let mut buffer = [0_u8; STACK];
            let bytes = &mut buffer[..text.len()];
            bytes.copy_from_slice(text.as_bytes());
            bytes.make_ascii_lowercase();
            let folded = std::str::from_utf8(bytes).ok()?;
            return self.keys.get(folded).map(|key| NameKey(Arc::clone(key)));
        }

        let folded = text.to_ascii_lowercase();
        self.keys
            .get(folded.as_str())
            .map(|key| NameKey(Arc::clone(key)))
    }

    /// Number of distinct display names and lookup keys held.
    ///
    /// Reported by memory tooling; the gap between the two is how much casing variance a project
    /// actually has.
    pub fn stats(&self) -> InternerStats {
        InternerStats {
            names: self.names.len(),
            keys: self.keys.len(),
        }
    }

    fn intern_into(set: &mut FxHashSet<Arc<str>>, text: &str) -> Arc<str> {
        if let Some(existing) = set.get(text) {
            return Arc::clone(existing);
        }

        let interned: Arc<str> = Arc::from(text);
        set.insert(Arc::clone(&interned));
        interned
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternerStats {
    pub names: usize,
    pub keys: usize,
}

// Ordering is by text, not by interning order: snapshot renderers sort by name and must not
// depend on which file happened to be indexed first.
macro_rules! impl_text_traits {
    ($ty:ident) => {
        impl PartialOrd for $ty {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $ty {
            fn cmp(&self, other: &Self) -> Ordering {
                self.as_str().cmp(other.as_str())
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:?})", stringify!($ty), self.as_str())
            }
        }

        impl std::borrow::Borrow<str> for $ty {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl AsRef<str> for $ty {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

impl_text_traits!(Name);
impl_text_traits!(NameKey);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_names_share_one_allocation() {
        let mut interner = Interner::new();
        let first = interner.name("getName");
        let second = interner.name("getName");

        assert_eq!(first, second);
        assert!(std::ptr::eq(first.as_str(), second.as_str()));
        assert_eq!(interner.stats().names, 1);
    }

    #[test]
    fn casing_differs_in_name_but_not_in_key() {
        let mut interner = Interner::new();
        let written = interner.name("GetName");
        let other = interner.name("getname");

        assert_ne!(written, other);
        assert_eq!(interner.key_of(&written), interner.key_of(&other));
        assert_eq!(interner.stats().names, 2);
        assert_eq!(interner.stats().keys, 1);
    }

    #[test]
    fn folding_is_ascii_only_like_the_php_engine() {
        let mut interner = Interner::new();

        // PHP's zend_str_tolower leaves non-ASCII bytes alone, so these stay distinct classes.
        assert_ne!(interner.key("Ärger"), interner.key("ärger"));
        assert_eq!(interner.key("ÄRger").as_str(), "Ärger");
    }

    #[test]
    fn names_sort_by_text_not_by_interning_order() {
        let mut interner = Interner::new();
        let mut names = [interner.name("zeta"), interner.name("alpha")];
        names.sort();

        assert_eq!(
            names.iter().map(Name::as_str).collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
    }

    /// The table PHP 8.5 itself printed for `gettype(array_key_first([$key => 1]))`.
    #[test]
    fn an_integer_key_is_the_one_php_would_print() {
        let read: Vec<_> = [
            "0",
            "1",
            "-1",
            "9223372036854775807",
            "-9223372036854775808",
            "01",
            "-0",
            "+1",
            " 1",
            "1 ",
            "9223372036854775808",
            "",
            "one",
        ]
        .into_iter()
        .map(|key| (key, is_an_integer_key(key)))
        .collect();

        assert_eq!(
            read,
            [
                ("0", true),
                ("1", true),
                ("-1", true),
                ("9223372036854775807", true),
                ("-9223372036854775808", true),
                ("01", false),
                ("-0", false),
                ("+1", false),
                (" 1", false),
                ("1 ", false),
                ("9223372036854775808", false),
                ("", false),
                ("one", false),
            ]
        );
    }
}
