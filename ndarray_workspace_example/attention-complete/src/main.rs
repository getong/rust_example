use tch::{
  Device, Kind, Tensor, nn,
  nn::{Module, OptimizerConfig},
};

// ------------------------- Sinusoidal Positional Encoding -------------------------
fn sinusoidal_positional_encoding(t_steps: i64, d_model: i64, device: Device) -> Tensor {
  assert!(
    d_model % 2 == 0,
    "d_model must be even for sine/cosine split"
  );
  let pos = Tensor::arange(t_steps, (Kind::Float, device)).unsqueeze(1); // (T,1)
  let i = Tensor::arange(d_model / 2, (Kind::Float, device)); // (d/2)
  // inv_freq_j = 1 / 10000^(2j/d_model) -> use: exp( ln(1/10000) * (2j/d) )
  let inv_freq = ((-10000.0_f64.ln() * 2.0 / d_model as f64) as f32 * &i).exp();
  let angles = &pos * inv_freq.unsqueeze(0); // (T, d/2)
  Tensor::cat(&[angles.sin(), angles.cos()], 1).unsqueeze(0) // (1,T,d)
}

// ------------------------- Multi-Head Self-Attention ------------------------------
struct MHSA {
  w_q: nn::Linear,
  w_k: nn::Linear,
  w_v: nn::Linear,
  w_o: nn::Linear,
  n_heads: i64,
  d_model: i64,
  d_head: i64,
  dropout_p: f64,
}

impl MHSA {
  fn new(vs: &nn::Path, d_model: i64, n_heads: i64, dropout_p: f64) -> Self {
    assert!(
      d_model % n_heads == 0,
      "d_model must be divisible by n_heads"
    );
    let d_head = d_model / n_heads;
    let linear_cfg = nn::LinearConfig {
      bias: true,
      ..Default::default()
    };
    let w_q = nn::linear(vs / "w_q", d_model, d_model, linear_cfg);
    let w_k = nn::linear(vs / "w_k", d_model, d_model, linear_cfg);
    let w_v = nn::linear(vs / "w_v", d_model, d_model, linear_cfg);
    let w_o = nn::linear(vs / "w_o", d_model, d_model, linear_cfg);
    Self {
      w_q,
      w_k,
      w_v,
      w_o,
      n_heads,
      d_model,
      d_head,
      dropout_p,
    }
  }

  // (B,T,d) -> (B,T,d)
  fn forward_t(&self, xs: &Tensor, train: bool) -> Tensor {
    let q = xs.apply_t(&self.w_q, train);
    let k = xs.apply_t(&self.w_k, train);
    let v = xs.apply_t(&self.w_v, train);

    let (b, t, _d) = (xs.size()[0], xs.size()[1], xs.size()[2]);
    let q = self.split_heads(&q, b, t); // (B,nH,T,dH)
    let k = self.split_heads(&k, b, t); // (B,nH,T,dH)
    let v = self.split_heads(&v, b, t); // (B,nH,T,dH)

    let scale = (self.d_head as f64).sqrt();
    let scores = q.matmul(&k.transpose(-2, -1)) / scale; // (B,nH,T,T)
    let mut attn = scores.softmax(-1, Kind::Float);
    if self.dropout_p > 0.0 {
      attn = attn.dropout(self.dropout_p, train);
    }
    let context = attn.matmul(&v); // (B,nH,T,dH)
    let concat = self.combine_heads(&context, b, t); // (B,T,d)
    concat.apply_t(&self.w_o, train)
  }

  fn split_heads(&self, x: &Tensor, b: i64, t: i64) -> Tensor {
    x.view([b, t, self.n_heads, self.d_head]) // (B,T,nH,dH)
      .transpose(1, 2) // (B,nH,T,dH)
  }

  fn combine_heads(&self, x: &Tensor, b: i64, t: i64) -> Tensor {
    x.transpose(1, 2) // (B,T,nH,dH)
      .contiguous()
      .view([b, t, self.n_heads * self.d_head]) // (B,T,d)
  }
}

// ------------------------- Encoder Block (Pre-LN) ---------------------------------
struct EncoderBlock {
  ln1: nn::LayerNorm,
  ln2: nn::LayerNorm,
  attn: MHSA,
  ffn: nn::Sequential,
  dropout_p: f64,
}

impl EncoderBlock {
  fn new(vs: &nn::Path, d_model: i64, n_heads: i64, d_ff: i64, dropout_p: f64) -> Self {
    let ln_cfg = nn::LayerNormConfig {
      eps: 1e-5,
      ..Default::default()
    };
    let ln1 = nn::layer_norm(vs / "ln1", vec![d_model], ln_cfg);
    let ln2 = nn::layer_norm(vs / "ln2", vec![d_model], ln_cfg);
    let attn = MHSA::new(&(vs / "attn"), d_model, n_heads, dropout_p);
    let ffn = nn::seq()
      .add(nn::linear(vs / "ff1", d_model, d_ff, Default::default()))
      .add_fn(|x| x.gelu("tanh"))
      .add(nn::linear(vs / "ff2", d_ff, d_model, Default::default()));
    Self {
      ln1,
      ln2,
      attn,
      ffn,
      dropout_p,
    }
  }

  fn forward_t(&self, x: &Tensor, train: bool) -> Tensor {
    // Pre-LN + MHSA + residual
    let h = x.apply_t(&self.ln1, train);
    let mut h = self.attn.forward_t(&h, train);
    if self.dropout_p > 0.0 {
      h = h.dropout(self.dropout_p, train);
    }
    let x = x + h;

    // Pre-LN + FFN + residual
    let h2 = x.apply_t(&self.ln2, train).apply_t(&self.ffn, train);
    let h2 = if self.dropout_p > 0.0 {
      h2.dropout(self.dropout_p, train)
    } else {
      h2
    };
    x + h2
  }
}

