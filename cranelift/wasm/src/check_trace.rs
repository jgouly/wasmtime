use cranelift_codegen::ir::types::{I16, I32, I64, I8};
use cranelift_codegen::ir::{AbiParam, ExternalName, InstBuilder, SigRef, Signature, Value};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::FunctionBuilder;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::vec::Vec;

/// A check-trace generator. This inserts calls to a runtime that prints a
/// check-trace from Wasm execution.
pub struct CheckTracer {
    int_constants: Vec<Value>,
    hook_fn_sig: SigRef,
    hook_fn_local: Value,
    instance: Option<Value>,
}

fn func_num(builder: &mut FunctionBuilder) -> Value {
    let func_num = match &builder.func.name {
        &ExternalName::User { index, .. } => index as i64,
        _ => -1,
    };
    builder.ins().iconst(I64, func_num)
}

fn cast64(builder: &mut FunctionBuilder, val: Value, zero: Value) -> Value {
    let ty = builder.func.dfg.value_type(val);
    match ty {
        I64 => val,
        I32 | I16 | I8 => builder.ins().uextend(I64, val),
        _ => zero,
    }
}

impl CheckTracer {
    pub(crate) fn new(builder: &mut FunctionBuilder) -> CheckTracer {
        // Set up some iconsts to share.
        let int_constants: Vec<Value> = (0..5).map(|i| builder.ins().iconst(I64, i)).collect();

        // Create the signature for the hook function.
        let mut hook_fn_sig = Signature::new(CallConv::SystemV);
        for _ in 0..5 {
            hook_fn_sig.params.push(AbiParam::new(I64));
        }
        hook_fn_sig.returns.push(AbiParam::new(I64));
        let hook_fn_sig = builder.import_signature(hook_fn_sig);

        // Import the address of the hook function as a constant. We assume we are compiling in a
        // JIT context, so the actual function in Cranelift's .text will be called directly by the
        // compiled code.
        let hook_fn = checktracer_hook as *const u8 as i64;
        let hook_fn_local = builder.ins().iconst(I64, hook_fn);

        let mut tracer = CheckTracer {
            int_constants,
            hook_fn_sig,
            hook_fn_local,
            instance: None,
        };

        let arg1 = tracer.int_constants[CHECKTRACER_HOOK_ENTER as usize];
        let arg2 = func_num(builder);
        let arg3 = tracer.int_constants[0];
        let arg4 = tracer.int_constants[0];
        let arg5 = tracer.int_constants[0];
        let ret = tracer.emit_call(builder, arg1, arg2, arg3, arg4, arg5);
        tracer.instance = Some(ret);

        tracer
    }

    pub(crate) fn leave(&mut self, builder: &mut FunctionBuilder) {
        let arg1 = self.int_constants[CHECKTRACER_HOOK_LEAVE as usize];
        let arg2 = self.instance.clone().unwrap();
        let arg3 = func_num(builder);
        let arg4 = self.int_constants[0];
        let arg5 = self.int_constants[0];
        self.emit_call(builder, arg1, arg2, arg3, arg4, arg5);
    }

    pub(crate) fn load(&mut self, builder: &mut FunctionBuilder, pc: u32, addr: Value, val: Value) {
        let arg1 = self.int_constants[CHECKTRACER_HOOK_LOAD as usize];
        let arg2 = self.instance.clone().unwrap();
        let arg3 = builder.ins().iconst(I64, pc as i64);
        let arg4 = cast64(builder, addr, self.int_constants[0]);
        let arg5 = cast64(builder, val, self.int_constants[0]);
        self.emit_call(builder, arg1, arg2, arg3, arg4, arg5);
    }

    pub(crate) fn store(
        &mut self,
        builder: &mut FunctionBuilder,
        pc: u32,
        addr: Value,
        val: Value,
    ) {
        let arg1 = self.int_constants[CHECKTRACER_HOOK_STORE as usize];
        let arg2 = self.instance.clone().unwrap();
        let arg3 = builder.ins().iconst(I64, pc as i64);
        let arg4 = cast64(builder, addr, self.int_constants[0]);
        let arg5 = cast64(builder, val, self.int_constants[0]);
        self.emit_call(builder, arg1, arg2, arg3, arg4, arg5);
    }

    fn emit_call(
        &mut self,
        builder: &mut FunctionBuilder,
        arg1: Value,
        arg2: Value,
        arg3: Value,
        arg4: Value,
        arg5: Value,
    ) -> Value {
        let inst = builder.ins().call_indirect(
            self.hook_fn_sig,
            self.hook_fn_local,
            &[arg1, arg2, arg3, arg4, arg5],
        );
        builder.func.dfg.first_result(inst)
    }
}

const CHECKTRACER_HOOK_ENTER: u64 = 1;
const CHECKTRACER_HOOK_LEAVE: u64 = 2;
const CHECKTRACER_HOOK_LOAD: u64 = 3;
const CHECKTRACER_HOOK_STORE: u64 = 4;

extern "C" fn checktracer_hook(arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> u64 {
    match arg1 {
        CHECKTRACER_HOOK_ENTER => checktracer_enter(arg2, arg3, arg4, arg5),
        CHECKTRACER_HOOK_LEAVE => checktracer_leave(arg2, arg3, arg4, arg5),
        CHECKTRACER_HOOK_LOAD => checktracer_load(arg2, arg3, arg4, arg5),
        CHECKTRACER_HOOK_STORE => checktracer_store(arg2, arg3, arg4, arg5),
        _ => unimplemented!(),
    }
}

fn checktracer_enter(func_num: u64, _: u64, _: u64, _: u64) -> u64 {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let instance = SERIAL.fetch_add(1, Ordering::Relaxed) as u64;
    eprintln!("==T== enter {} {}", instance, func_num);
    instance
}

fn checktracer_leave(instance: u64, func_num: u64, _: u64, _: u64) -> u64 {
    eprintln!("==T== leave {} {}", instance, func_num);
    0
}

fn checktracer_load(instance: u64, pc: u64, address: u64, value: u64) -> u64 {
    eprintln!(
        "==T== load {} 0x{:x} 0x{:x} 0x{:x}",
        instance, pc, address, value
    );
    0
}

fn checktracer_store(instance: u64, pc: u64, address: u64, value: u64) -> u64 {
    eprintln!(
        "==T== store {} 0x{:x} 0x{:x} 0x{:x}",
        instance, pc, address, value
    );
    0
}
