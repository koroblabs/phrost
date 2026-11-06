//! What a migration's calls on a table's `Blueprint` do to its columns, and which table a model's
//! rows live in.
//!
//! The column half is `Illuminate\Database\Schema\Blueprint` restated: every method that reaches
//! `addColumn()` adds one, under the name its first argument gives or the one the method fixes
//! (`id()`, `timestamps()`), and holds what the database hands back for that type before anything
//! casts it. That is the *raw* value, read off the drivers rather than off the column's name: a
//! `decimal` comes back as a string so as not to lose digits, a `json` as the text it stores, a date
//! as the text it was written in -- Laravel casts `created_at` and its siblings itself, and anything
//! else only where `$casts` says. A `boolean` is the one that differs by driver, `1` from MySQL and
//! `true` from PostgreSQL, so it holds whichever.
//!
//! The table half is `Model::getTable()`: `Str::snake(Str::pluralStudly(class_basename($this)))`
//! when the class names none, with Doctrine's English rules for the plural -- checked against
//! Laravel itself on every model name in the corpora this was measured on.

use phrost_model::{
    BuiltinTy,
    schema::{BuilderCall, ColumnChange},
};

/// What each column-adding method holds, by Blueprint's name for it. A method not here adds no
/// column: `index('email')`, `foreign('user_id')` and `comment('...')` name one without adding it.
const COLUMNS: &[(&str, BuiltinTy)] = {
    use BuiltinTy::{Int, Mixed, String};
    &[
        ("bigIncrements", Int),
        ("bigInteger", Int),
        ("foreignId", Int),
        ("id", Int),
        ("increments", Int),
        ("integer", Int),
        ("integerIncrements", Int),
        ("mediumIncrements", Int),
        ("mediumInteger", Int),
        ("smallIncrements", Int),
        ("smallInteger", Int),
        ("tinyIncrements", Int),
        ("tinyInteger", Int),
        ("unsignedBigInteger", Int),
        ("unsignedInteger", Int),
        ("unsignedMediumInteger", Int),
        ("unsignedSmallInteger", Int),
        ("unsignedTinyInteger", Int),
        ("year", Int),
        ("double", BuiltinTy::Float),
        ("float", BuiltinTy::Float),
        ("decimal", String),
        ("unsignedDecimal", String),
        ("boolean", Mixed),
        ("char", String),
        ("string", String),
        ("text", String),
        ("tinyText", String),
        ("mediumText", String),
        ("longText", String),
        ("json", String),
        ("jsonb", String),
        ("binary", String),
        ("uuid", String),
        ("ulid", String),
        ("foreignUuid", String),
        ("foreignUlid", String),
        ("ipAddress", String),
        ("macAddress", String),
        ("enum", String),
        ("set", String),
        ("date", String),
        ("dateTime", String),
        ("dateTimeTz", String),
        ("time", String),
        ("timeTz", String),
        ("timestamp", String),
        ("timestampTz", String),
        ("geometry", Mixed),
        ("geography", Mixed),
        ("computed", Mixed),
        ("vector", Mixed),
        ("rawColumn", Mixed),
    ]
};

/// The name a column method gives a column when its first argument names none.
const NAMED_BY_DEFAULT: &[(&str, &str)] = &[
    ("id", "id"),
    ("uuid", "uuid"),
    ("ulid", "ulid"),
    ("ipAddress", "ip_address"),
    ("macAddress", "mac_address"),
];

