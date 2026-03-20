use tch::{
  Device, Kind, Tensor, nn,
  nn::{OptimizerConfig, RNN},
}; // bring the trait into scope to use .seq(...)

/// Make a mini-batch:
/// x_idx: (B,T) ints; y_idx: (B,T) ints; x_onehot: (B,T,V) floats
fn make_batch(batch: i64, t_steps: i64, vocab: i64, device: Device) -> (Tensor, Tensor, Tensor) {
  let x_idx = Tensor::randint(vocab, [batch, t_steps], (Kind::Int64, device));
  let y_idx = (&x_idx + 1).remainder(vocab); // teacher: shift-by-1
  let x_onehot = x_idx.one_hot(vocab).to_kind(Kind::Float); // (B,T,V)
  (x_idx, y_idx, x_onehot)
}

fn main() {
  // Reproducibility
  tch::manual_seed(42);

  // Setup
  let device = Device::cuda_if_available();
  let vs = nn::VarStore::new(device);
  let root = &vs.root();

  // Dims / hyperparams
  let vocab: i64 = 6; // input & output size (one-hot)
  let hidden: i64 = 16; // hidden size
  let t_steps: i64 = 8; // sequence length
  let batch: i64 = 32; // batch size
  let epochs: i64 = 120;

  // LSTM configured as batch_first, so we feed (B,T,I) to .seq(...)
  let lstm = nn::lstm(
    root / "lstm",
    vocab,  // input size I
    hidden, // hidden size H
    nn::RNNConfig {
      num_layers: 1,
      bidirectional: false,
      batch_first: true, // IMPORTANT: expect (B,T,I)
      ..Default::default()
    },
  );
  let wy = nn::linear(root / "wy", hidden, vocab, Default::default()); // (H->V)

  let mut opt = nn::Adam::default().build(&vs, 1e-3).unwrap();

  // Fixed evaluation sample (B=1) to monitor learning
  let (_x_eval_idx, y_eval_idx, x_eval_oh) = make_batch(1, t_steps, vocab, device); // (1,T,V)

  for epoch in 1 ..= epochs {
    // Training batch (B,T,V)
    let (_x_idx, y_idx, x_oh) = make_batch(batch, t_steps, vocab, device);

    // Forward: LSTM -> (B,T,H), then linear head -> (B,T,V)
    let (h_seq, _state) = lstm.seq(&x_oh); // respects batch_first=true
    let logits = h_seq.apply(&wy); // (B,T,V)

    // Loss over all steps: (B*T,V) vs (B*T)
    let loss = logits
      .reshape([batch * t_steps, vocab])
      .cross_entropy_for_logits(&y_idx.reshape([batch * t_steps]));

    // Step
    opt.backward_step(&loss);

    // Log every 10 epochs: loss + eval accuracy on the fixed sample
    if epoch % 10 == 0 {
      // Eval on fixed (1,T,V)
      let (h_eval, _st) = lstm.seq(&x_eval_oh); // (1,T,H)
      let logits_eval = h_eval.apply(&wy); // (1,T,V)
      let preds = logits_eval.argmax(-1, false); // (1,T)

      // Tensor -> Vec<i64> and compute accuracy in Rust
      let preds_vec: Vec<i64> = preds
        .to_device(Device::Cpu)
        .view([-1])
        .iter::<i64>()
        .unwrap()
        .collect();
      let y_vec: Vec<i64> = y_eval_idx
        .to_device(Device::Cpu)
        .view([-1])
        .iter::<i64>()
        .unwrap()
        .collect();
      let correct = preds_vec
        .iter()
        .zip(y_vec.iter())
        .filter(|(a, b)| a == b)
        .count();
      let acc = correct as f64 / preds_vec.len() as f64;

      let loss_val = loss.to_device(Device::Cpu).double_value(&[]);
      println!(
        "epoch {:3} | loss {:.4} | eval acc {:>5.1}%",
        epoch,
        loss_val,
        acc * 100.0
      );
    }
  }

  // Final quick sample: show predicted indices over time
  let (_x_idx, _y_idx, x_oh) = make_batch(1, t_steps, vocab, device); // (1,T,V)
  let (h_seq, _st) = lstm.seq(&x_oh); // (1,T,H)
  let logits = h_seq.apply(&wy); // (1,T,V)
  let preds = logits.argmax(-1, false); // (1,T)
  let preds_vec: Vec<i64> = preds
    .to_device(Device::Cpu)
    .view([-1])
    .iter::<i64>()
    .unwrap()
    .collect();
  println!("pred indices over time: {:?}", preds_vec);
}
