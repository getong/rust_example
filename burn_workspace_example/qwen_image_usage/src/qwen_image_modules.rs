use burn::{
  Tensor,
  config::Config,
  module::{Module, Param},
  nn::{Linear, LinearConfig},
  tensor::{
    Bool, Device, FloatDType, Int,
    activation::{gelu_approximate, silu},
    module::attention,
    ops::AttentionModuleOptions,
    s,
  },
};

use crate::normalization::{RMSNorm, RMSNormConfig};

// use_real_unbind_dim = -1
fn apply_rotary_emb_qwen(x: Tensor<4>, freqs_cis: (Tensor<2>, Tensor<2>)) -> Tensor<4> {
  // x: (Batch, Sequence, Heads, Dimension)
  // cos: (S, D)
  // sin: (S, D)
  let initial_dtype = x.dtype();
  let x = x.cast(FloatDType::F32);
  let [b, s, h, d] = x.dims();
  let (cos, sin) = freqs_cis;
  let cos = cos.unsqueeze_dim::<3>(0).unsqueeze_dim::<4>(2); // (1, S, D) then (1, S, 1, D)
  let sin = sin.unsqueeze_dim::<3>(0).unsqueeze_dim::<4>(2); // (1, S, D) then (1, S, 1, D)

  let x_reshaped = x.clone().reshape([b, s, h, d / 2, 2]); // (Assuming d is exactly divisible by 2)
  let x_real = x_reshaped.clone().slice_dim(4, 0).squeeze_dim::<4>(4); // (B, S, H, D/2)
  let x_imag = x_reshaped.clone().slice_dim(4, 1).squeeze_dim::<4>(4); // (B, S, H, D/2)
  let x_rotated = Tensor::stack::<5>(vec![-x_imag, x_real], 4); // (B, S, H, D/2, 2)
  let x_rotated = x_rotated.reshape([b, s, h, d]);

  let out = x * cos + x_rotated * sin; // (B, S, H, D)
  out.cast(initial_dtype)
}

// timestep_dim = 256
// max_period = 10_000
// time_factor = 1_000.0
#[derive(Module, Debug)]
struct QwenImageTemporalTimesteps {
  // (half,) or (128,)
  freqs: Tensor<1>,
  time_factor: f64,
}

impl QwenImageTemporalTimesteps {
  fn forward(&self, timestep: Tensor<1>) -> Tensor<2> {
    // timestep: (B,)
    let initial_dtype = timestep.dtype();
    let timestep = (self.time_factor * timestep).cast(FloatDType::F32);
    let timestep = timestep.unsqueeze_dim::<2>(1); // (B, 1)
    let freqs = self.freqs.clone().unsqueeze_dim::<2>(0); // (1, half)
    let args = timestep * freqs;
    let cos = args.clone().cos(); // (B, half)
    let sin = args.clone().sin(); // (B, half)
    let out = Tensor::cat(vec![cos, sin], 1); // (B, timestep_dim) as half * 2 = timestep_dim
    out.cast(initial_dtype)
  }
}

#[derive(Config, Debug)]
struct QwenImageTemporalTimestepsConfig {
  timestep_dim: usize,
  #[config(default = 10_000)]
  max_period: usize,
  #[config(default = 1_000.0)]
  time_factor: f64,
}

impl QwenImageTemporalTimestepsConfig {
  fn init(&self, device: &Device) -> QwenImageTemporalTimesteps {
    let half = (self.timestep_dim / 2) as f64;
    let freqs = -(self.max_period as f64).ln()
      * Tensor::arange(0..half as i64, device).cast(FloatDType::F32)
      / half;
    let freqs = freqs.exp();
    QwenImageTemporalTimesteps {
      freqs,
      time_factor: self.time_factor,
    }
  }
}

// in_channels = 256
// time_embed_dim = num_attention_head * attention_head_dim
// sample_proj_bias = False
// act_fn = silu
// out_dim = None
// post_act_fn = None
// cond_proj_dim = None
#[derive(Module, Debug)]
struct TimestepEmbedding {
  linear_1: Linear,
  linear_2: Linear,
}