/// Everything one call on a `Blueprint` does to its table.
pub(super) fn columns(call: &BuilderCall) -> Vec<ColumnChange> {
    let first = || call.arguments.first().and_then(|written| written.first());
    let nullable = call.then.iter().rev().find_map(|(method, argument)| {
        method
            .eq_ignore_ascii_case("nullable")
            .then(|| argument.unwrap_or(true))
    });
    // A key the database enforces: `->constrained()`, or `->references('id')` on the column itself.
    let constrained = call.then.iter().any(|(method, _)| {
        method.eq_ignore_ascii_case("constrained") || method.eq_ignore_ascii_case("references")
    });
    let defined = |name: String, holds: BuiltinTy, nullable_anyway: bool| ColumnChange::Defined {
        name,
        holds,
        nullable: nullable.unwrap_or(nullable_anyway),
        constrained,
    };
    let method = call.method.as_str();
    let is = |name: &str| method.eq_ignore_ascii_case(name);

    // `$table->foreign('account_id')->references('id')->on('accounts')`, or several columns at once.
    if is("foreign") {
        return match constrained {
            true => first_argument_names(call)
                .into_iter()
                .map(ColumnChange::Constrained)
                .collect(),
            false => Vec::new(),
        };
    }

    if let Some(&(_, holds)) = COLUMNS.iter().find(|(name, _)| is(name)) {
        let named = first().cloned().or_else(|| {
            NAMED_BY_DEFAULT
                .iter()
                .find(|(name, _)| is(name))
                .map(|(_, column)| (*column).to_owned())
        });
        return named
            .map(|name| defined(name, holds, false))
            .into_iter()
            .collect();
    }
    // `foreignIdFor(User::class)` is `user_id`: the model's foreign key, its snake-cased name.
    if is("foreignIdFor") {
        let named = call
            .arguments
            .get(1)
            .and_then(|written| written.first())
            .cloned()
            .or_else(|| {
                first().map(|class| format!("{}_id", snake(phrost_model::short_name(class))))
            });
        return named
            .map(|name| defined(name, BuiltinTy::Int, false))
            .into_iter()
            .collect();
    }
    if is("timestamps") || is("timestampsTz") || is("nullableTimestamps") {
        return ["created_at", "updated_at"]
            .into_iter()
            .map(|name| defined(name.to_owned(), BuiltinTy::String, true))
            .collect();
    }
    if is("softDeletes") || is("softDeletesTz") || is("softDeletesDatetime") {
        let name = first().map_or("deleted_at", String::as_str).to_owned();
        return vec![defined(name, BuiltinTy::String, true)];
    }
    if is("rememberToken") {
        return vec![defined(
            "remember_token".to_owned(),
            BuiltinTy::String,
            true,
        )];
    }
    // `morphs('taggable')` is a `taggable_type` and a `taggable_id`.
    let morphs = [
        ("morphs", BuiltinTy::Int, false),
        ("nullableMorphs", BuiltinTy::Int, true),
        ("numericMorphs", BuiltinTy::Int, false),
        ("nullableNumericMorphs", BuiltinTy::Int, true),
        ("uuidMorphs", BuiltinTy::String, false),
        ("nullableUuidMorphs", BuiltinTy::String, true),
        ("ulidMorphs", BuiltinTy::String, false),
        ("nullableUlidMorphs", BuiltinTy::String, true),
    ];
    if let Some(&(_, id, nullable)) = morphs.iter().find(|(name, ..)| is(name)) {
        let Some(base) = first() else {
            return Vec::new();
        };
        return vec![
            defined(format!("{base}_type"), BuiltinTy::String, nullable),
            defined(format!("{base}_id"), id, nullable),
        ];
    }

    let dropped = |names: &[&str]| {
        names
            .iter()
            .map(|name| ColumnChange::Dropped((*name).to_owned()))
            .collect()
    };
    if is("dropColumn") || is("dropColumns") || is("dropConstrainedForeignId") {
        return call
            .arguments
            .iter()
            .flatten()
            .map(|name| ColumnChange::Dropped(name.clone()))
            .collect();
    }
    if is("dropTimestamps") || is("dropTimestampsTz") {
        return dropped(&["created_at", "updated_at"]);
    }
    if is("dropSoftDeletes") || is("dropSoftDeletesTz") {
        return vec![ColumnChange::Dropped(
            first().map_or("deleted_at", String::as_str).to_owned(),
        )];
    }
    if is("dropRememberToken") {
        return dropped(&["remember_token"]);
    }
    if is("dropMorphs") {
        return first()
            .map(|base| {
                vec![
                    ColumnChange::Dropped(format!("{base}_type")),
                    ColumnChange::Dropped(format!("{base}_id")),
                ]
            })
            .unwrap_or_default();
    }
    if is("renameColumn") {
        let to = call.arguments.get(1).and_then(|written| written.first());
        if let (Some(from), Some(to)) = (first(), to) {
            return vec![ColumnChange::Renamed {
                from: from.clone(),
                to: to.clone(),
            }];
        }
    }
    Vec::new()
}

/// The table a model names when it declares none, from its short name: `PageRevision` is
/// `page_revisions`, `Bookshelf` is `bookshelves`, `Person` is `people`.
pub(super) fn default_table(class: &str) -> String {
    snake(&plural_studly(class))
}

