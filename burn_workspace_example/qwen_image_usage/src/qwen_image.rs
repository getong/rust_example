use burn::{
  Tensor,
  config::Config,
  module::Module,
  nn::{Linear, LinearConfig},
  tensor::{Bool, Device, IndexingUpdateOp, Int, activation::silu, s},
};

use crate::{
  qwen_image_modules::{
    KvCacheMode, QwenImageAdaLayerNormContinuous, QwenImageAdaLayerNormContinuousConfig,
    QwenImageKVCache, QwenImageRope, QwenImageRopeConfig, QwenImageTextProjection,
    QwenImageTextProjectionConfig, QwenImageTimestepProjEmbeddings,
    QwenImageTimestepProjEmbeddingsConfig, QwenImageTransformerBlock,
    QwenImageTransformerBlockConfig, qwenimage_prefix_segments,
  },
  streamer::QwenImageBlockStreamerWeb,
};

#[derive(Module, Debug)]
pub struct DummyModule {}

#[derive(Module, Debug)]
pub struct QwenImageTransformerModel {
  pos_embed: QwenImageRope,
  time_text_embed: QwenImageTimestepProjEmbeddings,
  txt_in: QwenImageTextProjection,
  img_in: Linear,
  modulation: (DummyModule, Linear), // Original is nn.Sequential[nn.SiLU(), nn.Linear()].
  transformer_blocks: Vec<QwenImageTransformerBlock>,
  norm_out: QwenImageAdaLayerNormContinuous,
  proj_out: Linear,
}

impl QwenImageTransformerModel {
  pub fn build_token_metadata(
    image_pad_mask_vec: &[bool],
    img_shapes: Vec<(usize, usize, usize)>,
    device: &Device,
  ) -> (Vec<i64>, Tensor<1, Bool>, Vec<bool>) {
    let mut image_ids_vec = vec![-1_i64; image_pad_mask_vec.len()];
    let mut target_token_mask_vec = vec![false; image_pad_mask_vec.len()];

    let block_lengths: Vec<usize> = img_shapes.iter().map(|s| s.0 * s.1 * s.2).collect();

    let mut img_token_idx = 0;
    for (i, &is_img) in image_pad_mask_vec.iter().enumerate() {
      if is_img {
        let mut sum = 0;
        for (b, &len) in block_lengths.iter().enumerate() {
          sum += len;
          if img_token_idx < sum {
            image_ids_vec[i] = b as i64;
            if b == block_lengths.len() - 1 {
              target_token_mask_vec[i] = true;
            }
            break;
          }
        }
        img_token_idx += 1;
      }
    }

    let target_token_mask_int: Vec<i64> = target_token_mask_vec
      .iter()
      .map(|&b| if b { 1 } else { 0 })
      .collect();
    let target_token_mask =
      Tensor::<1, Int>::from_ints(target_token_mask_int.as_slice(), device).bool();

    (image_ids_vec, target_token_mask, target_token_mask_vec)
  }

