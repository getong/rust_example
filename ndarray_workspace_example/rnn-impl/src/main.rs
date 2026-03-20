use anyhow::Result;
use tch::{Device, Kind, Tensor, nn, nn::OptimizerConfig};

/// Make a mini-batch:
/// x_idx: (B,T) ints; y_idx: (B,T) ints; X_onehot: (B,T,V) float
fn make_batch(batch: i64, t_steps: i64, vocab: i64, device: Device) -> (Tensor, Tensor, Tensor) {
  let x_idx = Tensor::randint(vocab, [batch, t_steps], (Kind::Int64, device));
  let y_idx = (&x_idx + 1).remainder(vocab); // teacher: shift-by-1
  let x_onehot = x_idx.one_hot(vocab).to_kind(Kind::Float); // (B,T,V)
  (x_idx, y_idx, x_onehot)
}

fn main() -> Result<()> {
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
  let epochs: i64 = 400;

  // Vanilla RNN cell from linears:
  // h_t = tanh( x_t @ Wx + h_{t-1} @ Wh + b )
  let wx = nn::linear(root / "wx", vocab, hidden, Default::default()); // (V->H)
  let wh = nn::linear(root / "wh", hidden, hidden, Default::default()); // (H->H)
  let wy = nn::linear(root / "wy", hidden, vocab, Default::default()); // (H->V)

  let mut opt = nn::Adam::default().build(&vs, 3e-3)?;

  // Fixed evaluation sample (B=1) to monitor learning
  let (_x_eval_idx, y_eval_idx, x_eval_oh) = make_batch(1, t_steps, vocab, device);

  for epoch in 1 ..= epochs {
    // Training batch
    let (_x_idx, y_idx, x_oh) = make_batch(batch, t_steps, vocab, device);

    // Unroll over time
    let mut h = Tensor::zeros([batch, hidden], (Kind::Float, device)); // (B,H)
    let mut logits_per_t: Vec<Tensor> = Vec::with_capacity(t_steps as usize);

    for t in 0 .. t_steps {
      // slice time step: (B,1,V) -> (B,V)
      let x_t = x_oh.narrow(1, t, 1).squeeze_dim(1);
      // h_t = tanh(Wx x_t + Wh h_{t-1})
      let a = x_t.apply(&wx) + h.apply(&wh);
      h = a.tanh();
      // logits_t = Wy h_t
      let logits_t = h.apply(&wy); // (B,V)
      logits_per_t.push(logits_t);
    }

    // Stack logits to (B,T,V)
    let logits = Tensor::stack(&logits_per_t, 1);

    // Cross-entropy over all steps: (B*T,V) vs (B*T)
    let loss = logits
      .reshape([batch * t_steps, vocab])
      .cross_entropy_for_logits(&y_idx.reshape([batch * t_steps]));

    // Backprop + step
    opt.backward_step(&loss);

    // Log every 10 epochs: loss + eval accuracy on fixed sample
    if epoch % 10 == 0 {
      // Eval forward on the fixed (1,T,V)
      let mut h_eval = Tensor::zeros([1, hidden], (Kind::Float, device));
      let mut eval_logits_per_t: Vec<Tensor> = Vec::with_capacity(t_steps as usize);
      for t in 0 .. t_steps {
        let x_t = x_eval_oh.narrow(1, t, 1).squeeze_dim(1); // (1,V)
        h_eval = (x_t.apply(&wx) + h_eval.apply(&wh)).tanh();
        eval_logits_per_t.push(h_eval.apply(&wy));
      }
      let logits_eval = Tensor::stack(&eval_logits_per_t, 1); // (1,T,V)
      let preds = logits_eval.argmax(-1, false); // (1,T)

      // --- Convert tensors to Vec<i64> via iterator API ---
      let preds_vec: Vec<i64> = preds
        .to_device(Device::Cpu)
        .view([-1])
        .iter::<i64>()?
        .collect();
      let y_vec: Vec<i64> = y_eval_idx
        .to_device(Device::Cpu)
        .view([-1])
        .iter::<i64>()?
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

  // Final quick sample
  let (_x_idx, _y_idx, x_oh) = make_batch(1, t_steps, vocab, device); // (1,T,V)
  let mut h = Tensor::zeros([1, hidden], (Kind::Float, device));
  let mut preds: Vec<i64> = Vec::new();
  for t in 0 .. t_steps {
    let x_t = x_oh.narrow(1, t, 1).squeeze_dim(1); // (1,V)
    h = (x_t.apply(&wx) + h.apply(&wh)).tanh();
    let logits_t = h.apply(&wy); // (1,V)
    let pred = logits_t.argmax(-1, false).int64_value(&[0]);
    preds.push(pred);
  }
  println!("pred indices over time: {:?}", preds);

  Ok(())
}