/// `Str::pluralStudly`: the last word of a studly-cased name made plural.
fn plural_studly(value: &str) -> String {
    // The last word begins at the last capital that has something before it.
    let split = value
        .char_indices()
        .skip(1)
        .filter(|(_, ch)| ch.is_ascii_uppercase())
        .map(|(at, _)| at)
        .last()
        .unwrap_or(0);
    format!("{}{}", &value[..split], plural(&value[split..]))
}

/// `Str::snake`: an underscore before every capital that has something before it, then lower case.
fn snake(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 4);
    for (at, ch) in value.char_indices() {
        if at > 0 && ch.is_ascii_uppercase() {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

/// Laravel's own uncountables, asked before the inflector is.
const UNCOUNTABLE: &[&str] = &["recommended", "related"];

/// `Pluralizer::plural` for a count of two: Doctrine's English inflector, then the original's case.
fn plural(word: &str) -> String {
    if word.is_empty()
        || !word.ends_with(|ch: char| ch.is_ascii_alphanumeric() || !ch.is_ascii())
        || UNCOUNTABLE
            .iter()
            .any(|known| word.eq_ignore_ascii_case(known))
    {
        return word.to_owned();
    }
    let plural = inflect(word);
    // `matchCase`: the first of lower, upper and a leading capital that the original is already.
    if word.chars().all(|ch| !ch.is_uppercase()) {
        plural.to_lowercase()
    } else if word.chars().all(|ch| !ch.is_lowercase()) {
        plural.to_uppercase()
    } else {
        plural
    }
}

/// Words Doctrine leaves as they are, whole and case-insensitively.
const UNINFLECTED: &[&str] = &[
    "advice",
    "aircraft",
    "amoyese",
    "art",
    "audio",
    "baggage",
    "bison",
    "borghese",
    "bream",
    "breeches",
    "britches",
    "buffalo",
    "butter",
    "cantus",
    "carp",
    "cattle",
    "chassis",
    "clippers",
    "clothing",
    "coal",
    "cod",
    "coitus",
    "compensation",
    "congoese",
    "contretemps",
    "coreopsis",
    "corps",
    "cotton",
    "data",
    "debris",
    "deer",
    "diabetes",
    "djinn",
    "education",
    "eland",
    "elk",
    "emoji",
    "equipment",
    "evidence",
    "faroese",
    "feedback",
    "fish",
    "flounder",
    "flour",
    "foochowese",
    "food",
    "furniture",
    "gallows",
    "genevese",
    "genoese",
    "gilbertese",
    "gold",
    "headquarters",
    "herpes",
    "hijinks",
    "homework",
    "hottentotese",
    "impatience",
    "information",
    "innings",
    "jackanapes",
    "jeans",
    "jedi",
    "kin",
    "kiplingese",
    "knowledge",
    "kongoese",
    "leather",
    "love",
    "lucchese",
    "luggage",
    "mackerel",
    "maltese",
    "management",
    "metadata",
    "mews",
    "money",
    "moose",
    "mumps",
    "music",
    "nankingese",
    "news",
    "nexus",
    "niasese",
    "nutrition",
    "offspring",
    "oil",
    "patience",
    "pekingese",
    "piedmontese",
    "pincers",
    "pistoiese",
    "plankton",
    "pliers",
    "pokemon",
    "police",
    "polish",
    "portuguese",
    "proceedings",
    "progress",
    "rabies",
    "rain",
    "research",
    "rhinoceros",
    "rice",
    "salmon",
    "sand",
    "sarawakese",
    "scissors",
    "sea bass",
    "sea-bass",
    "series",
    "shavese",
    "shears",
    "sheep",
    "siemens",
    "silk",
    "sms",
    "soap",
    "social media",
    "spam",
    "species",
    "staff",
    "sugar",
    "swine",
    "talent",
    "toothpaste",
    "traffic",
    "travel",
    "trousers",
    "trout",
    "tuna",
    "us",
    "vermontese",
    "vinegar",
    "weather",
    "wenchowese",
    "wheat",
    "whiting",
    "wildebeest",
    "wood",
    "wool",
    "yengeese",
    "people",
    "trivia",
    "media",
];

/// Doctrine's irregular plurals, matched whole and case-insensitively.
const IRREGULAR: &[(&str, &str)] = &[
    ("abuse", "abuses"),
    ("alga", "algae"),
    ("atlas", "atlases"),
    ("avalanche", "avalanches"),
    ("axis", "axes"),
    ("axe", "axes"),
    ("beef", "beefs"),
    ("blouse", "blouses"),
    ("brother", "brothers"),
    ("brownie", "brownies"),
    ("cache", "caches"),
    ("cafe", "cafes"),
    ("canvas", "canvases"),
    ("cave", "caves"),
    ("chateau", "chateaux"),
    ("child", "children"),
    ("cookie", "cookies"),
    ("corpus", "corpuses"),
    ("cow", "cows"),
    ("criterion", "criteria"),
    ("curriculum", "curricula"),
    ("curve", "curves"),
    ("demo", "demos"),
    ("die", "dice"),
    ("domino", "dominoes"),
    ("echo", "echoes"),
    ("emphasis", "emphases"),
    ("epoch", "epochs"),
    ("foe", "foes"),
    ("foot", "feet"),
    ("fungus", "fungi"),
    ("ganglion", "ganglions"),
    ("gas", "gases"),
    ("genie", "genies"),
    ("genus", "genera"),
    ("goose", "geese"),
    ("graffito", "graffiti"),
    ("grave", "graves"),
    ("hippopotamus", "hippopotami"),
    ("hoax", "hoaxes"),
    ("hoof", "hoofs"),
    ("human", "humans"),
    ("iris", "irises"),
    ("larva", "larvae"),
    ("leaf", "leaves"),
    ("lens", "lenses"),
    ("loaf", "loaves"),
    ("man", "men"),
    ("medium", "media"),
    ("memorandum", "memoranda"),
    ("money", "monies"),
    ("mongoose", "mongooses"),
    ("motto", "mottoes"),
    ("move", "moves"),
    ("mythos", "mythoi"),
    ("neurosis", "neuroses"),
    ("niche", "niches"),
    ("niveau", "niveaux"),
    ("nucleus", "nuclei"),
    ("numen", "numina"),
    ("nursery", "nurseries"),
    ("oasis", "oases"),
    ("occiput", "occiputs"),
    ("octopus", "octopuses"),
    ("opus", "opuses"),
    ("ox", "oxen"),
    ("passerby", "passersby"),
    ("penis", "penises"),
    ("person", "people"),
    ("plateau", "plateaux"),
    ("runner-up", "runners-up"),
    ("safe", "safes"),
    ("save", "saves"),
    ("sex", "sexes"),
    ("sieve", "sieves"),
    ("soliloquy", "soliloquies"),
    ("son-in-law", "sons-in-law"),
    ("stadium", "stadiums"),
    ("syllabus", "syllabi"),
    ("testis", "testes"),
    ("thief", "thieves"),
    ("tooth", "teeth"),
    ("tornado", "tornadoes"),
    ("trilby", "trilbys"),
    ("turf", "turfs"),
    ("valve", "valves"),
    ("volcano", "volcanoes"),
    ("wave", "waves"),
    ("zombie", "zombies"),
];

/// Doctrine's `RulesetInflector::inflect` for English plurals: uninflected, then irregular, then the
/// first regular rule whose pattern matches.
fn inflect(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    let word_like = |rest: &str| !rest.is_empty() && rest.chars().all(is_word_char);
    if UNINFLECTED.contains(&lower.as_str())
        || lower.strip_suffix("media").is_some_and(word_like)
        || lower.strip_suffix("ware").is_some_and(word_like)
    {
        return word.to_owned();
    }
    if let Some((_, to)) = IRREGULAR.iter().find(|(from, _)| *from == lower) {
        // The first letter's case kept, as Doctrine's substitution does.
        let mut out = (*to).to_owned();
        if word.starts_with(|ch: char| ch.is_uppercase()) {
            out[..1].make_ascii_uppercase();
        }
        return out;
    }
    regular(word, &lower)
}

/// PCRE's `\w`, which is what the inflector's patterns are written in.
fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// The regular rules, in Doctrine's order, each a suffix rewrite of the original spelling: the
/// part of the word a rule keeps keeps its case, and what it writes is lower case.
fn regular(word: &str, lower: &str) -> String {
    let keep = |cut: usize, add: &str| format!("{}{add}", &word[..word.len() - cut]);
    let ends = |suffix: &str| lower.ends_with(suffix);
    let before = |suffix: &str| lower[..lower.len() - suffix.len()].chars().next_back();

    if ends("status") {
        return keep(0, "es");
    }
    if ends("quiz") {
        return keep(0, "zes");
    }
    if lower == "ox" {
        return keep(0, "en");
    }
    if ends("mouse") || ends("louse") {
        return keep(4, "ice");
    }
    for stem in ["matr", "vert", "ind"] {
        for tail in ["ix", "ex"] {
            if ends(&format!("{stem}{tail}")) {
                return keep(2, "ices");
            }
        }
    }
    if ends("x") || ends("ch") || ends("ss") || ends("sh") {
        return keep(0, "es");
    }
    if ends("y")
        && (ends("quy")
            || before("y").is_some_and(|ch| !matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')))
    {
        return keep(1, "ies");
    }
    if ends("hive") || ends("gulf") {
        return keep(0, "s");
    }
    if ends("fe") && before("fe").is_some_and(|ch| ch != 'f') {
        return keep(2, "ves");
    }
    if ends("f") && before("f").is_some_and(|ch| ch == 'l' || ch == 'r') {
        return keep(1, "ves");
    }
    if ends("sis") {
        return keep(3, "ses");
    }
    if (ends("tum") || ends("ium")) && lower.len() >= 3 {
        return keep(2, "a");
    }
    if ends("taxon") {
        return keep(2, "a");
    }
    if ends("criterion") {
        return keep(2, "a");
    }
    if ends("person") {
        return keep(5, "eople");
    }
    if ends("man") {
        return keep(2, "en");
    }
    if ends("child") {
        return keep(0, "ren");
    }
    if ends("foot") {
        return keep(3, "eet");
    }
    for stem in ["buffal", "her", "potat", "tomat", "volcan"] {
        if ends(&format!("{stem}o")) {
            return keep(0, "es");
        }
    }
    for stem in [
        "alumn", "bacill", "cact", "foc", "fung", "nucle", "radi", "stimul", "syllab", "termin",
        "vir",
    ] {
        if ends(&format!("{stem}us")) {
            return keep(2, "i");
        }
    }
    if ends("us") {
        return keep(2, "uses");
    }
    if ends("alias") {
        return keep(0, "es");
    }
    for stem in ["analys", "ax", "cris", "test", "thes"] {
        if ends(&format!("{stem}is")) {
            return keep(2, "es");
        }
    }
    if ends("s") {
        return word.to_owned();
    }
    keep(0, "s")
}

/// Every name the first argument writes: one for a string, each one for an array of them.
fn first_argument_names(call: &BuilderCall) -> Vec<String> {
    call.arguments.first().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use expect_test::{Expect, expect};
    use phrost_model::schema::{BuilderCall, ColumnChange};

    fn call(method: &str, arguments: &[&[&str]], then: &[(&str, Option<bool>)]) -> BuilderCall {
        BuilderCall {
            method: method.to_owned(),
            arguments: arguments
                .iter()
                .map(|written| written.iter().map(|value| (*value).to_owned()).collect())
                .collect(),
            then: then
                .iter()
                .map(|(method, argument)| ((*method).to_owned(), *argument))
                .collect(),
        }
    }

    fn check(calls: &[BuilderCall], expected: Expect) {
        let rendered = calls
            .iter()
            .flat_map(super::columns)
            .map(|change| match change {
                ColumnChange::Defined {
                    name,
                    holds,
                    nullable,
                    constrained,
                } => format!(
                    "+ {name}: {}{}{}\n",
                    if nullable { "?" } else { "" },
                    holds.as_str(),
                    if constrained { ", constrained" } else { "" }
                ),
                ColumnChange::Constrained(name) => format!("! {name} constrained\n"),
                ColumnChange::Dropped(name) => format!("- {name}\n"),
                ColumnChange::Renamed { from, to } => format!("~ {from} -> {to}\n"),
            })
            .collect::<String>();
        expected.assert_eq(&rendered);
    }

    /// A key the database enforces, written on the column or as a statement of its own -- and a
    /// `foreign()` that names no table it references is not one.
    #[test]
    fn a_foreign_key_the_database_enforces_is_marked() {
        check(
            &[
                call("unsignedBigInteger", &[&["account_id"]], &[]),
                call(
                    "foreign",
                    &[&["account_id"]],
                    &[("references", None), ("on", None)],
                ),
                call(
                    "foreign",
                    &[&["owner_id", "team_id"]],
                    &[("references", None)],
                ),
                call("foreign", &[&["loose_id"]], &[]),
                call("foreignId", &[&["vault_id"]], &[("references", None)]),
            ],
            expect![[r#"
                + account_id: int
                ! account_id constrained
                ! owner_id constrained
                ! team_id constrained
                + vault_id: int, constrained
            "#]],
        );
    }

    #[test]
    fn a_column_holds_what_the_database_hands_back() {
        check(
            &[
                call("id", &[], &[]),
                call("string", &[&["email"], &[]], &[("unique", None)]),
                call("text", &[&["bio"]], &[("nullable", None)]),
                call("integer", &[&["votes"]], &[("nullable", Some(false))]),
                call("decimal", &[&["price"], &[], &[]], &[]),
                call("boolean", &[&["active"]], &[("default", None)]),
                call("json", &[&["settings"]], &[]),
                call("dateTime", &[&["seen_at"]], &[("nullable", None)]),
                call("foreignId", &[&["user_id"]], &[("constrained", None)]),
                call("foreignIdFor", &[&["App\\Models\\BookShelf"]], &[]),
                call("timestamps", &[], &[]),
                call("softDeletes", &[], &[]),
                call("rememberToken", &[], &[]),
                call("nullableMorphs", &[&["taggable"]], &[]),
                call("index", &[&["email"]], &[]),
                call("string", &[], &[]),
            ],
            expect![[r#"
                + id: int
                + email: string
                + bio: ?string
                + votes: int
                + price: string
                + active: mixed
                + settings: string
                + seen_at: ?string
                + user_id: int, constrained
                + book_shelf_id: int
                + created_at: ?string
                + updated_at: ?string
                + deleted_at: ?string
                + remember_token: ?string
                + taggable_type: ?string
                + taggable_id: ?int
            "#]],
        );
    }

    #[test]
    fn a_change_takes_columns_away_or_renames_them() {
        check(
            &[
                call("dropColumn", &[&["a", "b"]], &[]),
                call("dropColumn", &[&["c"], &["d"]], &[]),
                call("dropTimestamps", &[], &[]),
                call("dropMorphs", &[&["taggable"]], &[]),
                call("renameColumn", &[&["from"], &["to"]], &[]),
                call(
                    "string",
                    &[&["name"]],
                    &[("nullable", None), ("change", None)],
                ),
            ],
            expect![[r#"
                - a
                - b
                - c
                - d
                - created_at
                - updated_at
                - taggable_type
                - taggable_id
                ~ from -> to
                + name: ?string
            "#]],
        );
    }

    /// Each checked against `Str::snake(Str::pluralStudly(...))` on Laravel 12 -- as were all 650
    /// model names in four Laravel projects' code and tests, every one the same.
    #[test]
    fn a_model_names_the_table_laravel_does() {
        let rendered = [
            "Book",
            "Bookshelf",
            "PageRevision",
            "Activity",
            "Person",
            "Status",
            "Address",
            "Category",
            "Day",
            "Child",
            "Index",
            "Analysis",
            "Software",
            "Medium",
            "Data",
            "APIToken",
            "Box",
            "Knife",
            "Wolf",
            "Hero",
            "Bus",
            "Alias",
            "Man",
            "Woman",
            "Human",
            "Information",
            "News",
            "Quiz",
        ]
        .iter()
        .map(|class| format!("{class} -> {}\n", super::default_table(class)))
        .collect::<String>();
        expect![[r#"
            Book -> books
            Bookshelf -> bookshelves
            PageRevision -> page_revisions
            Activity -> activities
            Person -> people
            Status -> statuses
            Address -> addresses
            Category -> categories
            Day -> days
            Child -> children
            Index -> indices
            Analysis -> analyses
            Software -> software
            Medium -> media
            Data -> data
            APIToken -> a_p_i_tokens
            Box -> boxes
            Knife -> knives
            Wolf -> wolves
            Hero -> heroes
            Bus -> buses
            Alias -> aliases
            Man -> men
            Woman -> women
            Human -> humans
            Information -> information
            News -> news
            Quiz -> quizzes
        "#]]
        .assert_eq(&rendered);
    }
}
