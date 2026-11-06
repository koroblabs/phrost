//! A member reached through a value that can be a sentinel -- null, or the `false` PHP's standard
//! library hands back to mean the same thing.
//!
//! `?T where T expected` in its commonest and most useful spelling: `$app->getId()` where `$app` is
//! a `?Application`. PHP's answer at runtime is a fatal `TypeError`, so this is not a style note --
//! it is the branch nobody tested, and it is the single most common real bug in typed PHP.
//!
//! # Why this can be turned on at all
//!
//! Strictness here is only liveable because the layer below narrows. A guard --
//! `if (!$app) { return; }`, `if ($app)`, `$app instanceof T`, an `@assert-if-true` method, an
//! `assert()` -- has already taken the null off the type by the time this check sees it, so the
//! check itself needs no flow analysis and cannot disagree with what hover says. Without that, this
//! would report every guarded access in a project and be switched off within a day.
//!
//! `?->` is silent for the same reason: the access handles its own null, and the view says so. It is
//! *not* silent about a `false`, because it does not handle one -- measured on PHP 8.5.10,
//! `$v?->title()` on a `false` is a fatal `Call to a member function title() on false`.
//!
//! Only the *possible* `false` is reported here. A certain one is `non-object-receiver`'s, which
//! already says `Cannot call method title() on false` and says it as an error, because there PHP
//! stops on every run.

use phrost_view::{IndexView, MemberAccess, MemberAccessOutcome, MemberKind, SentinelReach};

use super::{Check, Diagnostic, Severity};

/// Named rather than written twice: `actions::nullsafe` offers the fix for these, and a code
/// spelled out in both places is a link a typo silently breaks.
pub(crate) const NULL_RECEIVER: &str = "null-receiver";
pub(crate) const POSSIBLY_NULL_RECEIVER: &str = "possibly-null-receiver";
pub(crate) const POSSIBLY_FALSE_RECEIVER: &str = "possibly-false-receiver";

pub(super) struct NullableReceiver;

impl Check for NullableReceiver {
    fn run(&self, view: &IndexView<'_>, out: &mut Vec<Diagnostic>) {
        for access in view.member_accesses().iter() {
            // Null first where both can arrive, and one report per receiver: two about one name is
            // how a list stops being read, the guard a person writes is the same either way, and
            // `actions::nullsafe` offers its fix for the null codes only.
            //
            // A bool rather than the enum from here down: the third answer is the one that means
            // there is nothing to report, and carrying it further would put an arm in every match
            // below that cannot be reached.
            let (null, certain) = match (access.null_reach, access.false_reach) {
                (SentinelReach::Possible, _) => (true, false),
                (SentinelReach::Certain, _) => (true, true),
                // A certain `false` belongs to `non-object-receiver`, which reports it as the error
                // it is rather than as a branch nobody tested.
                (SentinelReach::Impossible, SentinelReach::Possible) => (false, false),
                (SentinelReach::Impossible, _) => continue,
            };

            // A possible null is only reported where the member itself is accounted for: an access
            // whose class is unknown, whose hierarchy is broken, or whose member is missing has
            // something else wrong with it, and two reports about one name is how a list stops being
            // read.
            //
            // A certain null is reported either way, because there the member is beside the point --
            // null has no members at all, so nothing else has anything to say about it and there is
            // nothing to collide with.
            let accounted_for = matches!(
                access.outcome,
                MemberAccessOutcome::Resolved { .. }
                    | MemberAccessOutcome::Dynamic { .. }
                    | MemberAccessOutcome::ShapeProperty
            );
            if !certain && !accounted_for {
                continue;
            }

            out.push(Diagnostic {
                // The receiver, not the member name: the null is in the value on the left, and that
                // is the expression a person has to go and guard.
                range: access.receiver_range,
                // A value that is null on every run is not a branch anyone gets away with; one
                // that may be null is a branch nobody tested, which is a warning.
                severity: match certain {
                    true => Severity::Error,
                    false => Severity::Warning,
                },
                code: match (null, certain) {
                    (true, true) => NULL_RECEIVER,
                    (true, false) => POSSIBLY_NULL_RECEIVER,
                    (false, _) => POSSIBLY_FALSE_RECEIVER,
                },
                message: message(null, certain, access),
                unnecessary: false,
                deprecated: false,
            });
        }
    }
}

fn message(null: bool, certain: bool, access: &MemberAccess) -> String {
    let name = access.name.as_str();
    let subject = match access.kind {
        MemberKind::Method => format!("call method {name}()"),
        MemberKind::Property => format!("access property ${name}"),
        MemberKind::ClassConstant | MemberKind::EnumCase => format!("access constant {name}"),
    };
    if certain {
        return format!("Cannot {subject} on null");
    }
    // `Row|false` rather than `?Row`, because that is how the type renders and what the reader will
    // see in a tooltip over the same value.
    if !null {
        return match receiver_type(access) {
            Some(class) => format!("Cannot {subject} on {class}|false"),
            None => format!("Cannot {subject} on a value that may be false"),
        };
    }

    // Named where the name is known, because that is the difference between a note and something
    // actionable: a reader who is told `?Drupal\...\StatementInterface` knows which declaration to
    // go and read, and one told only "a value" has to work it out from the expression. The
    // fully-qualified form, matching what the undefined-member checks say about the same class.
    match receiver_type(access) {
        Some(class) => format!("Cannot {subject} on ?{class}"),
        None => format!("Cannot {subject} on a value that may be null"),
    }
}

/// The receiver's class, when resolving got far enough to name one.
fn receiver_type(access: &MemberAccess) -> Option<&str> {
    match &access.outcome {
        MemberAccessOutcome::Resolved { class }
        | MemberAccessOutcome::Dynamic { class }
        | MemberAccessOutcome::UnknownClass { class }
        | MemberAccessOutcome::IncompleteHierarchy { class }
        | MemberAccessOutcome::MissingMember { class } => Some(class.as_str()),
        // None of these names a class: a value nothing typed, a value typed as something no class
        // can be, a value typed as every class at once, and an object shape.
        MemberAccessOutcome::UnknownReceiver
        | MemberAccessOutcome::NotAnObject { .. }
        | MemberAccessOutcome::AnyObject
        | MemberAccessOutcome::ShapeProperty => None,
    }
}
