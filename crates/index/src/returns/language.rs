//! What PHP itself guarantees that its own signatures do not say.
//!
//! The standard library reports failure two ways, and which one a call uses is a *runtime setting*
//! rather than a property of the signature. `PDO::prepare()` is declared `PDOStatement|false` and
//! returns `false` only under `PDO::ERRMODE_SILENT`; under `ERRMODE_EXCEPTION` it throws, and that
//! has been the default since PHP 8.0. The stub's own docblock says so in prose -- the fact was
//! always there, and only the check could not read it.
//!
//! Here rather than behind a framework provider, for two reasons. A provider is installed only when
//! it is *detected*, so a project with no framework -- which is the shape this was found on -- would
//! get nothing. And PDO is not a framework: it is the language, whose declarations this project
//! already ships.
//!
//! Not in the stub either, and that was measured rather than assumed: a docblock cannot narrow a
//! stated hint -- `@return PDOStatement` and `@phpstan-return PDOStatement` beside
//! `: PDOStatement|false` both lose to it, by design -- so the fact has nowhere to live in the
//! declaration. A local stub edit is also silently reverted by a re-vendor.
//!
//! Only ever *removes* a sentinel, exactly as a framework's non-null contract does, so the worst it
//! can do is miss a `false` somebody reached by asking for the silent error mode on purpose.

use phrost_model::BuiltinTy;

use super::ReturnContracts;

/// The methods whose declared `false` only an explicitly non-default error mode produces.
///
/// `PDOStatement::fetch` and its neighbours are deliberately absent: their `false` means *no more
/// rows*, which every run can reach and every caller has to handle.
const UNPRODUCED_FALSE: &[(&str, &str)] = &[
    ("PDO", "prepare"),
    ("PDO", "query"),
    ("PDO", "exec"),
    ("PDO", "lastInsertId"),
];

impl ReturnContracts {
    /// Installs what the language guarantees. Unconditional: every project has PHP.
    pub fn install_language_contracts(&mut self) {
        for (class, method) in UNPRODUCED_FALSE {
            // A real `false`, not an unreachable one: `PDO::ERRMODE_SILENT` produces it, and a
            // project that sets it is entitled to be told this was decided for it.
            self.add_unproduced(
                class,
                method,
                BuiltinTy::False,
                Some("the connection was put in PDO::ERRMODE_SILENT, and the default throws"),
                // A mode set on the connection, not an argument of this call.
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is installed, and only for the sentinel it is about.
    ///
    /// Also a tripwire on the fetch family: their `false` means *no more rows*, which every run
    /// reaches, so a name added here by mistake would silence a real failure branch.
    #[test]
    fn the_language_contracts_name_pdos_configuration_dependent_failures() {
        let mut contracts = ReturnContracts::default();
        contracts.install_language_contracts();

        for method in ["prepare", "query", "exec", "lastInsertId"] {
            assert_eq!(
                contracts
                    .unproduced_declarers(method, BuiltinTy::False)
                    .collect::<Vec<_>>(),
                ["PDO"],
                "{method} should carry the contract"
            );
            assert!(
                contracts
                    .unproduced_declarers(method, BuiltinTy::Null)
                    .next()
                    .is_none(),
                "{method} says nothing about null"
            );
        }

        for method in ["fetch", "fetchAll", "fetchColumn"] {
            assert!(
                contracts
                    .unproduced_declarers(method, BuiltinTy::False)
                    .next()
                    .is_none(),
                "{method} returns a real false when the rows run out"
            );
        }
    }
}
