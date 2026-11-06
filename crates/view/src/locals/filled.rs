//! What a literal holds once the writes into it since are counted.
//!
//! The body layer finds the writes -- see `phrost_body::slots_filled_since` -- and this folds them
//! into what the literal said, slot by slot, because what a write means depends on what the literal
//! was: a record learns a key, a list learns another kind of element, and either of them stops being
//! what it was when a write names a slot nothing here can.

use phrost_body::{FilledSlot, SlotFill};
use phrost_hir::FileHir;
use phrost_model::{BuiltinTy, Name};

use crate::{resolve::Held, view::IndexView};

/// How many keys a record filled in after the literal may grow to before it is an array.
///
/// Above the literal's own cap, because what that bounds is reading entries nobody asks about, and
/// every write counted here has been resolved already. Bounded still, because a render array is
/// built one key at a time and each key is a value resolved on every read of the variable.
const FILLED_ENTRIES: usize = 16;

/// What the literal has become so far.
enum Filling {
    Record(Vec<(Name, Held)>),
    List(Held),
    /// Keyed by something nothing here can name, holding one of these.
    Map(Held),
    /// An array, and nothing more can be said.
    Bare,
}

impl IndexView<'_> {
    /// The literal `held`, with every write in `fills` applied in source order.
    ///
    /// Each rule says no more than the writes do. A write in a branch may not have run, so it adds a
    /// possibility -- the slot's old value stays beside the new one -- and never replaces one. A key
    /// written conditionally is still put in the record, which claims the key is there: nothing reads
    /// a record's keys as a promise, and the alternative is losing every slot the literal named for
    /// one that may be missing. A value this cannot name is `mixed` in a record, where it fills one
    /// slot, and the end of the element type in a list, where a silent entry would make the union
    /// narrower than the truth -- the rule the literal itself is read by.
    pub(super) fn filled_in(
        &self,
        hir: &FileHir,
        held: Held,
        fills: &[SlotFill],
        budget: u8,
    ) -> Held {
        let mut filling = match held {
            Held::Record(fields) => Filling::Record(fields.into_vec()),
            Held::ListOf(element) => Filling::List(*element),
            other => return other,
        };
        for fill in fills {
            let value = match (&fill.value, fill.compound) {
                (Some(path), false) => self.resolve_path(fill.at, hir, path, budget),
                _ => None,
            };
            // A write through a slot leaves an array there and says nothing more about it.
            let value = match fill.slot {
                FilledSlot::Through(_) => Some(Held::Builtin(BuiltinTy::Array)),
                _ => value,
            };
            filling = match (filling, &fill.slot) {
                (Filling::Bare, _) => Filling::Bare,
                (Filling::Record(fields), FilledSlot::Removed(key)) => {
                    Self::out_of_record(fields, key.as_ref(), fill.certain)
                }
                // A list with a hole in it is keyed rather than listed, and holds what it held.
                (Filling::List(element) | Filling::Map(element), FilledSlot::Removed(_)) => {
                    Filling::Map(element)
                }
                (Filling::Record(fields), slot) => Self::into_record(fields, slot, fill, value),
                // An append keys the next integer, and a write through an element of a list
                // replaces what one element holds and keys nothing new.
                (Filling::List(element), FilledSlot::Appended | FilledSlot::Through(_)) => {
                    match value {
                        Some(value) => Filling::List(Self::one_of(vec![element, value])),
                        None => Filling::Bare,
                    }
                }
                (Filling::List(element) | Filling::Map(element), _) => {
                    Self::beside(element, value, fill.compound)
                }
            };
        }
        match filling {
            Filling::Record(fields) => Held::Record(fields.into_boxed_slice()),
            Filling::List(element) => Held::ListOf(Box::new(element)),
            Filling::Map(value) => Held::MapOf {
                key: Box::new(Self::one_of(vec![
                    Held::Builtin(BuiltinTy::Int),
                    Held::Builtin(BuiltinTy::String),
                ])),
                value: Box::new(value),
            },
            Filling::Bare => Held::Builtin(BuiltinTy::Array),
        }
    }

    /// One write into a record.
    ///
    /// A named key is a slot like the literal's own. An append takes PHP's next index -- one past
    /// the largest integer key, or `0` -- when it certainly ran; when it may not have, how many
    /// entries there are is the question, and the record becomes the list (or the map) it is
    /// growing into.
    fn into_record(
        mut fields: Vec<(Name, Held)>,
        slot: &FilledSlot,
        fill: &SlotFill,
        value: Option<Held>,
    ) -> Filling {
        let named = match slot {
            FilledSlot::Named(key) | FilledSlot::Through(Some(key)) => Some(key.clone()),
            FilledSlot::Appended if fill.certain => {
                let next = fields
                    .iter()
                    .filter(|(key, _)| phrost_model::is_an_integer_key(key.as_str()))
                    .filter_map(|(key, _)| key.as_str().parse::<usize>().ok())
                    .max()
                    .map_or(Some(0), |largest| largest.checked_add(1));
                next.and_then(phrost_model::slot_name)
            }
            _ => None,
        };
        let Some(key) = named else {
            let listed = matches!(slot, FilledSlot::Appended)
                && fields
                    .iter()
                    .all(|(key, _)| phrost_model::is_an_integer_key(key.as_str()));
            let values = Self::one_of(fields.into_iter().map(|(_, value)| value).collect());
            return match (listed, value) {
                (true, Some(value)) => Filling::List(Self::one_of(vec![values, value])),
                (true, None) => Filling::Bare,
                (false, value) => Self::beside(values, value, fill.compound),
            };
        };
        match fields.iter_mut().find(|(written, _)| *written == key) {
            // A compound write keeps the slot's own kind -- `.=` a string, `+=` a number -- which is
            // all the operator settles without the operand.
            Some(_) if fill.compound => {}
            Some((_, old)) => {
                let new = value.unwrap_or(Held::Builtin(BuiltinTy::Mixed));
                *old = match fill.certain {
                    true => new,
                    false => Self::one_of(vec![old.clone(), new]),
                };
            }
            None => fields.push((key, value.unwrap_or(Held::Builtin(BuiltinTy::Mixed)))),
        }
        // Past the cap the record is an array and no more, and not a map of the union: a map
        // answers every key with every value, which for `$payload['html_content']` is the
        // attachments and the headers as well -- a confident answer where there was a precise one.
        match fields.len() > FILLED_ENTRIES {
            true => Filling::Bare,
            false => Filling::Record(fields),
        }
    }

    /// One key taken out of a record.
    ///
    /// Only a removal that certainly ran, of a key the statement names, takes the slot out. One in a
    /// branch leaves the key where it was -- the record then claims a key that may be gone, which is
    /// the side a write in a branch errs on too -- and `unset($row[$key])` may have removed any of
    /// them, so it removes none.
    fn out_of_record(mut fields: Vec<(Name, Held)>, key: Option<&Name>, certain: bool) -> Filling {
        if let (Some(key), true) = (key, certain) {
            fields.retain(|(written, _)| written != key);
        }
        Filling::Record(fields)
    }

    /// A container keyed by what nothing here names, holding `held` and whatever this write put
    /// beside it. A compound write adds nothing new to the union; a value nothing could name ends
    /// it.
    fn beside(held: Held, value: Option<Held>, compound: bool) -> Filling {
        match (value, compound) {
            (_, true) => Filling::Map(held),
            (Some(value), false) => Filling::Map(Self::one_of(vec![held, value])),
            (None, false) => Filling::Bare,
        }
    }
}
