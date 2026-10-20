use burn::{
  Tensor,
  config::Config,
  module::{Module, Param},
  nn::{Linear, LinearConfig},
  tensor::{
    Bool, Device, Int, activation::silu, module::attention, ops::AttentionModuleOptions, s,
  },
};

fn rotate_half(x: Tensor<4>) -> Tensor<4> {
  // x: (b, num_heads, seq_len, head_dim)
  let head_dim = x.dims()[3];
  let x1 = x.clone().slice(s![.., .., .., 0..head_dim / 2]);
  let x2 = x.slice(s![.., .., .., head_dim / 2..]);
  Tensor::cat(vec![-x2, x1], 3)
}

fn apply_rotary_pos_emb(
  q: Tensor<4>,
  k: Tensor<4>,
  cos: Tensor<3>,
  sin: Tensor<3>,
) -> (Tensor<4>, Tensor<4>) {
  // q & k: (b, num_heads, seq_len, head_dim)
  // cos & sin: (b, seq_len, head_dim)
  let cos = cos.unsqueeze_dim::<4>(1); // (b, 1, seq_len, head_dim)
  let sin = sin.unsqueeze_dim::<4>(1); // (b, 1, seq_len, head_dim)
  let q_embed = (q.clone() * cos.clone()) + (rotate_half(q) * sin.clone());
  let k_embed = (k.clone() * cos) + (rotate_half(k) * sin);
  (q_embed, k_embed)
}

#[derive(Module, Debug)]
pub struct Qwen3VLTextRotaryEmbedding {
  inv_freqs: Tensor<1>,
  #[module(skip)]
  mrope_section: [usize; 3],
}

impl Qwen3VLTextRotaryEmbedding {
  pub fn recomposition_frequencies(&self, freq: Tensor<4>) -> Tensor<3> {
    // freq: (3, b, seq_len, head_dim / 2)
    let mut freqs_thw = freq.clone().slice(s![0..1, .., .., ..]).squeeze_dim::<3>(0); // (b, seq_len, head_dim / 2)
    for dim in 1..=2 {
      let offset = dim;
      let length = self.mrope_section[dim] * 3;

      let chunk = freq
        .clone()
        .slice(s![dim..dim + 1, .., .., offset..length;3])
        .squeeze_dim::<3>(0); // (b, seq_len, head_dim / 2)
      freqs_thw = freqs_thw.slice_assign(s![.., .., offset..length; 3], chunk);
    }
    Tensor::cat(vec![freqs_thw.clone(), freqs_thw], 2) // (b, seq_len, head_dim)
  }

  pub fn forward(&self, position_ids: Tensor<3, Int>) -> (Tensor<3>, Tensor<3>) {
    // x: (b, seq_len, hidden_size)
    // position_ids: (3, b, seq_len)
    let position_ids = position_ids.unsqueeze_dim::<4>(3).float(); // (3, b, seq_len, 1)

    let freqs = position_ids * self.inv_freqs.clone().unsqueeze_dims(&[0, 1, 2]);
    let cos = freqs.clone().cos();
    let sin = freqs.sin();

    let cos = self.recomposition_frequencies(cos);
    let sin = self.recomposition_frequencies(sin);

    (cos, sin)
  }
}

#[derive(Config, Debug)]
pub struct Qwen3VLTextRotaryEmbeddingConfig {
  rope_base: f32,
  head_dim: usize,
  mrope_section: [usize; 3],
}

impl Qwen3VLTextRotaryEmbeddingConfig {
  pub fn init(&self, device: &Device) -> Qwen3VLTextRotaryEmbedding {
    let inv_freqs = self.compute_axial_rope_parameters(device);
    Qwen3VLTextRotaryEmbedding {
      inv_freqs,
      mrope_section: self.mrope_section,
    }
  }

  fn compute_axial_rope_parameters(&self, device: &Device) -> Tensor<1> {
    let dim = self.head_dim;
    let arange = Tensor::arange_step(0..dim as i64, 2, device).float();
    // 1/(base^x) = base^(-x) = exp(-x * ln(base))
    // x = arange / spatial_dim
    let inv_freq = (arange / (dim as f32) * -self.rope_base.ln()).exp();
    inv_freq
  }
}

#[derive(Module, Debug)]
pub struct Qwen3VLTextRMSNorm {
  weight: Param<Tensor<1>>,
  variance_epsilon: f32,
}

