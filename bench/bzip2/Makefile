CC := gcc
WASI_CC := /opt/wasi-sdk/bin/clang
CFLAGS := -O2

.PHONY: all
all: bz2 bz2.wasm bz2.wat

bz2: bz2.c
	$(CC) $(CFLAGS) -o bz2 bz2.c

bz2.wasm: bz2.c
	$(WASI_CC) $(CFLAGS) -o bz2.wasm bz2.c

bz2.wat: bz2.wasm
	wasm2wat bz2.wasm > bz2.wat

.PHONY: clean
clean:
	rm -f bz2 bz2.wasm bz2.wat
