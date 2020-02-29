Cranelift: ARM64 and new-x86 porting effort
===========================================

- New code is in
  - cranelift/codegen/src/machinst/ (target-independent backend infra)
  - cranelift/codegen/src/isa/arm64/ (ARM64 target-specific)
  - cranelift/codegen/src/isa/x64/ (x86 target-specific)

- Invocations to try:

  - cd cranelift/codegen && cargo test
    - this runs unit tests, including encoding checks against golden bytes.

  - target/debug/clif-util compile --target arm64 \
                           -d -D -p cranelift/filetests/filetests/vcode/arm64/file.clif
    - this compiles a test input (in CraneLift IR) and prints:
      - ARM64 assembly, which should be parseable by GNU as (-D flag)
      - Machine code, in 32-bit words (-p flag)
      - if RUST_LOG=debug is set, debug spew (-d flag)

  - the same, but with --target x86_64
    - does not do anything yet

  - target/debug/clif-util test cranelift/filetests/filetests/vcode/arm64/file.clif
    - this runs the "filecheck" utility, which performs specified checks
      against the ARM64-assembly (really vcode) output of compilation
      - see https://docs.rs/filecheck/0.4.0/filecheck/ for directives

Building an aarch64 binary
==========================

- Check out https://github.com/cfallin/wasmtime, `arm64` branch

- Install aarch64-linux-gnu-gcc (Arch Linux: package aarch64-linux-gnu-gcc)
- Install qemu-aarch64 (Arch Linux: package qemu-arch-extra)

- Add to ~/.cargo/config:

    ```
    [target.aarch64-unknown-linux-gnu]
    linker = "aarch64-linux-gnu-gcc"
    ```

- Install rust aarch64 target:

    ```
    $ rustup target add aarch64-unknown-linux-gnu
    ```

- In wasmtime, run:

    ```
    $ cargo build --release --target aarch64-unknown-linux-gnu
    ```

- Run (using ~/test.wasm):

    ```
    $ LD_LIBRARY_PATH=/usr/aarch64-linux-gnu/lib64/ \
      QEMU_LD_PREFIX=/usr/aarch64-linux-gnu/ \
      qemu-aarch64 target/aarch64-unknown-linux-gnu/release/wasmtime \
      ~/test.wasm
    ```