impl Qwen3VLTextRMSNorm {
  pub fn forward<const D: usize>(&self, hidden_states: Tensor<D>) -> Tensor<D> {
    // hidden_states: (b, seq_len, hidden_size) or (b, seq_len, num_heads, head_dim)
    let variance = hidden_states.clone().square().mean_dim(D - 1);
    let hidden_states = hidden_states * (variance + self.variance_epsilon).sqrt().recip();
    let mut weight_shape = [1; D];
    weight_shape[D - 1] = self.weight.dims()[0]; // If D = 4, (1, 1, 1, D)
    let weight = self.weight.val().reshape(weight_shape);
    weight * hidden_states
  }
}

#[derive(Config, Debug)]
pub struct Qwen3VLTextRMSNormConfig {
  hidden_size: usize,
  eps: f32,
}

impl Qwen3VLTextRMSNormConfig {
  pub fn init(&self, device: &Device) -> Qwen3VLTextRMSNorm {
    Qwen3VLTextRMSNorm {
      weight: Param::from_tensor(Tensor::ones([self.hidden_size], device)),
      variance_epsilon: self.eps,
    }
  }
}

fn repeat_kv(kv: Tensor<4>, repeat: usize) -> Tensor<4> {
  // Repeats key and values repeat times
  // kv: (B, num_kv_heads, seq_len, head_dim)
  let [b, num_kv_heads, seq_len, head_dim] = kv.dims();
  let kv = kv.unsqueeze_dim::<5>(2); // (B, num_kv_heads, 1, seq_len, head_dim)

  let kv = kv.repeat_dim(2, repeat); // (B, num_kv_heads, repeat, seq_len, head_dim)
  kv.reshape([b, num_kv_heads * repeat, seq_len, head_dim])
}

#[derive(Module, Debug)]
pub struct Qwen3VLTextAttention {
  q_proj: Linear,
  k_proj: Linear,
  v_proj: Linear,
  o_proj: Linear,
  q_norm: Qwen3VLTextRMSNorm,
  k_norm: Qwen3VLTextRMSNorm,
  head_dim: usize,
  num_heads: usize,
  num_kv_heads: usize,
}

impl Qwen3VLTextAttention {
  pub fn forward(
    &self,
    hidden_states: Tensor<3>,
    position_embeddings: (Tensor<3>, Tensor<3>),
    attention_mask: Option<Tensor<3, Bool>>,
  ) -> Tensor<3> {
    // hidden_states: (b, seq_len, hidden_size)
    // position_embeddings are sin and cos with (b, seq_len, head_dim)
    // attention_mask: (b, seq_len, seq_len)

    let [b, seq_len, hidden_size] = hidden_states.dims();
    let query = self.q_proj.forward(hidden_states.clone());
    let key = self.k_proj.forward(hidden_states.clone());
    let value = self.v_proj.forward(hidden_states.clone());

    let head_dim = self.head_dim;
    let num_heads = self.num_heads;
    let num_kv_heads = self.num_kv_heads;

    let query = query.reshape([b, seq_len, num_heads, head_dim]);
    let key = key.reshape([b, seq_len, num_kv_heads, head_dim]);
    let value = value.reshape([b, seq_len, num_kv_heads, head_dim]);
    let query = self.q_norm.forward(query).swap_dims(1, 2); // (B, num_heads, seq_len, head_dim)
    let key = self.k_norm.forward(key).swap_dims(1, 2); // (B, num_kv_heads, seq_len, head_dim)
    let value = value.swap_dims(1, 2); // (B, num_kv_heads, seq_len, head_dim)

    let repeat = num_heads / num_kv_heads; // 4
    let key = repeat_kv(key, repeat); // (B, num_heads, seq_len, head_dim)
    let value = repeat_kv(value, repeat); // (B, num_heads, seq_len, head_dim)

    let (cos, sin) = position_embeddings;
    let (query_states, key_states) = apply_rotary_pos_emb(query, key, cos, sin);

    // expects (b, num_heads, seq_len, head_dim)
    // Mask should be (b, 1, seq_len, seq_len)
    let attention_mask = attention_mask.map(|m| m.unsqueeze_dim::<4>(1));
    let out = attention(
      query_states,
      key_states,
      value,
      attention_mask,
      None,
      AttentionModuleOptions::default(),
    );
    let out = out.swap_dims(1, 2); // (B, seq_len, num_heads, head_dim)
    let out = out.reshape([b, seq_len, hidden_size]); // (B, seq_len, hidden_size)
    self.o_proj.forward(out)
  }
}

// attention_bias = false
#[derive(Config, Debug)]
pub struct Qwen3VLTextAttentionConfig {
  num_heads: usize,
  num_kv_heads: usize,
  head_dim: usize,
  hidden_size: usize,
  rms_norm_eps: f32,
}