impl TimestepEmbedding {
  fn forward(&self, mut sample: Tensor<2>) -> Tensor<2> {
    sample = self.linear_1.forward(sample);
    sample = silu(sample);
    self.linear_2.forward(sample)
  }
}

#[derive(Config, Debug)]
struct TimestepEmbeddingConfig {
  in_channels: usize,
  time_embed_dim: usize,
}

impl TimestepEmbeddingConfig {
  fn init(&self, device: &Device) -> TimestepEmbedding {
    let time_embed_dim_out = self.time_embed_dim;
    TimestepEmbedding {
      linear_1: LinearConfig::new(self.in_channels, self.time_embed_dim)
        .with_bias(false)
        .init(device),
      linear_2: LinearConfig::new(self.time_embed_dim, time_embed_dim_out)
        .with_bias(false)
        .init(device),
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageTimestepProjEmbeddings {
  time_proj: QwenImageTemporalTimesteps,
  timestep_embedder: TimestepEmbedding,
}

impl QwenImageTimestepProjEmbeddings {
  pub fn forward(&self, timestep: Tensor<1>) -> Tensor<2> {
    let timesteps_proj = self.time_proj.forward(timestep);
    self.timestep_embedder.forward(timesteps_proj)
  }
}

#[derive(Config, Debug)]
pub struct QwenImageTimestepProjEmbeddingsConfig {
  embedding_dim: usize,
}

impl QwenImageTimestepProjEmbeddingsConfig {
  pub fn init(&self, device: &Device) -> QwenImageTimestepProjEmbeddings {
    QwenImageTimestepProjEmbeddings {
      time_proj: QwenImageTemporalTimestepsConfig::new(256).init(device),
      timestep_embedder: TimestepEmbeddingConfig::new(256, self.embedding_dim).init(device),
    }
  }
}

#[derive(Module, Debug)]
struct QwenImageZeroCenterRMSNorm {
  // (dim,)
  weight: Param<Tensor<1>>,
  eps: f64,
}

impl QwenImageZeroCenterRMSNorm {
  fn forward(&self, hidden_states: Tensor<3>) -> Tensor<3> {
    // hidden_states: (B, S, D)

    let initial_dtype = hidden_states.dtype();
    let hidden_states = hidden_states.cast(FloatDType::F32);
    let arg = hidden_states.clone().powf_scalar(2.0).mean_dim(2) + self.eps;
    let rrms = arg.sqrt().recip();

    let weight = self
      .weight
      .val()
      // .cast(FloatDType::F32) // Cast to F32 when using BF16
      .unsqueeze_dim::<2>(0)
      .unsqueeze_dim::<3>(0)
      + 1.0; // (1, dim)
    let out = hidden_states * rrms * weight;
    out.cast(initial_dtype)
  }
}

#[derive(Config, Debug)]
struct QwenImageZeroCenterRMSNormConfig {
  dim: usize,
  eps: f64,
}

impl QwenImageZeroCenterRMSNormConfig {
  fn init(&self, device: &Device) -> QwenImageZeroCenterRMSNorm {
    QwenImageZeroCenterRMSNorm {
      weight: Param::from_tensor(Tensor::zeros([self.dim], device)),
      eps: self.eps,
    }
  }
}

// act = GELU(approximate=tanh)
#[derive(Module, Debug)]
pub struct QwenImageTextProjection {
  text_norm: QwenImageZeroCenterRMSNorm,
  in_layer: Linear,
  out_layer: Linear,
}

impl QwenImageTextProjection {
  pub fn forward(&self, hidden_states: Tensor<3>) -> Tensor<3> {
    let hidden_states = self.text_norm.forward(hidden_states);
    let hidden_states = self.in_layer.forward(hidden_states);
    let hidden_states = gelu_approximate(hidden_states);
    self.out_layer.forward(hidden_states)
  }
}

#[derive(Config, Debug)]
pub struct QwenImageTextProjectionConfig {
  context_in_dim: usize,
  hidden_size: usize,
  eps: f64,
}

impl QwenImageTextProjectionConfig {
  pub fn init(&self, device: &Device) -> QwenImageTextProjection {
    QwenImageTextProjection {
      text_norm: QwenImageZeroCenterRMSNormConfig::new(self.context_in_dim, self.eps).init(device),
      in_layer: LinearConfig::new(self.context_in_dim, self.hidden_size)
        .with_bias(false)
        .init(device),
      out_layer: LinearConfig::new(self.hidden_size, self.hidden_size)
        .with_bias(false)
        .init(device),
    }
  }
}

#[derive(Module, Debug)]
struct QwenImageSwiGLUFeedForward {
  proj: Linear,
  out: Linear,
  gate_layer: Linear,
}

impl QwenImageSwiGLUFeedForward {
  // Doing compute in F16
  fn forward(&self, hidden_states: Tensor<3>) -> Tensor<3> {
    // let hidden_states = hidden_states.cast(FloatDType::F16);
    let gate_out = silu(self.gate_layer.forward(hidden_states.clone()));
    let other = self.proj.forward(hidden_states);

    self.out.forward(gate_out * other) //.cast(FloatDType::F32)
  }
}

#[derive(Config, Debug)]
struct QwenImageSwiGLUFeedForwardConfig {
  hidden_size: usize,
  mlp_hidden_size: usize,
}

impl QwenImageSwiGLUFeedForwardConfig {
  fn init(&self, device: &Device) -> QwenImageSwiGLUFeedForward {
    QwenImageSwiGLUFeedForward {
      proj: LinearConfig::new(self.hidden_size, self.mlp_hidden_size)
        .with_bias(false)
        .init(device),
      out: LinearConfig::new(self.mlp_hidden_size, self.hidden_size)
        .with_bias(false)
        .init(device),
      gate_layer: LinearConfig::new(self.hidden_size, self.mlp_hidden_size)
        .with_bias(false)
        .init(device),
    }
  }
}

#[derive(Module, Debug)]
struct QwenLayerNorm {
  eps: f64,
}

impl QwenLayerNorm {
  fn forward(&self, hidden_states: Tensor<3>) -> Tensor<3> {
    // hidden_states: (B, S, D)
    let initial_dtype = hidden_states.dtype();
    let hidden_states = hidden_states.cast(FloatDType::F32);
    let mean = hidden_states.clone().mean_dim(2); // (B, S, 1)
    let diff = hidden_states.clone() - mean; // (B, S, D)
    let var = diff.clone().powf_scalar(2.0).mean_dim(2); // (B, S, 1)
    let out = diff / (var + self.eps).sqrt(); // (B, S, D)
    out.cast(initial_dtype)
  }
}

#[derive(Config, Debug)]
struct QwenLayerNormConfig {
  eps: f64,
}

impl QwenLayerNormConfig {
  fn init(&self) -> QwenLayerNorm {
    QwenLayerNorm { eps: self.eps }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageAdaLayerNormContinuous {
  linear: Linear,
  norm: QwenLayerNorm, // Burn doesn't supports elementwise_affine=False in LayerNorm as of now
  eps: f64,
}

impl QwenImageAdaLayerNormContinuous {
  pub fn forward(
    &self,
    hidden_states: Tensor<3>,
    conditioning_embedding: Tensor<2>,
    target_token_mask: Tensor<1, Bool>,
  ) -> Tensor<3> {
    // hidden_states: (B, S, D)
    // conditioning_embedding: (B+1, D)
    // target_token_mask: (S)
    let normalized_hidden_states = self.norm.forward(hidden_states); // (B, S, D)

    let scale = self.linear.forward(silu(conditioning_embedding));
    let scale = select_modulation_rows(scale, Some(target_token_mask));
    let out: Tensor<3> = normalized_hidden_states * (1.0 + scale);
    out
  }
}

fn select_modulation_rows(
  params: Tensor<2>,
  target_token_mask: Option<Tensor<1, Bool>>,
) -> Tensor<3> {
  // params: (B+1, D)
  // target_token_mask: (seq_len,)
  if let Some(target_token_mask) = target_token_mask {
    let [b, _d] = params.dims();
    let real = params.clone().slice(s![0..b - 1, ..]).unsqueeze_dim::<3>(1); // (B, 1, D)
    let zero = params.slice(s![b - 1.., ..]).unsqueeze_dim::<3>(0); // (1, 1, D)
    let target_token_mask = target_token_mask.unsqueeze_dims::<3>(&[0, 2]); // (1. seq_len, 1)
    zero.mask_where(target_token_mask, real)
  } else {
    params.unsqueeze_dim::<3>(1)
  }
}

#[derive(Config, Debug)]
pub struct QwenImageAdaLayerNormContinuousConfig {
  embedding_dim: usize,
  conditioning_embedding_dim: usize,
  eps: f64,
}

impl QwenImageAdaLayerNormContinuousConfig {
  pub fn init(&self, device: &Device) -> QwenImageAdaLayerNormContinuous {
    QwenImageAdaLayerNormContinuous {
      linear: LinearConfig::new(self.conditioning_embedding_dim, self.embedding_dim)
        .with_bias(false)
        .init(device),
      norm: QwenLayerNormConfig::new(self.eps).init(),
      eps: self.eps,
    }
  }
}

pub fn qwenimage_prefix_segments(
  image_ids: &[i64],
  prefix_len: usize,
) -> Vec<(usize, usize, bool)> {
  // image_ids: (seq_len,)
  let prefix_ids = &image_ids[0..prefix_len];

  let mut segments = vec![];
  let mut start = 0;
  for index in 1..=prefix_len {
    if index == prefix_len || prefix_ids[index] != prefix_ids[start] {
      segments.push((start, index, prefix_ids[start] < 0));
      start = index;
    }
  }
  segments
}

/// Stores the K and V projections (post-RoPE) for the prefix extracted at the first denoising step.
/// K and V both are of shape: (batch_size, num_prefix_tokens, num_heads, head_dim)
#[derive(Clone)]
pub struct QwenImageKVLayerCache {
  k: Tensor<4>,
  v: Tensor<4>,
}

impl QwenImageKVLayerCache {
  fn store(&mut self, k: Tensor<4>, v: Tensor<4>) {
    self.k = k;
    self.v = v;
  }

  fn get(&self) -> (Tensor<4>, Tensor<4>) {
    (self.k.clone(), self.v.clone())
  }
}

pub struct QwenImageKVCache {
  pub layer_caches: Vec<QwenImageKVLayerCache>,
}

impl QwenImageKVCache {
  pub fn new(num_layers: usize, batch_size: usize, device: &Device) -> Self {
    let layer_caches = (0..num_layers)
      .map(|_| QwenImageKVLayerCache {
        k: Tensor::zeros([batch_size, 1, 32, 128], device),
        v: Tensor::zeros([batch_size, 1, 32, 128], device),
      })
      .collect();
    Self { layer_caches }
  }
  pub fn get_layer_mut(&mut self, layer_idx: usize) -> &mut QwenImageKVLayerCache {
    &mut self.layer_caches[layer_idx]
  }
}

#[derive(Clone)]
pub enum KvCacheMode {
  EXTRACT,
  CACHED,
}

#[derive(Module, Debug)]
struct QwenImageAttention {
  to_q: Linear,
  to_k: Linear,
  to_v: Linear,
  to_out: Vec<Linear>, // (Original ModuleList was [Linear, Dropout] but dropout isn't used in inference and we need the name mapping to work, so using Vec<Linear>)
  norm_q: RMSNorm,
  norm_k: RMSNorm,
}

fn qwen_attention(
  query: Tensor<4>,
  key: Tensor<4>,
  value: Tensor<4>,
  attention_mask: Option<Tensor<4, Bool>>,
) -> Tensor<4> {
  // query, key and value are of shape: (B, S, H, D)
  let query = query.swap_dims(1, 2); // (B, H, S, D)
  let key = key.swap_dims(1, 2); // (B, H, S, D)
  let value = value.swap_dims(1, 2); // (B, H, S, D)

  // Flash Attention
  let out = attention(
    query,
    key,
    value,
    attention_mask.map(|m| m.bool_not()), // true = masked out
    None,
    AttentionModuleOptions::default(),
  ); // (B, H, S, D)
  return out.swap_dims(1, 2); // (B, S, H, D)

  /*
  let head_dim = query.dims()[3] as f64;
  let scale = 1.0 / head_dim.sqrt();

  let query = query * scale;
  // (B, H, S, D) @ (B, H, D, S) => (B, H, S, S)
  let mut scores = query.matmul(key.transpose());

  if let Some(mask) = attention_mask {
      let neg_inf = Tensor::zeros_like(&scores).add_scalar(-1e9);
      scores = scores.mask_where(mask.bool_not(), neg_inf);
  }

  // Upcast softmax
  let initial_dtype = scores.dtype();
  let scores = scores.cast(FloatDType::F32);
  let weights = softmax(scores, 3); // (B, H, S, S)
  let weights = weights.cast(initial_dtype);

  // (B, H, S, S) @ (B, H, S, D) => (B, H, S, D)
  let context = weights.matmul(value); //.cast(initial_dtype);
  context.swap_dims(1, 2) //(B, S, H, D)
   */
}

impl QwenImageAttention {
  fn forward(
    &self,
    hidden_states: Tensor<3>,
    attention_mask: Option<Tensor<4, Bool>>,
    rotary_emb: (Tensor<2>, Tensor<2>),
    layer_cache: Option<&mut QwenImageKVLayerCache>,
    kv_cache_mode: Option<KvCacheMode>,
    prefix_len: usize,
    segments: Option<Vec<(usize, usize, bool)>>,
    key_valid: Option<Tensor<2, Bool>>,
  ) -> Tensor<3> {
    let num_attention_heads = 32;
    let head_dim = 128;
    let [b, s, _] = hidden_states.dims();
    let query = self.to_q.forward(hidden_states.clone()); // (B, S, 4096)
    let key = self.to_k.forward(hidden_states.clone()); // (B, S, 4096)
    let value = self.to_v.forward(hidden_states.clone()); // (B, S, 4096)

    let mut query = query.reshape([b, s, num_attention_heads, head_dim]);
    let mut key = key.reshape([b, s, num_attention_heads, head_dim]);
    let mut value = value.reshape([b, s, num_attention_heads, head_dim]);
    // All are of shape (B, S, 32, 128) now

    query = self.norm_q.forward(query);
    key = self.norm_k.forward(key);

    query = apply_rotary_emb_qwen(query, rotary_emb.clone());
    key = apply_rotary_emb_qwen(key, rotary_emb);
    if let (Some(layer_cache), Some(kv_cache_mode)) = (layer_cache, kv_cache_mode) {
      match kv_cache_mode {
        KvCacheMode::EXTRACT => {
          layer_cache.store(
            key.clone().slice(s![.., ..prefix_len, .., ..]),
            value.clone().slice(s![.., ..prefix_len, .., ..]),
          );
        }
        KvCacheMode::CACHED => {
          let (cached_k, cached_v) = layer_cache.get();
          key = Tensor::cat(vec![cached_k, key], 1);
          value = Tensor::cat(vec![cached_v, value], 1);
        }
      }
    }
    let seq_len_q = query.dims()[1];

    let hidden_states = if let Some(segments) = segments {
      let prefix_len = segments.last().unwrap().1;
      let mut outputs = vec![];
      for (start, end, is_text) in segments {
        let mut seg_mask = None;
        if is_text {
          let seg_len = end - start;
          seg_mask = Some(
            Tensor::cat(
              vec![
                Tensor::<2, Int>::ones([seg_len, start], &query.device()).bool(),
                Tensor::<2, Int>::ones([seg_len, seg_len], &query.device())
                  .tril(0)
                  .bool(),
              ],
              1,
            )
            .unsqueeze_dims::<4>(&[0, 1]),
          );
        }
        if let Some(key_valid) = key_valid.clone() {
          let mut seg_key_valid = key_valid.unsqueeze_dims::<4>(&[1, 2]); // (B, 1, 1, S)
          seg_key_valid = seg_key_valid.slice(s![.., .., .., 0..end]);
          seg_mask = match seg_mask {
            Some(mask) => Some(mask.bool_and(seg_key_valid)),
            None => Some(seg_key_valid),
          }
        }

        outputs.push(qwen_attention(
          query.clone().slice(s![.., start..end, .., ..]),
          key.clone().slice(s![.., 0..end, .., ..]),
          value.clone().slice(s![.., 0..end, .., ..]),
          seg_mask,
        ));
      }
      // (B, 1, 1, S)
      let trailing_mask = key_valid.map(|kv| kv.unsqueeze_dims::<4>(&[1, 2]));
      outputs.push(qwen_attention(
        query.slice(s![.., prefix_len.., .., ..]),
        key,
        value,
        trailing_mask,
      ));

      let prefill_hidden_states = Tensor::cat(outputs, 1);
      prefill_hidden_states.slice(s![.., 0..seq_len_q, .., ..]) // (B, S, 32, 128)
    } else {
      let decode_hidden_states = qwen_attention(query, key, value, attention_mask);
      decode_hidden_states.slice(s![.., 0..seq_len_q, .., ..])
    };

    let hidden_states = hidden_states.reshape([b, seq_len_q, num_attention_heads * head_dim]);
    self.to_out[0].forward(hidden_states)
  }
}

#[derive(Config, Debug)]
struct QwenImageAttentionConfig {
  dim: usize,
  heads: usize,
  dim_head: usize,
  eps: f64,
}

impl QwenImageAttentionConfig {
  fn init(&self, device: &Device) -> QwenImageAttention {
    let inner_dim = self.heads * self.dim_head;

    QwenImageAttention {
      to_q: LinearConfig::new(self.dim, inner_dim)
        .with_bias(false)
        .init(device),
      to_k: LinearConfig::new(self.dim, inner_dim)
        .with_bias(false)
        .init(device),
      to_v: LinearConfig::new(self.dim, inner_dim)
        .with_bias(false)
        .init(device),
      to_out: vec![
        LinearConfig::new(inner_dim, self.dim)
          .with_bias(false)
          .init(device),
      ],
      norm_q: RMSNormConfig::new(self.dim_head, self.eps).init(device),
      norm_k: RMSNormConfig::new(self.dim_head, self.eps).init(device),
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageTransformerBlock {
  img_norm1: QwenLayerNorm,
  attn: QwenImageAttention,
  img_norm2: QwenLayerNorm,
  img_mlp: QwenImageSwiGLUFeedForward,
}

impl QwenImageTransformerBlock {
  fn modulate(
    &self,
    hidden_states: Tensor<3>,
    mod_params: Tensor<2>,
    target_token_mask: Option<Tensor<1, Bool>>,
  ) -> (Tensor<3>, Tensor<3>) {
    let mod_params = mod_params.chunk(2, 1);
    let scale = mod_params[0].clone();
    let gate = mod_params[1].clone();
    let scale = select_modulation_rows(scale, target_token_mask.clone());
    let gate = select_modulation_rows(gate, target_token_mask);

    (hidden_states * (scale + 1.0), gate)
  }

  pub fn forward(
    &self,
    hidden_states: Tensor<3>,
    modulation: Tensor<2>,
    rotary_emb: (Tensor<2>, Tensor<2>),
    attention_mask: Option<Tensor<4, Bool>>,
    target_token_mask: Option<Tensor<1, Bool>>,
    layer_cache: Option<&mut QwenImageKVLayerCache>,
    kv_cache_mode: Option<KvCacheMode>,
    prefix_len: usize,
    segments: Option<Vec<(usize, usize, bool)>>,
    key_valid: Option<Tensor<2, Bool>>,
  ) -> Tensor<3> {
    let mods = modulation.chunk(2, 1);
    let mod1 = mods[0].clone();
    let mod2 = mods[1].clone();

    let (img_modulated, img_gate1) = self.modulate(
      self.img_norm1.forward(hidden_states.clone()),
      mod1,
      target_token_mask.clone(),
    );

    let attn_output = self.attn.forward(
      img_modulated,
      attention_mask,
      rotary_emb,
      layer_cache,
      kv_cache_mode,
      prefix_len,
      segments,
      key_valid,
    );

    let hidden_states = hidden_states.clone() + img_gate1.tanh() * attn_output;

    let (img_modulated2, img_gate2) = self.modulate(
      self.img_norm2.forward(hidden_states.clone()),
      mod2,
      target_token_mask,
    );

    let hidden_states = hidden_states + img_gate2.tanh() * self.img_mlp.forward(img_modulated2);
    hidden_states
  }
}

#[derive(Config, Debug)]
pub struct QwenImageTransformerBlockConfig {
  dim: usize,
  num_attention_heads: usize,
  attention_head_dim: usize,
  mlp_ratio: usize,
  eps: f64,
}

impl QwenImageTransformerBlockConfig {
  pub fn init(&self, device: &Device) -> QwenImageTransformerBlock {
    QwenImageTransformerBlock {
      img_norm1: QwenLayerNormConfig::new(self.eps).init(),
      attn: QwenImageAttentionConfig::new(
        self.dim,
        self.num_attention_heads,
        self.attention_head_dim,
        self.eps,
      )
      .init(device),
      img_norm2: QwenLayerNormConfig::new(self.eps).init(),
      img_mlp: QwenImageSwiGLUFeedForwardConfig::new(self.dim, self.dim * self.mlp_ratio)
        .init(device),
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageRope {
  freqs_cos_frame: Tensor<2>,
  freqs_cos_height: Tensor<2>,
  freqs_cos_width: Tensor<2>,
  freqs_sin_frame: Tensor<2>,
  freqs_sin_height: Tensor<2>,
  freqs_sin_width: Tensor<2>,
  total_dim: usize,
}

impl QwenImageRope {
  pub fn forward(
    &self,
    img_shapes: Vec<(usize, usize, usize)>,
    is_image_token: &[bool],
    device: &Device,
  ) -> (Tensor<2>, Tensor<2>) {
    let total_len = is_image_token.len();

    let mut frame_index = Vec::with_capacity(total_len);
    let mut image_height_index = Vec::new();
    let mut image_width_index = Vec::new();

    let mut cursor = 0;
    let mut position = 0;

    for (_frame, height, width) in img_shapes {
      let block_start = is_image_token[cursor..].iter().position(|&x| x).unwrap() + cursor;
      let text_len = block_start - cursor;

      for p in position..position + text_len {
        frame_index.push(p as i64);
      }
      position += text_len;

      cursor = block_start + height * width;
      for _ in 0..height * width {
        frame_index.push(position as i64);
      }

      position += height.max(width);

      let half_h = (height as i64) / 2;
      let half_w = (width as i64) / 2;

      for h in -(height as i64 - half_h)..half_h {
        for _ in 0..width {
          image_height_index.push(h);
        }
      }
      for _ in 0..height {
        for w in -(width as i64 - half_w)..half_w {
          image_width_index.push(w);
        }
      }
    }

    if cursor < total_len {
      for p in position..(position + total_len - cursor) {
        frame_index.push(p as i64);
      }
    }

    let mut final_height_index = frame_index.clone();
    let mut final_width_index = frame_index.clone();

    let mut img_idx = 0;
    for (i, &is_img) in is_image_token.iter().enumerate() {
      if is_img {
        final_height_index[i] = image_height_index[img_idx];
        final_width_index[i] = image_width_index[img_idx];
        img_idx += 1;
      }
    }

    let map_idx = |idx: i64| if idx < 0 { 9216 + idx } else { idx };

    let frame_mapped: Vec<i64> = frame_index.into_iter().map(map_idx).collect();
    let height_mapped: Vec<i64> = final_height_index.into_iter().map(map_idx).collect();
    let width_mapped: Vec<i64> = final_width_index.into_iter().map(map_idx).collect();

    let frame_tensor = Tensor::<1, Int>::from_ints(frame_mapped.as_slice(), device);
    let height_tensor = Tensor::<1, Int>::from_ints(height_mapped.as_slice(), device);
    let width_tensor = Tensor::<1, Int>::from_ints(width_mapped.as_slice(), device);

    let cos_f = self.freqs_cos_frame.clone().select(0, frame_tensor.clone());
    let sin_f = self.freqs_sin_frame.clone().select(0, frame_tensor);

    let cos_h = self
      .freqs_cos_height
      .clone()
      .select(0, height_tensor.clone());
    let sin_h = self.freqs_sin_height.clone().select(0, height_tensor);

    let cos_w = self.freqs_cos_width.clone().select(0, width_tensor.clone());
    let sin_w = self.freqs_sin_width.clone().select(0, width_tensor);

    let cos = Tensor::cat(vec![cos_f, cos_h, cos_w], 1);
    let sin = Tensor::cat(vec![sin_f, sin_h, sin_w], 1);

    let cos = Tensor::stack::<3>(vec![cos.clone(), cos], 2).reshape([total_len, self.total_dim]);
    let sin = Tensor::stack::<3>(vec![sin.clone(), sin], 2).reshape([total_len, self.total_dim]);

    (cos, sin)
  }
}

#[derive(Config, Debug)]
pub struct QwenImageRopeConfig {
  #[config(default = 10_000)]
  theta: usize,
  axes_dim: [usize; 3],
}

impl QwenImageRopeConfig {
  pub fn init(&self, device: &Device) -> QwenImageRope {
    let pos_index = Tensor::<1, Int>::arange(0..8192, device).cast(FloatDType::F32);
    let neg_index = Tensor::<1, Int>::arange(-1024..0, device).cast(FloatDType::F32);

    let index = Tensor::cat(vec![pos_index, neg_index], 0); // (9216,)
    let index_unsqueezed = index.unsqueeze_dim::<2>(1); // (9216, 1)

    let mut cos_tensors = vec![];
    let mut sin_tensors = vec![];

    let theta_ln = (self.theta as f64).ln();

    for &dim in self.axes_dim.iter() {
      let dim_f32 = dim as f32;
      let arange = Tensor::<1, Int>::arange_step(0..dim as i64, 2, device).cast(FloatDType::F32);
      let inv_freq = (arange / dim_f32 * -theta_ln).exp();
      let inv_freq_unsqueezed = inv_freq.unsqueeze_dim::<2>(0); // (1, dim/2)

      let freqs = index_unsqueezed.clone() * inv_freq_unsqueezed;
      cos_tensors.push(freqs.clone().cos());
      sin_tensors.push(freqs.sin());
    }

    let total_dim = self.axes_dim.iter().sum();

    QwenImageRope {
      freqs_cos_frame: cos_tensors[0].clone(),
      freqs_cos_height: cos_tensors[1].clone(),
      freqs_cos_width: cos_tensors[2].clone(),
      freqs_sin_frame: sin_tensors[0].clone(),
      freqs_sin_height: sin_tensors[1].clone(),
      freqs_sin_width: sin_tensors[2].clone(),
      total_dim,
    }
  }
}
