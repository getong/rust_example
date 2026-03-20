use tch::{
  Device, Kind, Tensor, nn,
  nn::{Module, OptimizerConfig},
};

// ---------------- Positional Encoding (sin/cos) ----------------
fn sinusoidal_pe(t_steps: i64, d_model: i64, device: Device) -> Tensor {
  assert!(d_model % 2 == 0, "d_model must be even");
  let pos = Tensor::arange(t_steps, (Kind::Float, device)).unsqueeze(1); // (T,1)
  let i = Tensor::arange(d_model / 2, (Kind::Float, device)); // (d/2)
  let inv_freq = ((-10000.0_f64.ln() * 2.0 / d_model as f64) as f32 * &i).exp(); // (d/2)
  let angles = &pos * inv_freq.unsqueeze(0); // (T, d/2)
  Tensor::cat(&[angles.sin(), angles.cos()], 1).unsqueeze(0) // (1,T,d)
}

// ---------------- Minimal Multi-Head Self-Attention -----------
struct MHSA {
  w_q: nn::Linear,
  w_k: nn::Linear,
  w_v: nn::Linear,
  w_o: nn::Linear,
  n_heads: i64,
  d_head: i64,
}

impl MHSA {
  fn new(vs: &nn::Path, d_model: i64, n_heads: i64) -> Self {
    assert!(d_model % n_heads == 0);
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
      d_head: d_model / n_heads,
    }
  }

  // xs: (B,T,d) -> (B,T,d)
  fn forward(&self, xs: &Tensor, train: bool) -> Tensor {
    let (b, t, d) = (xs.size()[0], xs.size()[1], xs.size()[2]);
    let q = xs.apply(&self.w_q);
    let k = xs.apply(&self.w_k);
    let v = xs.apply(&self.w_v);

    // (B,T,d) -> (B,h,T,dH)
    let split = |x: Tensor| x.view([b, t, self.n_heads, self.d_head]).transpose(1, 2);
    let q = split(q);
    let k = split(k);
    let v = split(v);

    // attention: (B,h,T,T)
    let scale = (self.d_head as f64).sqrt();
    let scores = q.matmul(&k.transpose(-2, -1)) / scale;
    let attn = scores.softmax(-1, Kind::Float);

    // context: (B,h,T,dH)
    let ctx = attn.matmul(&v);
    // concat heads -> (B,T,d)
    let out = ctx.transpose(1, 2).contiguous().view([b, t, d]);
    out.apply_t(&self.w_o, train)
  }
}

// ---------------- Encoder Block (Pre-LN + Residual) ----------
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
    let ffn = nn::seq()
      .add(nn::linear(vs / "ff1", d_model, d_ff, Default::default()))
      .add_fn(|x| x.gelu("tanh"))
      .add(nn::linear(vs / "ff2", d_ff, d_model, Default::default()));
    Self {
      ln1,
      ln2,
      attn,
      ffn,
    }
  }

  fn forward(&self, x: &Tensor, train: bool) -> Tensor {
    let h = self.attn.forward(&x.apply(&self.ln1), train);
    let x = x + h;
    let h2 = x.apply(&self.ln2).apply_t(&self.ffn, train);
    x + h2
  }
}

// ---------------- Tiny Transformer for Sentiment -------------
struct TinyNlpTransformer {
  embed: nn::Embedding,
  block: EncoderBlock,
  ln_f: nn::LayerNorm,
  head: nn::Linear,
  d_model: i64,
  device: Device,
}

impl TinyNlpTransformer {
  fn new(vs: &nn::Path, vocab: i64, d_model: i64, n_heads: i64, d_ff: i64, device: Device) -> Self {
    let embed = nn::embedding(vs / "embed", vocab, d_model, Default::default());
    let block = EncoderBlock::new(&(vs / "enc0"), d_model, n_heads, d_ff);
    let ln_f = nn::layer_norm(
      vs / "ln_f",
      vec![d_model],
      nn::LayerNormConfig {
        eps: 1e-5,
        ..Default::default()
      },
    );
    let head = nn::linear(vs / "head", d_model, 2, Default::default()); // 2 classes
    Self {
      embed,
      block,
      ln_f,
      head,
      d_model,
      device,
    }
  }

  // x_idx: (B,T) int tokens
  fn forward(&self, x_idx: &Tensor, train: bool) -> Tensor {
    let t = x_idx.size()[1];
    let pe = sinusoidal_pe(t, self.d_model, self.device); // (1,T,d)
    let mut x = self.embed.forward(x_idx) + pe;
    x = self.block.forward(&x, train);
    let x = x
      .apply(&self.ln_f)
      .mean_dim([1].as_slice(), false, Kind::Float); // (B,d)
    x.apply(&self.head) // (B,2)
  }
}

// ---------------- A toy tokenizer/vocab ----------------------
#[derive(Default)]
struct Vocab {
  stoi: std::collections::HashMap<String, i64>,
  itos: Vec<String>,
}