impl Qwen3VLTextAttentionConfig {
  pub fn init(&self, device: &Device) -> Qwen3VLTextAttention {
    Qwen3VLTextAttention {
      q_proj: LinearConfig::new(self.hidden_size, self.num_heads * self.head_dim)
        .with_bias(false)
        .init(device),
      k_proj: LinearConfig::new(self.hidden_size, self.num_kv_heads * self.head_dim)
        .with_bias(false)
        .init(device),
      v_proj: LinearConfig::new(self.hidden_size, self.num_kv_heads * self.head_dim)
        .with_bias(false)
        .init(device),
      o_proj: LinearConfig::new(self.num_heads * self.head_dim, self.hidden_size)
        .with_bias(false)
        .init(device),
      q_norm: Qwen3VLTextRMSNormConfig::new(self.head_dim, self.rms_norm_eps).init(device),
      k_norm: Qwen3VLTextRMSNormConfig::new(self.head_dim, self.rms_norm_eps).init(device),
      num_heads: self.num_heads,
      num_kv_heads: self.num_kv_heads,
      head_dim: self.head_dim,
    }
  }
}

#[derive(Module, Debug)]
pub struct Qwen3VLTextMLP {
  gate_proj: Linear,
  up_proj: Linear,
  down_proj: Linear,
}

impl Qwen3VLTextMLP {
  pub fn forward(&self, x: Tensor<3>) -> Tensor<3> {
    let gate = silu(self.gate_proj.forward(x.clone()));
    let up = self.up_proj.forward(x);
    self.down_proj.forward(gate * up)
  }
}

#[derive(Config, Debug)]
pub struct Qwen3VLTextMLPConfig {
  hidden_size: usize,
  intermediate_size: usize,
}

impl Qwen3VLTextMLPConfig {
  pub fn init(&self, device: &Device) -> Qwen3VLTextMLP {
    Qwen3VLTextMLP {
      gate_proj: LinearConfig::new(self.hidden_size, self.intermediate_size)
        .with_bias(false)
        .init(device),
      up_proj: LinearConfig::new(self.hidden_size, self.intermediate_size)
        .with_bias(false)
        .init(device),
      down_proj: LinearConfig::new(self.intermediate_size, self.hidden_size)
        .with_bias(false)
        .init(device),
    }
  }
}

#[derive(Module, Debug)]
pub struct Qwen3VLTextDecoderLayer {
  self_attn: Qwen3VLTextAttention,
  mlp: Qwen3VLTextMLP,
  input_layernorm: Qwen3VLTextRMSNorm,
  post_attention_layernorm: Qwen3VLTextRMSNorm,
}

impl Qwen3VLTextDecoderLayer {
  pub fn forward(
    &self,
    hidden_states: Tensor<3>,
    position_embeddings: (Tensor<3>, Tensor<3>),
    attention_mask: Option<Tensor<3, Bool>>,
  ) -> Tensor<3> {
    let residual = hidden_states.clone();
    let hidden_states = self.input_layernorm.forward(hidden_states);

    let hidden_states = self
      .self_attn
      .forward(hidden_states, position_embeddings, attention_mask);
    let hidden_states = residual + hidden_states;
    let residual = hidden_states.clone();
    let hidden_states = self.post_attention_layernorm.forward(hidden_states);
    let hidden_states = self.mlp.forward(hidden_states);
    residual + hidden_states
  }
}

#[derive(Config, Debug)]
pub struct Qwen3VLTextDecoderLayerConfig {
  hidden_size: usize,
  head_dim: usize,
  num_heads: usize,
  num_kv_heads: usize,
  intermediate_size: usize,
  rms_norm_eps: f32,
}

impl Qwen3VLTextDecoderLayerConfig {
  pub fn init(&self, device: &Device) -> Qwen3VLTextDecoderLayer {
    Qwen3VLTextDecoderLayer {
      self_attn: Qwen3VLTextAttentionConfig::new(
        self.num_heads,
        self.num_kv_heads,
        self.head_dim,
        self.hidden_size,
        self.rms_norm_eps,
      )
      .init(device),
      mlp: Qwen3VLTextMLPConfig::new(self.hidden_size, self.intermediate_size).init(device),
      input_layernorm: Qwen3VLTextRMSNormConfig::new(self.hidden_size, self.rms_norm_eps)
        .init(device),
      post_attention_layernorm: Qwen3VLTextRMSNormConfig::new(self.hidden_size, self.rms_norm_eps)
        .init(device),
    }
  }
}
