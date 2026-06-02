# ark-mpc workspace example

This is a small standalone workspace modeled after Renegade's `mpc-analysis.org`
notes and the raw `ark_mpc` examples.

It shows two layers:

- `run_scalar_product_example`: the minimal `my_val -> share_scalar -> a * b ->
  open_authenticated` flow.
- `run_mini_order_example`: a tiny Renegade-style business object that is
  flattened into scalars, allocated with `batch_share_scalar`, computed on as
  authenticated shares, then opened with batched MAC authentication.
- `run_additive_certificate_example`: the share style present in Renegade's
  `SecretShareType` / `StateWrapper`: private share + public share reconstructs
  the full object by field-wise addition.
- `run_threshold_certificate_example`: a standalone Shamir-style 3-of-5 example
  where any three certificate shares reconstruct the same certificate fields.

In the Renegade code this example was based on, `SecretShareType::add_shares`
and `StateWrapper::private_shares/public_share` are additive two-share patterns.
I did not find an existing `k-of-n` threshold certificate reconstruction flow in
the visible project code, so the threshold module is included here as a clear
contrast to the existing additive pattern.

Run it with:

```sh
cargo test
cargo run -p ark_mpc_example
```

The example pins `ark-mpc` to the same git revision used by the Renegade
workspace lockfile that this was based on.
