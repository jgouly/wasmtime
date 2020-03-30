//! Support types for generated encoding tables.
//!
//! This module contains types and functions for working with the encoding tables generated by
//! `cranelift-codegen/meta/src/gen_encodings.rs`.

#![allow(dead_code)] // TODO keep this until the new backend is finished.

use crate::constant_hash::{probe, Table};
use crate::ir::{Function, InstructionData, Opcode, Type};
use crate::isa::{Encoding, Legalize};
use crate::settings::PredicateView;
use core::ops::Range;

/// A recipe predicate.
///
/// This is a predicate function capable of testing ISA and instruction predicates simultaneously.
///
/// A None predicate is always satisfied.
pub type RecipePredicate = Option<fn(PredicateView, &InstructionData) -> bool>;

/// An instruction predicate.
///
/// This is a predicate function that needs to be tested in addition to the recipe predicate. It
/// can't depend on ISA settings.
pub type InstPredicate = fn(&Function, &InstructionData) -> bool;

/// Legalization action to perform when no encoding can be found for an instruction.
///
/// This is an index into an ISA-specific table of legalization actions.
pub type LegalizeCode = u8;

/// Level 1 hash table entry.
///
/// One level 1 hash table is generated per CPU mode. This table is keyed by the controlling type
/// variable, using `INVALID` for non-polymorphic instructions.
///
/// The hash table values are references to level 2 hash tables, encoded as an offset in `LEVEL2`
/// where the table begins, and the binary logarithm of its length. All the level 2 hash tables
/// have a power-of-two size.
///
/// Entries are generic over the offset type. It will typically be `u32` or `u16`, depending on the
/// size of the `LEVEL2` table.
///
/// Empty entries are encoded with a `!0` value for `log2len` which will always be out of range.
/// Entries that have a `legalize` value but no level 2 table have an `offset` field that is out of
/// bounds.
pub struct Level1Entry<OffT: Into<u32> + Copy> {
    pub ty: Type,
    pub log2len: u8,
    pub legalize: LegalizeCode,
    pub offset: OffT,
}

impl<OffT: Into<u32> + Copy> Level1Entry<OffT> {
    /// Get the level 2 table range indicated by this entry.
    fn range(&self) -> Range<usize> {
        let b = self.offset.into() as usize;
        b..b + (1 << self.log2len)
    }
}

impl<OffT: Into<u32> + Copy> Table<Type> for [Level1Entry<OffT>] {
    fn len(&self) -> usize {
        self.len()
    }

    fn key(&self, idx: usize) -> Option<Type> {
        if self[idx].log2len != !0 {
            Some(self[idx].ty)
        } else {
            None
        }
    }
}

/// Level 2 hash table entry.
///
/// The second level hash tables are keyed by `Opcode`, and contain an offset into the `ENCLISTS`
/// table where the encoding recipes for the instruction are stored.
///
/// Entries are generic over the offset type which depends on the size of `ENCLISTS`. A `u16`
/// offset allows the entries to be only 32 bits each. There is no benefit to dropping down to `u8`
/// for tiny ISAs. The entries won't shrink below 32 bits since the opcode is expected to be 16
/// bits.
///
/// Empty entries are encoded with a `NotAnOpcode` `opcode` field.
pub struct Level2Entry<OffT: Into<u32> + Copy> {
    pub opcode: Option<Opcode>,
    pub offset: OffT,
}

impl<OffT: Into<u32> + Copy> Table<Opcode> for [Level2Entry<OffT>] {
    fn len(&self) -> usize {
        self.len()
    }

    fn key(&self, idx: usize) -> Option<Opcode> {
        self[idx].opcode
    }
}

/// Two-level hash table lookup and iterator construction.
///
/// Given the controlling type variable and instruction opcode, find the corresponding encoding
/// list.
///
/// Returns an iterator that produces legal encodings for `inst`.
pub fn lookup_enclist<'a, OffT1, OffT2>(
    ctrl_typevar: Type,
    inst: &'a InstructionData,
    func: &'a Function,
    level1_table: &'static [Level1Entry<OffT1>],
    level2_table: &'static [Level2Entry<OffT2>],
    enclist: &'static [EncListEntry],
    legalize_actions: &'static [Legalize],
    recipe_preds: &'static [RecipePredicate],
    inst_preds: &'static [InstPredicate],
    isa_preds: PredicateView<'a>,
) -> Encodings<'a>
where
    OffT1: Into<u32> + Copy,
    OffT2: Into<u32> + Copy,
{
    let (offset, legalize) = match probe(level1_table, ctrl_typevar, ctrl_typevar.index()) {
        Err(l1idx) => {
            // No level 1 entry found for the type.
            // We have a sentinel entry with the default legalization code.
            (!0, level1_table[l1idx].legalize)
        }
        Ok(l1idx) => {
            // We have a valid level 1 entry for this type.
            let l1ent = &level1_table[l1idx];
            let offset = match level2_table.get(l1ent.range()) {
                Some(l2tab) => {
                    let opcode = inst.opcode();
                    match probe(l2tab, opcode, opcode as usize) {
                        Ok(l2idx) => l2tab[l2idx].offset.into() as usize,
                        Err(_) => !0,
                    }
                }
                // The l1ent range is invalid. This means that we just have a customized
                // legalization code for this type. The level 2 table is empty.
                None => !0,
            };
            (offset, l1ent.legalize)
        }
    };

    // Now we have an offset into `enclist` that is `!0` when no encoding list could be found.
    // The default legalization code is always valid.
    Encodings::new(
        offset,
        legalize,
        inst,
        func,
        enclist,
        legalize_actions,
        recipe_preds,
        inst_preds,
        isa_preds,
    )
}