  pub fn forward(
    &self,
    streamer: &mut QwenImageBlockStreamerWeb,
    hidden_states: Tensor<3>,
    encoder_hidden_states: Tensor<3>,
    timestep: Tensor<1>,
    img_shapes: Vec<(usize, usize, usize)>,
    img_mask: &[bool],
    encoder_hidden_states_mask: Option<Tensor<2, Bool>>,
    mut kv_cache: Option<&mut QwenImageKVCache>,
    kv_cache_mode: Option<KvCacheMode>,
  ) -> Tensor<3> {
    let device = &hidden_states.device();
    let [batch_size, _, dim] = encoder_hidden_states.dims();

    let hidden_states = self.img_in.forward(hidden_states);

    let encoder_hidden_states = self.txt_in.forward(encoder_hidden_states);

    let img_mask_bool: &[bool] = img_mask;

    let mut image_pad_mask_vec = Vec::new();
    let mut repeat_indices = Vec::new();
    for (i, &is_img) in img_mask_bool.iter().enumerate() {
      if is_img {
        for _ in 0..4 {
          image_pad_mask_vec.push(true);
          repeat_indices.push(i as i64);
        }
      } else {
        image_pad_mask_vec.push(false);
        repeat_indices.push(i as i64);
      }
    }

    let mut joint_key_valid = None;
    if let Some(enc_mask) = encoder_hidden_states_mask {
      let [batch_size_mask, text_seq_len] = enc_mask.dims();
      let enc_mask_vec = enc_mask.into_data().iter::<bool>().collect::<Vec<bool>>();
      let joint_seq_len = image_pad_mask_vec.len();

      let mut jkv_vec = vec![true; batch_size_mask * joint_seq_len];

      let text_in_orig: Vec<usize> = img_mask_bool
        .iter()
        .enumerate()
        .take(text_seq_len)
        .filter(|x| !*x.1)
        .map(|(i, _)| i)
        .collect();

      let text_in_joint: Vec<usize> = image_pad_mask_vec
        .iter()
        .enumerate()
        .filter(|x| !*x.1)
        .map(|(i, _)| i)
        .collect();

      for b in 0..batch_size_mask {
        for (idx, &orig_i) in text_in_orig.iter().enumerate() {
          if idx < text_in_joint.len() {
            let joint_j = text_in_joint[idx];
            jkv_vec[b * joint_seq_len + joint_j] = enc_mask_vec[b * text_seq_len + orig_i];
          }
        }
      }

      let jkv_ints: Vec<i64> = jkv_vec.into_iter().map(|b| if b { 1 } else { 0 }).collect();
      joint_key_valid = Some(
        Tensor::<1, Int>::from_ints(jkv_ints.as_slice(), device)
          .reshape([batch_size_mask, joint_seq_len])
          .bool(),
      );
    }

    let target_shape = img_shapes.last().unwrap();
    let target_tokens = target_shape.0 * target_shape.1 * target_shape.2;
    let zeros = Tensor::<3>::zeros([batch_size, target_tokens / 4, dim], device);

    let mut joint_hidden_states = Tensor::cat(vec![encoder_hidden_states, zeros], 1);

    let repeat_indices_tensor = Tensor::<1, Int>::from_ints(repeat_indices.as_slice(), device);
    joint_hidden_states = joint_hidden_states.select(1, repeat_indices_tensor);

    let img_indices: Vec<i64> = image_pad_mask_vec
      .iter()
      .enumerate()
      .filter(|x| *x.1)
      .map(|(i, _)| i as i64)
      .collect();

    let img_indices_tensor = Tensor::<1, Int>::from_ints(img_indices.as_slice(), device)
      .unsqueeze_dims::<3>(&[0, 2])
      .repeat_dim(0, batch_size)
      .repeat_dim(2, dim);

    joint_hidden_states = joint_hidden_states.scatter(
      1,
      img_indices_tensor,
      hidden_states,
      IndexingUpdateOp::Assign,
    );

    let (image_ids, target_token_mask, target_token_mask_vec) =
      Self::build_token_metadata(&image_pad_mask_vec, img_shapes.clone(), device);

    let (cos, sin) = self
      .pos_embed
      .forward(img_shapes, &image_pad_mask_vec, device);
    let mut rotary_emb = (cos, sin);

    let t_zero = Tensor::<1>::zeros([1], device);
    let timestep = Tensor::cat(vec![timestep, t_zero], 0);
    let temb = self.time_text_embed.forward(timestep);

    let modulation = self.modulation.1.forward(silu(temb.clone()));

    let prefix_len = target_token_mask_vec.iter().filter(|&&b| !b).count();
    let mut modulation_mask = target_token_mask.clone();

    let mut segments = None;
    let mut attention_mask = None;
    let mut block_key_valid = joint_key_valid.clone();

    if let Some(KvCacheMode::CACHED) = kv_cache_mode {
      let seq_len = joint_hidden_states.dims()[1];
      let rope_len = rotary_emb.0.dims()[0];
      let rope_dim = rotary_emb.0.dims()[1];
      let mask_len = modulation_mask.dims()[0];

      joint_hidden_states =
        joint_hidden_states.slice(s![0..batch_size, prefix_len..seq_len, 0..dim]);
      rotary_emb = (
        rotary_emb.0.slice(s![prefix_len..rope_len, 0..rope_dim]),
        rotary_emb.1.slice(s![prefix_len..rope_len, 0..rope_dim]),
      );
      modulation_mask = modulation_mask.slice(s![prefix_len..mask_len]);

      attention_mask = block_key_valid.map(|jkv| jkv.unsqueeze_dims::<4>(&[1, 2]));
      block_key_valid = None;
    } else {
      segments = Some(qwenimage_prefix_segments(&image_ids, prefix_len));
    }

    for i in 0..streamer.num_layers {
      let layer_cache = kv_cache.as_deref_mut().map(|cache| cache.get_layer_mut(i));

      joint_hidden_states = streamer.get(i).forward(
        joint_hidden_states,
        modulation.clone(),
        rotary_emb.clone(),
        attention_mask.clone(),
        Some(modulation_mask.clone()),
        layer_cache,
        kv_cache_mode.clone(),
        prefix_len,
        segments.clone(),
        block_key_valid.clone(),
      );
    }

    joint_hidden_states = self
      .norm_out
      .forward(joint_hidden_states, temb, modulation_mask);

    let out = self.proj_out.forward(joint_hidden_states);
    out
  }
}

