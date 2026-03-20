use tch::{
  Device, IndexOp, Kind, Tensor, nn,
  nn::{Module, OptimizerConfig},
}; // for .i((.., 0, ..))

/// ---------------- Clean sinusoidal positional encoding ----------------
/// PE[pos, 2i]   = sin(pos / 10000^(2i/d_model))
/// PE[pos, 2i+1] = cos(pos / 10000^(2i/d_model))
fn sinusoidal_positional_encoding(t_steps: i64, d_model: i64, device: Device) -> Tensor {
  assert!(d_model % 2 == 0, "d_model must be even for this PE");
  let pos = Tensor::arange(t_steps, (Kind::Float, device)).unsqueeze(1); // (T,1)
  let i = Tensor::arange(d_model / 2, (Kind::Float, device)); // (D/2)

  // inv_freq = 10000^{-2i/d_model} = exp(-ln(10000) * 2i/d_model)
  let inv_freq = ((-10000.0_f64.ln() * 2.0 / d_model as f64) as f32 * &i).exp(); // (D/2)

  let angles = &pos * inv_freq.unsqueeze(0); // (T, D/2)
  let sin = angles.sin();
  let cos = angles.cos();

  Tensor::cat(&[sin, cos], 1).unsqueeze(0) // (1,T,D)
}

/// ---------------- Multi-Head Self Attention ----------------
struct MHSA {
  w_q: nn::Linear,
  w_k: nn::Linear,
  w_v: nn::Linear,
  w_o: nn::Linear,
  n_heads: i64,
  d_model: i64,
  d_head: i64,
}

impl MHSA {
  fn new(vs: &nn::Path, d_model: i64, n_heads: i64) -> Self {
    assert!(
      d_model % n_heads == 0,
      "d_model must be divisible by n_heads"
    );
    let d_head = d_model / n_heads;
    let cfg = nn::LinearConfig {
      bias: true,
      ..Default::default()
    };
    let w_q = nn::linear(vs / "w_q", d_model, d_model, cfg);
    let w_k = nn::linear(vs / "w_k", d_model, d_model, cfg);
    let w_v = nn::linear(vs / "w_v", d_model, d_model, cfg);
    let w_o = nn::linear(vs / "w_o", d_model, d_model, cfg);
    Self {
      w_q,
      w_k,
      w_v,
      w_o,
      n_heads,
      d_model,
      d_head,
    }
  }

  /// xs: (B,T,D) -> (B,T,D)
  fn forward_t(&self, xs: &Tensor, _train: bool) -> Tensor {
    let (b, t, _) = (xs.size()[0], xs.size()[1], xs.size()[2]);

    let q = xs.apply(&self.w_q);
    let k = xs.apply(&self.w_k);
    let v = xs.apply(&self.w_v);

    // reshape to heads: (B,H,T,Dh)
    let q = q.view([b, t, self.n_heads, self.d_head]).transpose(1, 2);
    let k = k.view([b, t, self.n_heads, self.d_head]).transpose(1, 2);
    let v = v.view([b, t, self.n_heads, self.d_head]).transpose(1, 2);

    // scaled dot-product attention
    let scale = (self.d_head as f64).sqrt();
    let attn = (q.matmul(&k.transpose(-2, -1)) / scale).softmax(-1, Kind::Float); // (B,H,T,T)
    let ctx = attn.matmul(&v); // (B,H,T,Dh)

    // concat heads -> (B,T,D)
    let ctx = ctx.transpose(1, 2).contiguous().view([b, t, self.d_model]);
    ctx.apply(&self.w_o)
  }
}

/// ---------------- Position-wise Feed-Forward ----------------
fn build_ffn(vs: &nn::Path, d_model: i64, d_ff: i64) -> nn::Sequential {
  nn::seq()
    .add(nn::linear(vs / "ff1", d_model, d_ff, Default::default()))
    .add_fn(|x| x.gelu("tanh")) // gelu in tch 0.18 needs an &str
    .add(nn::linear(vs / "ff2", d_ff, d_model, Default::default()))
}

/// ---------------- Transformer Encoder Block ----------------
struct EncoderBlock {
  ln1: nn::LayerNorm,
  ln2: nn::LayerNorm,
  attn: MHSA,
  ffn: nn::Sequential,
}

impl EncoderBlock {
  fn new(vs: &nn::Path, d_model: i64, n_heads: i64, d_ff: i64) -> Self {
    let ln_cfg = nn::LayerNormConfig {
      eps: 1e-5,
      ..Default::default()
    };
    let ln1 = nn::layer_norm(vs / "ln1", vec![d_model], ln_cfg);
    let ln2 = nn::layer_norm(vs / "ln2", vec![d_model], ln_cfg);
    let attn = MHSA::new(&(vs / "attn"), d_model, n_heads);
    let ffn = build_ffn(&(vs / "ffn"), d_model, d_ff);
    Self {
      ln1,
      ln2,
      attn,
      ffn,
    }
  }

