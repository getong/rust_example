;; hello: the minimal stored-handler demo — write a fixed string to stdout
;; via WASI fd_write. See prime_count.wat for the full argv/compute example.
(module
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory 1)
  (export "memory" (memory 0))
  (data (i32.const 8) "hello from wasm task\n")
  (func (export "_start")
    (i32.store (i32.const 0) (i32.const 8))
    (i32.store (i32.const 4) (i32.const 21))
    (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 32)))))