#[derive(Config, Debug)]
pub struct QwenImageTransformerModelConfig {
  #[config(default = 1)]
  pub patch_size: usize,
  #[config(default = 64)]
  pub in_channels: usize,
  #[config(default = 64)]
  pub out_channels: usize,
  #[config(default = 32)]
  pub num_layers: usize,
  #[config(default = 128)]
  pub attention_head_dim: usize,
  #[config(default = 32)]
  pub num_attention_heads: usize,
  #[config(default = 4096)]
  pub context_in_dim: usize,
  #[config(default = 3)]
  pub mlp_ratio: usize,
  pub axes_dim_rope: [usize; 3],
  #[config(default = 1e-6)]
  pub eps: f64,
  #[config(default = true)]
  pub causal_condition: bool,
}

impl QwenImageTransformerModelConfig {
  pub fn init(&self, device: &Device) -> QwenImageTransformerModel {
    let inner_dim = self.num_attention_heads * self.attention_head_dim;
    let mut transformer_blocks = vec![];
    for _ in 0..self.num_layers {
      transformer_blocks.push(
        QwenImageTransformerBlockConfig::new(
          inner_dim,
          self.num_attention_heads,
          self.attention_head_dim,
          self.mlp_ratio,
          self.eps,
        )
        .init(device),
      );
    }
    QwenImageTransformerModel {
      pos_embed: QwenImageRopeConfig::new(self.axes_dim_rope).init(device),
      time_text_embed: QwenImageTimestepProjEmbeddingsConfig::new(inner_dim).init(device),
      txt_in: QwenImageTextProjectionConfig::new(self.context_in_dim, inner_dim, self.eps)
        .init(device),
      img_in: LinearConfig::new(
        self.in_channels * self.patch_size * self.patch_size,
        inner_dim,
      )
      .with_bias(false)
      .init(device),
      modulation: (
        DummyModule {},
        LinearConfig::new(inner_dim, 4 * inner_dim)
          .with_bias(false)
          .init(device),
      ),
      transformer_blocks,
      norm_out: QwenImageAdaLayerNormContinuousConfig::new(inner_dim, inner_dim, self.eps)
        .init(device),
      proj_out: LinearConfig::new(
        inner_dim,
        self.patch_size * self.patch_size * self.out_channels,
      )
      .with_bias(false)
      .init(device),
    }
  }
}