  /// x: (B,T,D)
  fn forward_t(&self, x: &Tensor, train: bool) -> Tensor {
    let h = x.apply_t(&self.ln1, train);
    let h = self.attn.forward_t(&h, train);
    let x = x + h; // residual 1

    let h2 = x.apply_t(&self.ln2, train);
    let h2 = h2.apply_t(&self.ffn, train);
    x + h2 // residual 2
  }
}

fn main() {
  tch::manual_seed(42);
  let device = Device::cuda_if_available();
  println!("Device: {:?}", device);

  // ---------------- Hyperparameters ----------------
  let vocab: i64 = 10; // digits 0..9
  let d_model: i64 = 128;
  let n_heads: i64 = 8; // head dim = 16
  let d_ff: i64 = 256;
  let t_steps: i64 = 16; // sequence length (not counting [CLS])
  let n_classes: i64 = 5; // sum mod 5
  let batch: i64 = 128;

  let epochs: i64 = 60;
  let steps_per_epoch: i64 = 200; // many batches per epoch

  // ---------------- Model ----------------
  let vs = nn::VarStore::new(device);
  let root = &vs.root();

  let embed = nn::embedding(root / "embed", vocab, d_model, Default::default());
  let enc = EncoderBlock::new(&(root / "enc1"), d_model, n_heads, d_ff);
  let head = nn::linear(root / "head", d_model, n_classes, Default::default());

  // Learnable [CLS] token
  let cls = root.randn("cls", &[1, 1, d_model], 0.0, 0.02); // (1,1,D)

  // Positional enc for sequence length INCLUDING [CLS]
  let pe = sinusoidal_positional_encoding(t_steps + 1, d_model, device); // (1,T+1,D)

  let mut opt = nn::Adam::default().build(&vs, 1e-3).unwrap();

  for epoch in 1 ..= epochs {
    for _ in 0 .. steps_per_epoch {
      // (B,T) with integers in 0..9
      let x_idx = Tensor::randint(vocab, [batch, t_steps], (Kind::Int64, device));

      // y = sum over time mod n_classes
      let y = x_idx
        .to_kind(Kind::Float)
        .sum_dim_intlist([1].as_slice(), false, Kind::Float)
        .remainder(n_classes as f64)
        .to_kind(Kind::Int64); // (B)

      // Embed + prepend CLS + add PE
      let tok = embed.forward(&x_idx); // (B,T,D)
      let cls_rep = cls.expand([batch, 1, d_model], true); // (B,1,D)
      let x = Tensor::cat(&[cls_rep, tok], 1) + &pe; // (B,T+1,D)

      // Encoder -> take CLS (position 0)
      let h = enc.forward_t(&x, true); // (B,T+1,D)
      let cls_h = h.i((.., 0, ..)); // (B,D)

      let logits = cls_h.apply(&head); // (B,C)
      let loss = logits.cross_entropy_for_logits(&y);
      opt.backward_step(&loss);
    }

    // quick monitor
    let x_idx = Tensor::randint(vocab, [batch, t_steps], (Kind::Int64, device));
    let y = x_idx
      .to_kind(Kind::Float)
      .sum_dim_intlist([1].as_slice(), false, Kind::Float)
      .remainder(n_classes as f64)
      .to_kind(Kind::Int64);

    let tok = embed.forward(&x_idx);
    let cls_rep = cls.expand([batch, 1, d_model], true);
    let x = Tensor::cat(&[cls_rep, tok], 1) + &pe;

    let h = enc.forward_t(&x, false);
    let cls_h = h.i((.., 0, ..));
    let logits = cls_h.apply(&head);
    let loss = logits.cross_entropy_for_logits(&y);

    let preds = logits.argmax(-1, false);
    let acc = preds
      .eq_tensor(&y)
      .to_kind(Kind::Float)
      .mean(Kind::Float)
      .to_device(Device::Cpu)
      .double_value(&[])
      * 100.0;
    let l = loss.to_device(Device::Cpu).double_value(&[]);
    println!("epoch {:3} | loss {:.4} | acc {:5.1}%", epoch, l, acc);
  }

  // --------- Quick sanity eval on 8 sequences ----------
  let x_idx = Tensor::randint(vocab, [8, t_steps], (Kind::Int64, device));
  let y = x_idx
    .to_kind(Kind::Float)
    .sum_dim_intlist([1].as_slice(), false, Kind::Float)
    .remainder(n_classes as f64)
    .to_kind(Kind::Int64);

  let tok = embed.forward(&x_idx);
  let cls_rep = cls.expand([8, 1, d_model], true);
  let x = Tensor::cat(&[cls_rep, tok], 1) + &pe;

  let h = enc.forward_t(&x, false);
  let cls_h = h.i((.., 0, ..));
  let logits = cls_h.apply(&head);
  let preds = logits.argmax(-1, false).to_device(Device::Cpu);

  let true_vec: Vec<i64> = y.to(Device::Cpu).try_into().unwrap();
  let pred_vec: Vec<i64> = preds.try_into().unwrap();

  println!("true  labels: {:?}", true_vec);
  println!("pred  labels: {:?}", pred_vec);
}
