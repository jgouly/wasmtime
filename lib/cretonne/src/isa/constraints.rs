//! Register constraints for instruction operands.
//!
//! An encoding recipe specifies how an instruction is encoded as binary machine code, but it only
//! works if the operands and results satisfy certain constraints. Constraints on immediate
//! operands are checked by instruction predicates when the recipe is chosen.
//!
//! It is the register allocator's job to make sure that the register constraints on value operands
//! are satisfied.

use binemit::CodeOffset;
use isa::{RegClass, RegUnit};

/// Register constraint for a single value operand or instruction result.
pub struct OperandConstraint {
    /// The kind of constraint.
    pub kind: ConstraintKind,

    /// The register class of the operand.
    ///
    /// This applies to all kinds of constraints, but with slightly different meaning.
    pub regclass: RegClass,
}

/// The different kinds of operand constraints.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    /// This operand or result must be a register from the given register class.
    Reg,

    /// This operand or result must be a fixed register.
    ///
    /// The constraint's `regclass` field is the top-level register class containing the fixed
    /// register.
    FixedReg(RegUnit),

    /// This result value must use the same register as an input value operand.
    ///
    /// The associated number is the index of the input value operand this result is tied to. The
    /// constraint's `regclass` field is the same as the tied operand's register class.
    ///
    /// When an (in, out) operand pair is tied, this constraint kind appears in both the `ins` and
    /// the `outs` arrays. The constraint for the in operand is `Tied(out)`, and the constraint for
    /// the out operand is `Tied(in)`.
    Tied(u8),

    /// This operand must be a value in a stack slot.
    ///
    /// The constraint's `regclass` field is the register class that would normally be used to load
    /// and store values of this type.
    Stack,
}

/// Value operand constraints for an encoding recipe.
#[derive(Clone)]
pub struct RecipeConstraints {
    /// Constraints for the instruction's fixed value operands.
    ///
    /// If the instruction takes a variable number of operands, the register constraints for those
    /// operands must be computed dynamically.
    ///
    /// - For branches and jumps, EBB arguments must match the expectations of the destination EBB.
    /// - For calls and returns, the calling convention ABI specifies constraints.
    pub ins: &'static [OperandConstraint],

    /// Constraints for the instruction's fixed results.
    ///
    /// If the instruction produces a variable number of results, it's probably a call and the
    /// constraints must be derived from the calling convention ABI.
    pub outs: &'static [OperandConstraint],

    /// Are any of the input constraints `FixedReg`?
    pub fixed_ins: bool,

    /// Are any of the output constraints `FixedReg`?
    pub fixed_outs: bool,

    /// Are there any tied operands?
    pub tied_ops: bool,
}

/// Constraints on the range of a branch instruction.
///
/// A branch instruction usually encodes its destination as a signed n-bit offset from an origin.
/// The origin depends on the ISA and the specific instruction:
///
/// - RISC-V and ARM Aarch64 use the address of the branch instruction, `origin = 0`.
/// - Intel uses the address of the instruction following the branch, `origin = 2` for a 2-byte
///   branch instruction.
/// - ARM's A32 encoding uses the address of the branch instruction + 8 bytes, `origin = 8`.
#[derive(Clone, Copy)]
pub struct BranchRange {
    /// Offset in bytes from the address of the branch instruction to the origin used for computing
    /// the branch displacement. This is the destination of a branch that encodes a 0 displacement.
    pub origin: u8,

    /// Number of bits in the signed byte displacement encoded in the instruction. This does not
    /// account for branches that can only target aligned addresses.
    pub bits: u8,
}

impl BranchRange {
    /// Determine if this branch range can represent the range from `branch` to `dest`, where
    /// `branch` is the code offset of the branch instruction itself and `dest` is the code offset
    /// of the destination EBB header.
    ///
    /// This method does not detect if the range is larger than 2 GB.
    pub fn contains(self, branch: CodeOffset, dest: CodeOffset) -> bool {
        let d = dest.wrapping_sub(branch + CodeOffset::from(self.origin)) as i32;
        let s = 32 - self.bits;
        d == d << s >> s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_range() {
        // ARM T1 branch.
        let t1 = BranchRange { origin: 4, bits: 9 };
        assert!(t1.contains(0, 0));
        assert!(t1.contains(0, 2));
        assert!(t1.contains(2, 0));
        assert!(t1.contains(1000, 1000));

        // Forward limit.
        assert!(t1.contains(1000, 1258));
        assert!(!t1.contains(1000, 1260));

        // Backward limit
        assert!(t1.contains(1000, 748));
        assert!(!t1.contains(1000, 746));

    }
}
