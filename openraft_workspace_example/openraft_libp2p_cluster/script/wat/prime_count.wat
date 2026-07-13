;; prime-count: a REAL computation stored as data in a task payload.
;;
;; Reads a decimal limit N from argv[1] (WASI args_get), counts the primes
;; <= N by trial division, formats the count as decimal and writes it plus
;; a newline to stdout. π(1000) = 168, which the test asserts exactly.
;;
;; Exercises the full code-as-data plumbing: argv passing through the p2
;; wasi:cli/arguments interface (via the preview1 adapter), a CPU loop that
;; consumes measurable fuel, integer formatting, and captured stdout.
;;
;; Exit codes: 1 = missing or non-numeric argv[1].
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory 1)
  (export "memory" (memory 0))

  ;; memory map:
  ;;      0..4    argc
  ;;      4..8    argv buffer size
  ;;     16..112  argv pointer array
  ;;    128..     argv string buffer (NUL-terminated strings)
  ;;   1024..1032 iovec { base, len }
  ;;   1040..1044 nwritten
  ;;   1056..1088 decimal output, written backwards, '\n' at 1087

  (func $is_prime (param $n i32) (result i32)
    (local $d i32)
    (if (i32.lt_u (local.get $n) (i32.const 2))
      (then (return (i32.const 0))))
    (local.set $d (i32.const 2))
    (block $done
      (loop $check
        (br_if $done
          (i32.gt_u
            (i32.mul (local.get $d) (local.get $d))
            (local.get $n)))
        (if (i32.eqz (i32.rem_u (local.get $n) (local.get $d)))
          (then (return (i32.const 0))))
        (local.set $d (i32.add (local.get $d) (i32.const 1)))
        (br $check)))
    (i32.const 1))

  (func (export "_start")
    (local $limit i32)
    (local $i i32)
    (local $count i32)
    (local $p i32)
    (local $c i32)
    (local $digits i32)

    ;; fetch argv
    (drop (call $args_sizes_get (i32.const 0) (i32.const 4)))
    (drop (call $args_get (i32.const 16) (i32.const 128)))
    (if (i32.lt_u (i32.load (i32.const 0)) (i32.const 2))
      (then (call $proc_exit (i32.const 1))))

    ;; parse decimal argv[1] (NUL-terminated)
    (local.set $p (i32.load (i32.const 20)))
    (block $parsed
      (loop $parse
        (local.set $c (i32.load8_u (local.get $p)))
        (br_if $parsed (i32.eqz (local.get $c)))
        (if (i32.or
              (i32.lt_u (local.get $c) (i32.const 48))
              (i32.gt_u (local.get $c) (i32.const 57)))
          (then (call $proc_exit (i32.const 1))))
        (local.set $limit
          (i32.add
            (i32.mul (local.get $limit) (i32.const 10))
            (i32.sub (local.get $c) (i32.const 48))))
        (local.set $p (i32.add (local.get $p) (i32.const 1)))
        (br $parse)))

    ;; count primes <= limit
    (local.set $i (i32.const 2))
    (block $counted
      (loop $sieve
        (br_if $counted (i32.gt_u (local.get $i) (local.get $limit)))
        (local.set $count
          (i32.add (local.get $count) (call $is_prime (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $sieve)))

    ;; format count as decimal, backwards, ending with '\n' at 1087
    (local.set $digits (i32.const 1087))
    (i32.store8 (local.get $digits) (i32.const 10))
    (loop $fmt
      (local.set $digits (i32.sub (local.get $digits) (i32.const 1)))
      (i32.store8 (local.get $digits)
        (i32.add (i32.const 48) (i32.rem_u (local.get $count) (i32.const 10))))
      (local.set $count (i32.div_u (local.get $count) (i32.const 10)))
      (br_if $fmt (i32.ne (local.get $count) (i32.const 0))))

    ;; fd_write(stdout, iov, 1, nwritten)
    (i32.store (i32.const 1024) (local.get $digits))
    (i32.store (i32.const 1028) (i32.sub (i32.const 1088) (local.get $digits)))
    (drop (call $fd_write
      (i32.const 1) (i32.const 1024) (i32.const 1) (i32.const 1040)))))