impl Vocab {
  fn new(words: &[&str]) -> Self {
    let mut set = std::collections::BTreeSet::new();
    set.insert("<unk>".to_string());
    for w in words {
      set.insert(w.to_lowercase());
    }
    let itos: Vec<String> = set.into_iter().collect();
    let stoi = itos
      .iter()
      .enumerate()
      .map(|(i, s)| (s.clone(), i as i64))
      .collect();
    Self { stoi, itos }
  }

  fn encode(&self, sentence: &str, seq_len: usize) -> Vec<i64> {
    let mut ids: Vec<i64> = sentence
      .split_whitespace()
      .map(|w| self.stoi.get(&w.to_lowercase()).cloned().unwrap_or(0)) // <unk>=0
      .collect();
    ids.truncate(seq_len);
    while ids.len() < seq_len {
      ids.push(0);
    }
    ids
  }

  fn size(&self) -> i64 {
    self.itos.len() as i64
  }
}

// ---------------- Small toy dataset --------------------------
struct Example {
  x: Vec<i64>,
  y: i64,
}

fn toy_data(seq_len: usize) -> (Vocab, Vec<Example>) {
  // Tiny labeled corpus
  let pos = [
    "i love this movie",
    "this film is fantastic",
    "what a great experience",
    "absolutely wonderful and inspiring",
    "i really liked it",
  ];
  let neg = [
    "i hate this movie",
    "this film is terrible",
    "what a bad experience",
    "absolutely awful and boring",
    "i really disliked it",
  ];

  // Build vocab from all words
  let words: Vec<&str> = pos
    .iter()
    .chain(neg.iter())
    .flat_map(|s| s.split_whitespace())
    .collect();
  let vocab = Vocab::new(&words);

  // Encode to (ids, label)
  let mut data = Vec::new();
  for s in pos {
    data.push(Example {
      x: vocab.encode(s, seq_len),
      y: 1,
    });
  }
  for s in neg {
    data.push(Example {
      x: vocab.encode(s, seq_len),
      y: 0,
    });
  }

  (vocab, data)
}

// ---------------- Utility: accuracy --------------------------
fn accuracy(logits: &Tensor, y: &Tensor) -> f64 {
  let pred = logits.argmax(-1, false);
  pred
    .eq_tensor(y)
    .to_kind(Kind::Float)
    .mean(Kind::Float)
    .double_value(&[])
}

// ---------------- Main: train & test -------------------------
fn main() -> tch::Result<()> {
  tch::manual_seed(42);
  let device = Device::Cpu; // keep CPU for simplicity

  // Hyperparams
  let seq_len = 8usize;
  let d_model = 64i64;
  let n_heads = 4i64;
  let d_ff = 128i64;
  let epochs = 300i64;
  let batch_size = 4i64;
  let lr = 1e-3f64;

  // Data
  let (vocab, dataset) = toy_data(seq_len);
  let n = dataset.len() as i64;

  // Model
  let mut vs = nn::VarStore::new(device);
  let root = &vs.root();
  let model = TinyNlpTransformer::new(root, vocab.size(), d_model, n_heads, d_ff, device);
  let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

  // Training loop (full-batch or tiny-batch on tiny dataset)
  for epoch in 1 ..= epochs {
    // simple mini-batch sampling
    let mut loss_epoch = 0.0;
    let mut acc_epoch = 0.0;
    let mut i0 = 0;

    while i0 < n {
      let i1 = (i0 + batch_size).min(n);
      let batch = &dataset[i0 as usize .. i1 as usize];
      let x: Vec<i64> = batch.iter().flat_map(|ex| ex.x.clone()).collect();
      let y: Vec<i64> = batch.iter().map(|ex| ex.y).collect();

      let xs = Tensor::from_slice(&x)
        .to(device)
        .view([i1 - i0, seq_len as i64]);
      let ys = Tensor::from_slice(&y).to(device);

      let logits = model.forward(&xs, true);
      let loss = logits.cross_entropy_for_logits(&ys);
      let acc = accuracy(&logits, &ys);

      opt.backward_step(&loss);

      loss_epoch += loss.double_value(&[]);
      acc_epoch += acc * (i1 - i0) as f64;

      i0 = i1;
    }

    if epoch % 20 == 0 || epoch == 1 {
      println!(
        "epoch {:4} | loss {:.4} | acc {:.1}%",
        epoch,
        loss_epoch,
        100.0 * acc_epoch / (n as f64)
      );
    }
  }

  // Quick test on a few sentences
  let test_sentences = [
    "i really love this fantastic movie",
    "this film is bad and boring",
    "great and inspiring experience",
    "absolutely terrible",
    "i disliked this",
    "i liked this wonderful film",
  ];

  println!("\n--- quick test ---");
  for s in test_sentences {
    let ids = vocab.encode(s, seq_len);
    let xs = Tensor::from_slice(&ids)
      .to(device)
      .view([1, seq_len as i64]);
    let logits = model.forward(&xs, false);
    let prob = logits.softmax(-1, Kind::Float);
    let cls = prob.argmax(-1, false).int64_value(&[]);
    let p_pos = prob.double_value(&[0, 1]);
    println!("{:45} -> class={} (p_pos={:.3})", s, cls, p_pos);
  }

  Ok(())
}
