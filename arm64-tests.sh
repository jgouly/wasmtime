#!/bin/bash

set -e

(cd cranelift/ && cargo build)
(cd cranelift/codegen && cargo test)

for f in cranelift/filetests/filetests/vcode/arm64/*.clif; do
  echo $f
  target/debug/clif-util test $f
done
