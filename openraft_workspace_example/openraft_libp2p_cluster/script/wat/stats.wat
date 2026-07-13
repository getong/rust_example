;; stats: the "rich" stored handler — a mini analytics function that shows
;; every input/output channel of the code-as-data pipeline at once:
;;
;;   - argv[1..]  a list of decimal numbers (multiple --arg values)
;;   - env LABEL  a caller-supplied tag (looked up in WASI environ)
;;   - stdout     a JSON document ASSEMBLED INSIDE THE GUEST:
;;                {"label":"...","count":N,"sum":N,"min":N,"max":N,"avg":N}
;;
;; The task result therefore contains machine-checkable JSON produced by
;; the stored function itself; the test parses it and asserts every field.
;; Exit code 1 = no numbers given or a non-numeric argument.
(module
  (import "wasi_snapshot_preview1" "args_sizes_get"
    (func $args_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "args_get"
    (func $args_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "environ_sizes_get"
    (func $environ_sizes_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "environ_get"
    (func $environ_get (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_write"
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit"
    (func $proc_exit (param i32)))
  (memory 1)
  (export "memory" (memory 0))

  ;; memory map:
  ;;      0..4    argc              4..8    argv buffer size
  ;;      8..12   environ count    12..16   environ buffer size
  ;;     16..144  argv pointer array (up to 32 args)
  ;;    144..272  environ pointer array (up to 32 vars)
  ;;    300..390  constant JSON fragments + "LABEL=" needle (data below)
  ;;    512..2048 argv string buffer
  ;;   2048..4096 environ string buffer
  ;;   4096..4104 iovec { base, len }    4112..4116 nwritten
  ;;   4608..     JSON output, built forward through $out

  (data (i32.const 300) "{\22label\22:\22")  ;; len 10
  (data (i32.const 316) "\22,\22count\22:")  ;; len 10
  (data (i32.const 332) ",\22sum\22:")       ;; len 7
  (data (i32.const 344) ",\22min\22:")       ;; len 7
  (data (i32.const 356) ",\22max\22:")       ;; len 7
  (data (i32.const 368) ",\22avg\22:")       ;; len 7
  (data (i32.const 380) "}\0a")              ;; len 2
  (data (i32.const 384) "LABEL=")            ;; len 6

  (global $out (mut i32) (i32.const 4608))

  (func $emit_byte (param $c i32)
    (i32.store8 (global.get $out) (local.get $c))
    (global.set $out (i32.add (global.get $out) (i32.const 1))))

  ;; copy $len bytes from $ptr into the output
  (func $emit_seg (param $ptr i32) (param $len i32)
    (block $done
      (loop $copy
        (br_if $done (i32.eqz (local.get $len)))
        (call $emit_byte (i32.load8_u (local.get $ptr)))
        (local.set $ptr (i32.add (local.get $ptr) (i32.const 1)))
        (local.set $len (i32.sub (local.get $len) (i32.const 1)))
        (br $copy))))

  ;; copy a NUL-terminated string into the output
  (func $emit_cstr (param $ptr i32)
    (local $c i32)
    (block $done
      (loop $copy
        (local.set $c (i32.load8_u (local.get $ptr)))
        (br_if $done (i32.eqz (local.get $c)))
        (call $emit_byte (local.get $c))
        (local.set $ptr (i32.add (local.get $ptr) (i32.const 1)))
        (br $copy))))

  ;; recursive decimal formatter (most significant digit first)
  (func $emit_num (param $n i32)
    (if (i32.ge_u (local.get $n) (i32.const 10))
      (then (call $emit_num (i32.div_u (local.get $n) (i32.const 10)))))
    (call $emit_byte
      (i32.add (i32.const 48) (i32.rem_u (local.get $n) (i32.const 10)))))

  ;; parse a NUL-terminated decimal string; exits 1 on any non-digit
  (func $parse_dec (param $ptr i32) (result i32)
    (local $c i32)
    (local $v i32)
    (block $done
      (loop $parse
        (local.set $c (i32.load8_u (local.get $ptr)))
        (br_if $done (i32.eqz (local.get $c)))
        (if (i32.or
              (i32.lt_u (local.get $c) (i32.const 48))
              (i32.gt_u (local.get $c) (i32.const 57)))
          (then (call $proc_exit (i32.const 1))))
        (local.set $v
          (i32.add
            (i32.mul (local.get $v) (i32.const 10))
            (i32.sub (local.get $c) (i32.const 48))))
        (local.set $ptr (i32.add (local.get $ptr) (i32.const 1)))
        (br $parse)))
    (local.get $v))

  ;; scan environ for "LABEL=..."; returns pointer to the value or 0
  (func $find_label (result i32)
    (local $i i32)
    (local $count i32)
    (local $p i32)
    (local $j i32)
    (local $matched i32)
    (local.set $count (i32.load (i32.const 8)))
    (block $notfound
      (loop $vars
        (br_if $notfound (i32.ge_u (local.get $i) (local.get $count)))
        (local.set $p
          (i32.load (i32.add (i32.const 144) (i32.mul (local.get $i) (i32.const 4)))))
        (local.set $j (i32.const 0))
        (local.set $matched (i32.const 1))
        (block $cmp_done
          (loop $cmp
            (br_if $cmp_done (i32.ge_u (local.get $j) (i32.const 6)))
            (if (i32.ne
                  (i32.load8_u (i32.add (local.get $p) (local.get $j)))
                  (i32.load8_u (i32.add (i32.const 384) (local.get $j))))
              (then
                (local.set $matched (i32.const 0))
                (br $cmp_done)))
            (local.set $j (i32.add (local.get $j) (i32.const 1)))
            (br $cmp)))
        (if (local.get $matched)
          (then (return (i32.add (local.get $p) (i32.const 6)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $vars)))
    (i32.const 0))

  (func (export "_start")
    (local $argc i32)
    (local $i i32)
    (local $v i32)
    (local $count i32)
    (local $sum i32)
    (local $min i32)
    (local $max i32)
    (local $label i32)

    (drop (call $args_sizes_get (i32.const 0) (i32.const 4)))
    (drop (call $args_get (i32.const 16) (i32.const 512)))
    (drop (call $environ_sizes_get (i32.const 8) (i32.const 12)))
    (drop (call $environ_get (i32.const 144) (i32.const 2048)))

    (local.set $argc (i32.load (i32.const 0)))
    (if (i32.lt_u (local.get $argc) (i32.const 2))
      (then (call $proc_exit (i32.const 1))))

    ;; fold count/sum/min/max over argv[1..]
    (local.set $i (i32.const 1))
    (local.set $min (i32.const -1)) ;; u32::MAX
    (block $stats_done
      (loop $stats
        (br_if $stats_done (i32.ge_u (local.get $i) (local.get $argc)))
        (local.set $v
          (call $parse_dec
            (i32.load (i32.add (i32.const 16) (i32.mul (local.get $i) (i32.const 4))))))
        (local.set $count (i32.add (local.get $count) (i32.const 1)))
        (local.set $sum (i32.add (local.get $sum) (local.get $v)))
        (if (i32.lt_u (local.get $v) (local.get $min))
          (then (local.set $min (local.get $v))))
        (if (i32.gt_u (local.get $v) (local.get $max))
          (then (local.set $max (local.get $v))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $stats)))

    ;; assemble the JSON report
    (call $emit_seg (i32.const 300) (i32.const 10))
    (local.set $label (call $find_label))
    (if (local.get $label)
      (then (call $emit_cstr (local.get $label))))
    (call $emit_seg (i32.const 316) (i32.const 10))
    (call $emit_num (local.get $count))
    (call $emit_seg (i32.const 332) (i32.const 7))
    (call $emit_num (local.get $sum))
    (call $emit_seg (i32.const 344) (i32.const 7))
    (call $emit_num (local.get $min))
    (call $emit_seg (i32.const 356) (i32.const 7))
    (call $emit_num (local.get $max))
    (call $emit_seg (i32.const 368) (i32.const 7))
    (call $emit_num (i32.div_u (local.get $sum) (local.get $count)))
    (call $emit_seg (i32.const 380) (i32.const 2))

    ;; flush [4608, $out) to stdout
    (i32.store (i32.const 4096) (i32.const 4608))
    (i32.store (i32.const 4100) (i32.sub (global.get $out) (i32.const 4608)))
    (drop (call $fd_write
      (i32.const 1) (i32.const 4096) (i32.const 1) (i32.const 4112)))))
