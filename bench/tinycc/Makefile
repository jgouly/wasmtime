CC := gcc
WASI_CC := /opt/wasi-sdk/bin/clang
CFLAGS := -O2
WASMTIME := wasmtime

.PHONY: all
all: tinycc tinycc.wasm tinycc.wat

tinycc: tinycc.c
	$(CC) $(CFLAGS) -o tinycc tinycc.c

tinycc.wasm: tinycc.c
	$(WASI_CC) $(CFLAGS) -o tinycc.wasm tinycc.c

tinycc.wat: tinycc.wasm
	wasm2wat tinycc.wasm > tinycc.wat

.PHONY: clean
clean:
	rm -f tinycc tinycc.wasm tinycc.wat test_input_for_tinycc.o

.PHONY: run
run: tinycc.wasm
	$(WASMTIME) run --dir . ./tinycc.wasm -- -c test_input_for_tinycc.c
