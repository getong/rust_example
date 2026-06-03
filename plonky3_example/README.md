# Plonky3 Examples

This project contains a standalone copy of the Plonky3 example program using
crates.io `p3-*` dependencies.

## Build

```bash
cargo build
```

Build the example targets too:

```bash
cargo build --examples
```

## Run

`cargo run` without arguments starts the binary but fails because the CLI
requires several options:

```text
--field <FIELD>
--objective <OBJECTIVE>
--log-trace-length <LOG_TRACE_LENGTH>
--merkle-hash <MERKLE_HASH>
```

Pass program arguments after `--`:

```bash
cargo run -- --field baby-bear --objective blake-3-permutations --log-trace-length 4 --discrete-fourier-transform radix-2-dit-parallel --merkle-hash keccak-f
```

Show all options:

```bash
cargo run -- --help
```

## Options

Fields:

```text
baby-bear
koala-bear
mersenne-31
```

Proof objectives:

```text
blake-3-permutations
poseidon-2-permutations
keccak-f-permutations
```

Merkle hashes:

```text
poseidon-2
keccak-f
```

DFT options:

```text
radix-2-dit-parallel
recursive-dft
small-batch-dft
```

For `baby-bear` and `koala-bear`, provide `--discrete-fourier-transform`.
For `mersenne-31`, omit `--discrete-fourier-transform`.

## Examples

BabyBear + Blake3 AIR + Keccak Merkle tree:

```bash
cargo run -- --field baby-bear --objective blake-3-permutations --log-trace-length 4 --discrete-fourier-transform radix-2-dit-parallel --merkle-hash keccak-f
```

KoalaBear + Poseidon2 AIR + Poseidon2 Merkle tree:

```bash
cargo run -- --field koala-bear --objective poseidon-2-permutations --log-trace-length 4 --discrete-fourier-transform recursive-dft --merkle-hash poseidon-2
```

Mersenne31 + Keccak AIR + Keccak Merkle tree:

```bash
cargo run -- --field mersenne-31 --objective keccak-f-permutations --log-trace-length 8 --merkle-hash keccak-f
```

Run the copied upstream example target directly:

```bash
cargo run --example prove_prime_field_31 -- --field baby-bear --objective blake-3-permutations --log-trace-length 4 --discrete-fourier-transform radix-2-dit-parallel --merkle-hash keccak-f
```

## Notes

The `--log-trace-length` value controls the trace size as a power of two.
Start with small values such as `4` or `8`; larger values can take much longer
to prove.
