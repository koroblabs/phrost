//! `$record?->status === PreferenceStatus::Requested` cannot hold where `$record` is null, so the code
//! that runs because it held reads `$record->...` with a plain arrow -- and is right to.
//!
//! The shapes are the reference project's own, all five of them reported as possibly-null receivers
//! until an identity with an enum case was read as proof. The control is the one place the same
//! chain proves nothing: the right of an `||` whose left *held*, where `$record` may still be null.

use expect_test::expect;

use super::diagnostics::check;

#[test]
fn a_receiver_whose_chain_matched_an_enum_case_is_there() {
    check(
        r#"
//- /app.php
<?php
enum PreferenceStatus: string
{
    case Requested = 'r';
    case Declined = 'd';
}

final class PreferenceRecord
{
    public function __construct(
        public PreferenceStatus $status,
        public ?array $baseQuote = null,
        public ?string $selection = null,
    ) {}
}

interface Preferences
{
    public function find(int $nid): ?PreferenceRecord;
}

final class PreferenceService
{
    public function __construct(private Preferences $preferences) {}

    public function restore(int $nid): array
    {
        $record = $this->preferences->find($nid);
        if ($record?->status !== PreferenceStatus::Requested || $record->baseQuote === NULL) {
            return [];
        }
        return $record->baseQuote;
    }

    public function requested(int $nid): ?string
    {
        $record = $this->preferences->find($nid);
        return $record?->status === PreferenceStatus::Requested ? $record->selection : NULL;
    }

    public function control(int $nid): bool
    {
        $record = $this->preferences->find($nid);
        return $record?->status === PreferenceStatus::Requested || $record->selection === NULL;
    }
}
"#,
        expect![[r#"
            44:68-44:75 warning [possibly-null-receiver] Cannot access property $selection on ?PreferenceRecord
        "#]],
    );
}