/// Encoding list entry.
///
/// Encoding lists are represented as sequences of u16 words.
pub type EncListEntry = u16;

/// Number of bits used to represent a predicate. c.f. `meta/src/gen_encodings.rs`.
const PRED_BITS: u8 = 12;
const PRED_MASK: usize = (1 << PRED_BITS) - 1;
/// First code word representing a predicate check. c.f. `meta/src/gen_encodings.rs`.
const PRED_START: usize = 0x1000;

/// An iterator over legal encodings for the instruction.
pub struct Encodings<'a> {
    // Current offset into `enclist`, or out of bounds after we've reached the end.
    offset: usize,
    // Legalization code to use of no encoding is found.
    legalize: LegalizeCode,
    inst: &'a InstructionData,
    func: &'a Function,
    enclist: &'static [EncListEntry],
    legalize_actions: &'static [Legalize],
    recipe_preds: &'static [RecipePredicate],
    inst_preds: &'static [InstPredicate],
    isa_preds: PredicateView<'a>,
}

impl<'a> Encodings<'a> {
    /// Creates a new instance of `Encodings`.
    ///
    /// This iterator provides search for encodings that applies to the given instruction. The
    /// encoding lists are laid out such that first call to `next` returns valid entry in the list
    /// or `None`.
    pub fn new(
        offset: usize,
        legalize: LegalizeCode,
        inst: &'a InstructionData,
        func: &'a Function,
        enclist: &'static [EncListEntry],
        legalize_actions: &'static [Legalize],
        recipe_preds: &'static [RecipePredicate],
        inst_preds: &'static [InstPredicate],
        isa_preds: PredicateView<'a>,
    ) -> Self {
        Encodings {
            offset,
            inst,
            func,
            legalize,
            isa_preds,
            recipe_preds,
            inst_preds,
            enclist,
            legalize_actions,
        }
    }

    /// Get the legalization action that caused the enumeration of encodings to stop.
    /// This can be the default legalization action for the type or a custom code for the
    /// instruction.
    ///
    /// This method must only be called after the iterator returns `None`.
    pub fn legalize(&self) -> Legalize {
        debug_assert_eq!(self.offset, !0, "Premature Encodings::legalize()");
        self.legalize_actions[self.legalize as usize]
    }

    /// Check if the `rpred` recipe predicate is satisfied.
    fn check_recipe(&self, rpred: RecipePredicate) -> bool {
        match rpred {
            Some(p) => p(self.isa_preds, self.inst),
            None => true,
        }
    }

    /// Check an instruction or isa predicate.
    fn check_pred(&self, pred: usize) -> bool {
        if let Some(&p) = self.inst_preds.get(pred) {
            p(self.func, self.inst)
        } else {
            let pred = pred - self.inst_preds.len();
            self.isa_preds.test(pred)
        }
    }
}

impl<'a> Iterator for Encodings<'a> {
    type Item = Encoding;

    fn next(&mut self) -> Option<Encoding> {
        while let Some(entryref) = self.enclist.get(self.offset) {
            let entry = *entryref as usize;

            // Check for "recipe+bits".
            let recipe = entry >> 1;
            if let Some(&rpred) = self.recipe_preds.get(recipe) {
                let bits = self.offset + 1;
                if entry & 1 == 0 {
                    self.offset += 2; // Next entry.
                } else {
                    self.offset = !0; // Stop.
                }
                if self.check_recipe(rpred) {
                    return Some(Encoding::new(recipe as u16, self.enclist[bits]));
                }
                continue;
            }

            // Check for "stop with legalize".
            if entry < PRED_START {
                self.legalize = (entry - 2 * self.recipe_preds.len()) as LegalizeCode;
                self.offset = !0; // Stop.
                return None;
            }

            // Finally, this must be a predicate entry.
            let pred_entry = entry - PRED_START;
            let skip = pred_entry >> PRED_BITS;
            let pred = pred_entry & PRED_MASK;

            if self.check_pred(pred) {
                self.offset += 1;
            } else if skip == 0 {
                self.offset = !0; // Stop.
                return None;
            } else {
                self.offset += 1 + skip;
            }
        }
        None
    }
}
