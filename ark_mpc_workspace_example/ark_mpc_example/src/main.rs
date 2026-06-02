use ark_mpc_example::{
  run_mini_order_example, run_scalar_product_example,
  threshold_certificate::{run_additive_certificate_example, run_threshold_certificate_example},
};

#[tokio::main]
async fn main() {
  let product = run_scalar_product_example().await;
  println!("party0_value * party1_value = {product}");

  let order = run_mini_order_example().await;
  println!(
    "filled_base={}, quote_total={}, remaining_base={}",
    order.filled_base, order.quote_total, order.remaining_base
  );

  let additive_certificate = run_additive_certificate_example();
  println!(
    "additive certificate serial={}, holder={}, issuer={}",
    additive_certificate.serial, additive_certificate.holder_id, additive_certificate.issuer_id
  );

  let threshold_certificate = run_threshold_certificate_example();
  println!(
    "{}-of-{} certificate recovered from shares {:?}: serial={}, holder={}, issuer={}",
    threshold_certificate.threshold,
    threshold_certificate.total_shares,
    threshold_certificate.used_indices,
    threshold_certificate.recovered.serial,
    threshold_certificate.recovered.holder_id,
    threshold_certificate.recovered.issuer_id
  );
}