// ------------------------- Tiny Encoder Model -------------------------------------
struct SumModTransformer {
  embed: nn::Embedding,
  blocks: Vec<EncoderBlock>,
  ln_f: nn::LayerNorm,
  head: nn::Linear,
  d_model: i64,
  max_t: i64,
  dropout_p: f64,
  device: Device,
}

impl SumModTransformer {
  #[allow(clippy::too_many_arguments)]
  fn new(
    vs: &nn::Path,
    vocab: i64,
    d_model: i64,
    n_heads: i64,
    d_ff: i64,
    n_layers: i64,
    n_classes: i64,
    max_t: i64,
    dropout_p: f64,
    device: Device,
  ) -> Self {
    let embed = nn::embedding(vs / "embed", vocab, d_model, Default::default());
    let mut blocks = Vec::new();
    for i in 0 .. n_layers {
      let b = EncoderBlock::new(
        &(vs / format!("enc{}", i)),
        d_model,
        n_heads,
        d_ff,
        dropout_p,
      );
      blocks.push(b);
    }
    let ln_f = nn::layer_norm(
      vs / "ln_f",
      vec![d_model],
      nn::LayerNormConfig {
        eps: 1e-5,
        ..Default::default()
      },
    );
    let head = nn::linear(vs / "head", d_model, n_classes, Default::default());

    Self {
      embed,
      blocks,
      ln_f,
      head,
      d_model,
      max_t: max_t,
      dropout_p,
      device,
    }
  }

  // x_idx: (B,T) int64 tokens in [0, vocab)
  fn forward_t(&self, x_idx: &Tensor, train: bool) -> Tensor {
    let (_b, t) = (x_idx.size()[0], x_idx.size()[1]);
    // (1,T,d), broadcast over batch
    let pe = sinusoidal_positional_encoding(t, self.d_model, self.device);
    let mut x = self.embed.forward(x_idx) + pe; // (B,T,d)
    if self.dropout_p > 0.0 {
      x = x.dropout(self.dropout_p, train);
    }
    for b in &self.blocks {
      x = b.forward_t(&x, train); // (B,T,d)
    }
    // final LN, mean-pool over time, classifier head
    let x = x
      .apply_t(&self.ln_f, train)
      .mean_dim([1].as_slice(), false, Kind::Float); // (B,d)
    x.apply(&self.head) // (B,C)
  }
}

// ------------------------- Training Utilities -------------------------------------
fn accuracy_from_logits(logits: &Tensor, y: &Tensor) -> f64 {
  let pred = logits.argmax(-1, false);
  let correct = pred.eq_tensor(y).to_kind(Kind::Float).mean(Kind::Float);
  correct.double_value(&[])
}

fn main() -> tch::Result<()> {
  tch::manual_seed(42);

  // ---------------- Hyperparameters ----------------
  let device = Device::cuda_if_available(); // CPU on most Macs
  let vocab: i64 = 10; // tokens 0..9
  let d_model: i64 = 64;
  let n_heads: i64 = 4;
  let d_ff: i64 = 256;
  let n_layers: i64 = 2;
  let n_classes: i64 = 5; // modulo base C
  let t_steps: i64 = 16; // sequence length T
  let batch: i64 = 128;
  let epochs: i64 = 300;
  let dropout_p: f64 = 0.1;
  let lr: f64 = 1e-3;

  // ---------------- Model & Optimizer --------------
  let mut vs = nn::VarStore::new(device);
  let root = &vs.root();
  let model = SumModTransformer::new(
    root, vocab, d_model, n_heads, d_ff, n_layers, n_classes, t_steps, dropout_p, device,
  );
  let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

  // ---------------- Training Loop ------------------
  for epoch in 1 ..= epochs {
    // Sample a batch of integer tokens (B,T) in [0, vocab)
    let x_idx = Tensor::randint(vocab, [batch, t_steps], (Kind::Int64, device));

    // Labels: y = (sum_t x_t) mod C
    // (sum over dim=1) -> float -> remainder(C) -> int64
    let y = x_idx
      .to_kind(Kind::Float)
      .sum_dim_intlist([1].as_slice(), false, Kind::Float)
      .remainder(n_classes as f64)
      .to_kind(Kind::Int64); // (B)

    // Forward + loss
    let logits = model.forward_t(&x_idx, true); // (B,C)
    let loss = logits.cross_entropy_for_logits(&y);

    // Backprop + step
    opt.backward_step(&loss);

    if epoch % 10 == 0 || epoch == 1 {
      let acc = accuracy_from_logits(&logits, &y);
      let l = loss.to_device(Device::Cpu).double_value(&[]);
      println!(
        "epoch {:4} | loss {:6.4} | acc {:5.1}%",
        epoch,
        l,
        acc * 100.0
      );
    }
  }

  // ---------------- Quick Sanity Check -------------
  let test_b: i64 = 8;
  let x_idx = Tensor::randint(vocab, [test_b, t_steps], (Kind::Int64, device));
  let y = x_idx
    .to_kind(Kind::Float)
    .sum_dim_intlist([1].as_slice(), false, Kind::Float)
    .remainder(n_classes as f64)
    .to_kind(Kind::Int64); // (B)

  let logits = model.forward_t(&x_idx, false);
  let pred = logits.argmax(-1, false);
  let y_vec: Vec<i64> = y.to_device(Device::Cpu).try_into().unwrap();
  let pred_vec: Vec<i64> = pred.to_device(Device::Cpu).try_into().unwrap();
  println!("true  labels: {:?}", y_vec);
  println!("pred  labels: {:?}", pred_vec);
  Ok(())
}
