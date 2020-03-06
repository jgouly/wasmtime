//! X86_64-bit Instruction Set Architecture.

#![allow(unused_imports)]
#![allow(dead_code)]

use crate::binemit::{CodeSink, MemoryCodeSink, RelocSink, StackmapSink, TrapSink};
use crate::ir::Function;
use crate::isa::Builder as IsaBuilder;
use crate::isa::TargetIsa;
use crate::machinst::{
    compile, MachBackend, MachCompileResult, ShowWithRRU, TargetIsaAdapter, VCode,
};
use crate::result::CodegenResult;
use crate::settings;

use alloc::boxed::Box;
use alloc::vec::Vec;
use std::str::FromStr;
use std::string::String;

use regalloc::RealRegUniverse;
use target_lexicon::Triple;

// New backend:
mod abi;
mod inst;
mod lower;

use inst::create_reg_universe;

/// An X64 backend.
pub struct X64Backend {
    flags: settings::Flags,
}

impl X64Backend {
    /// Create a new X64 backend.
    pub fn new() -> X64Backend {
        X64Backend {
            flags: settings::Flags::new(settings::builder()),
        }
    }

    /// Create a new X64 backend with the given (shared) flags.
    pub fn new_with_flags(flags: settings::Flags) -> X64Backend {
        X64Backend { flags }
    }

    fn compile_vcode(&self, mut func: Function) -> VCode<inst::Inst> {
        // This performs lowering to VCode, register-allocates the code, computes
        // block layout and finalizes branches. The result is ready for binary emission.
        let abi = Box::new(abi::X64ABIBody::new(&func));
        compile::compile::<X64Backend>(&mut func, self, abi)
    }
}

impl MachBackend for X64Backend {
    fn compile_function(
        &self,
        func: Function,
        want_disasm: bool,
    ) -> CodegenResult<MachCompileResult> {
        let vcode = self.compile_vcode(func);
        let sections = vcode.emit();
        let frame_size = vcode.frame_size();

        let disasm = if want_disasm {
            Some(vcode.show_rru(Some(&create_reg_universe())))
        } else {
            None
        };

        Ok(MachCompileResult {
            sections,
            frame_size,
            disasm,
        })
    }

    fn flags(&self) -> &settings::Flags {
        &self.flags
    }

    fn name(&self) -> &'static str {
        "x64"
    }

    fn triple(&self) -> Triple {
        FromStr::from_str("x86_64").unwrap()
    }

    fn reg_universe(&self) -> RealRegUniverse {
        create_reg_universe()
    }
}

/// Create a new `isa::Builder`.
pub fn isa_builder(triple: Triple) -> IsaBuilder {
    IsaBuilder {
        triple,
        setup: settings::builder(),
        constructor: isa_constructor,
    }
}

fn isa_constructor(
    _: Triple,
    shared_flags: settings::Flags,
    _arch_flag_builder: settings::Builder,
) -> Box<dyn TargetIsa> {
    let backend = X64Backend::new_with_flags(shared_flags);
    Box::new(TargetIsaAdapter::new(backend))
}
