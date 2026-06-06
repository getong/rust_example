# num-bigint example

This project demonstrates `num-bigint`, the Rust crate for arbitrary precision
integers.

Rust primitive integers such as `u64`, `i128`, and `u128` are fast and compact,
but they have fixed limits. `num-bigint` provides integer types that can grow as
large as memory allows:

- `BigUint`: non-negative arbitrary precision integers.
- `BigInt`: signed arbitrary precision integers.

The example in `src/main.rs` covers:

- creating big integers from primitive integers and decimal or hexadecimal text;
- addition, subtraction, multiplication, division, and remainder;
- calculating `100!` without overflow;
- exponentiation with `pow`;
- modular exponentiation with `modpow`, a common operation in cryptography;
- signed arithmetic with `BigInt`;
- converting big integers to strings and big-endian bytes.

Run it with:

```sh
cargo run
```

Use `num-bigint` when exact integer results matter and values may exceed the
range of fixed-width integer types. For decimal money or fixed-scale business
values, prefer a decimal crate instead of big integers.
