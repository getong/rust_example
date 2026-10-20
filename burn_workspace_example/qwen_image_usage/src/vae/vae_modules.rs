use burn::{
  Tensor,
  config::Config,
  module::{Module, Param},
  nn::{
    PaddingConfig2d,
    conv::{Conv2d, Conv2dConfig},
  },
  tensor::{
    Device,
    activation::silu,
    module::{attention, conv2d, interpolate},
    ops::{AttentionModuleOptions, ConvOptions, InterpolateMode, InterpolateOptions, PadMode},
    s,
  },
};
use serde::{Deserialize, Serialize};

#[derive(Module, Debug)]
pub struct QwenImageAvgDown3D {
  out_channels: usize,
  factor_t: usize,
  factor_s: usize,
  factor: usize,
  group_size: usize,
}

impl QwenImageAvgDown3D {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    // x: (B, C, T, H, W)
    let pad_t = (self.factor_t - x.dims()[2] % self.factor_t) % self.factor_t;
    let x = x.pad([(pad_t, 0), (0, 0), (0, 0)], PadMode::Constant(0.0));
    let [b, c, t, h, w] = x.dims();
    let x = x.reshape([
      b,
      c,
      t / self.factor_t,
      self.factor_t,
      h / self.factor_s,
      self.factor_s,
      w / self.factor_s,
      self.factor_s,
    ]);
    let x = x.permute([0, 1, 3, 5, 7, 2, 4, 6]);
    let x = x.reshape([
      b,
      c * self.factor,
      t / self.factor_t,
      h / self.factor_s,
      w / self.factor_s,
    ]);
    let x = x.reshape([
      b,
      self.out_channels,
      self.group_size,
      t / self.factor_t,
      h / self.factor_s,
      w / self.factor_s,
    ]);

    x.mean_dim(2).squeeze_dim(2)
  }
}

#[derive(Config, Debug)]
pub struct QwenImageAvgDown3DConfig {
  in_channels: usize,
  out_channels: usize,
  factor_t: usize,
  #[config(default = 1)]
  factor_s: usize,
}

impl QwenImageAvgDown3DConfig {
  pub fn init(&self) -> QwenImageAvgDown3D {
    let factor = self.factor_t * self.factor_s * self.factor_s;
    assert!(
      (self.in_channels * factor % self.out_channels == 0),
      "`in_channels` ({}) times the downsampling factor ({}) must be divisible by `out_channels` ({}).",
      self.in_channels,
      factor,
      self.out_channels
    );

    let group_size = self.in_channels * factor / self.out_channels;
    QwenImageAvgDown3D {
      out_channels: self.out_channels,
      factor_t: self.factor_t,
      factor_s: self.factor_s,
      factor,
      group_size,
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageDupUp3D {
  out_channels: usize,
  factor_t: usize,
  factor_s: usize,
  repeats: usize,
}

impl QwenImageDupUp3D {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    let [b, c, t, h, w] = x.dims();

    let x = x
      .unsqueeze_dim::<6>(2) // (B, C, 1, T, H, W)
      .expand([b, c, self.repeats, t, h, w])
      .reshape([b, c * self.repeats, t, h, w]); // (B, C * repeats, t, h, w)

    let x = x.reshape([
      b,
      self.out_channels,
      self.factor_t,
      self.factor_s,
      self.factor_s,
      t,
      h,
      w,
    ]);

    let x = x.permute([0, 1, 5, 2, 6, 3, 7, 4]); // (B, out_channels, t, factor_t, h, factor_s, w, factor_s)
    let new_t = t * self.factor_t;
    let new_h = h * self.factor_s;
    let new_w = w * self.factor_s;
    let x = x.reshape([b, self.out_channels, new_t, new_h, new_w]);

    x.slice(s![.., .., self.factor_t - 1.., .., ..])
  }
}

#[derive(Config, Debug)]
pub struct QwenImageDupUp3DConfig {
  in_channels: usize,
  out_channels: usize,
  factor_t: usize,
  #[config(default = 1)]
  factor_s: usize,
}

impl QwenImageDupUp3DConfig {
  pub fn init(&self) -> QwenImageDupUp3D {
    let factor = self.factor_t * self.factor_s * self.factor_s;
    assert!(self.out_channels * factor % self.in_channels == 0);
    let repeats = self.out_channels * factor / self.in_channels;
    QwenImageDupUp3D {
      out_channels: self.out_channels,
      factor_t: self.factor_t,
      factor_s: self.factor_s,
      repeats,
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageCausalConv3D {
  pub weight: Param<Tensor<4>>,
  pub bias: Param<Tensor<1>>,
  stride: [usize; 2],
  padding: [usize; 2],
}

impl QwenImageCausalConv3D {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    let x = x.squeeze_dim::<4>(2); // (B, C, H, W)
    let x = conv2d(
      x,
      self.weight.val(),
      Some(self.bias.val()),
      ConvOptions::new(self.stride, self.padding, [1, 1], 1),
    );

    x.unsqueeze_dim(2)
  }
}

#[derive(Config, Debug)]
pub struct QwenImageCausalConv3DConfig {
  in_channels: usize,
  out_channels: usize,
  kernel_size: [usize; 2],
  #[config(default = "[1, 1]")]
  stride: [usize; 2],
  #[config(default = "[0, 0]")]
  padding: [usize; 2],
}

impl QwenImageCausalConv3DConfig {
  pub fn init(&self, device: &Device) -> QwenImageCausalConv3D {
    let conv2d = Conv2dConfig::new([self.in_channels, self.out_channels], self.kernel_size)
      .with_stride(self.stride)
      .init(device);
    QwenImageCausalConv3D {
      weight: conv2d.weight,
      bias: conv2d.bias.expect("Conv2d bias is enabled by default"),
      stride: self.stride,
      padding: self.padding,
    }
  }
}

fn rms_normalize<const D: usize>(x: Tensor<D>, scale: f32) -> Tensor<D> {
  let norm = x.clone().square().sum_dim(1).sqrt().clamp_min(1e-12);
  x / norm * scale
}

#[derive(Module, Debug)]
pub struct QwenImageRMSNorm3D {
  gamma: Param<Tensor<4>>,
  scale: f32,
}

impl QwenImageRMSNorm3D {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    rms_normalize(x, self.scale) * self.gamma.val().unsqueeze::<5>()
  }
}

#[derive(Config, Debug)]
pub struct QwenImageRMSNorm3DConfig {
  dim: usize,
}

impl QwenImageRMSNorm3DConfig {
  pub fn init(&self, device: &Device) -> QwenImageRMSNorm3D {
    QwenImageRMSNorm3D {
      gamma: Param::from_tensor(Tensor::ones([self.dim, 1, 1, 1], device)),
      scale: (self.dim as f32).sqrt(),
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageRMSNorm2D {
  gamma: Param<Tensor<3>>,
  scale: f32,
}

impl QwenImageRMSNorm2D {
  pub fn forward(&self, x: Tensor<4>) -> Tensor<4> {
    rms_normalize(x, self.scale) * self.gamma.val().unsqueeze::<4>()
  }
}

#[derive(Config, Debug)]
pub struct QwenImageRMSNorm2DConfig {
  dim: usize,
}

impl QwenImageRMSNorm2DConfig {
  pub fn init(&self, device: &Device) -> QwenImageRMSNorm2D {
    QwenImageRMSNorm2D {
      gamma: Param::from_tensor(Tensor::ones([self.dim, 1, 1], device)),
      scale: (self.dim as f32).sqrt(),
    }
  }
}

// scale_factor is always [2.0, 2.0] with nearest-exact mode
#[derive(Module, Debug)]
pub struct QwenImageUpsample {}

impl QwenImageUpsample {
  pub fn forward(&self, x: Tensor<4>) -> Tensor<4> {
    interpolate(
      x,
      InterpolateOptions::new(InterpolateMode::Nearest).with_scale_factor([2.0, 2.0]),
    )
  }
}

#[derive(Config, Debug)]
pub struct QwenImageUpsampleConfig {}

impl QwenImageUpsampleConfig {
  pub fn init(&self) -> QwenImageUpsample {
    QwenImageUpsample {}
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ResampleMode {
  Upsample2D,
  Upsample3D,
  Downsample2D,
  Downsample3D,
}

// We don't need time_conv
#[derive(Module, Debug)]
pub struct QwenImageResample {
  resample: (Option<QwenImageUpsample>, Conv2d),
}

impl QwenImageResample {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    let [b, c, t, h, w] = x.dims();
    let x = x.permute([0, 2, 1, 3, 4]); // (B, T, C, H, W)
    let mut x = x.reshape([b * t, c, h, w]); // (B * T, C, H, W)
    if let Some(upsample) = &self.resample.0 {
      // Upsample
      x = upsample.forward(x);
    } else {
      // Downsample
      x = x.pad([(0, 1), (0, 1)], PadMode::Constant(0.0));
    }
    x = self.resample.1.forward(x);
    let [_, c, h, w] = x.dims();
    let x = x.reshape([b, t, c, h, w]); // (B, T, C, H, W)
    let x = x.permute([0, 2, 1, 3, 4]); // (B, C, T, H, W)
    x
  }
}

#[derive(Config, Debug)]
pub struct QwenImageResampleConfig {
  dim: usize,
  mode: ResampleMode,
  upsample_out_dim: Option<usize>,
}

impl QwenImageResampleConfig {
  pub fn init(&self, device: &Device) -> QwenImageResample {
    let upsample_out_dim = self.upsample_out_dim.unwrap_or(self.dim / 2);
    let resample = match self.mode {
      ResampleMode::Upsample2D => (
        Some(QwenImageUpsampleConfig::new().init()),
        Conv2dConfig::new([self.dim, upsample_out_dim], [3, 3])
          .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
          .init(device),
      ),
      ResampleMode::Upsample3D => (
        Some(QwenImageUpsampleConfig::new().init()),
        Conv2dConfig::new([self.dim, upsample_out_dim], [3, 3])
          .with_padding(PaddingConfig2d::Explicit(1, 1, 1, 1))
          .init(device),
      ),
      ResampleMode::Downsample2D => (
        None,
        Conv2dConfig::new([self.dim, self.dim], [3, 3])
          .with_stride([2, 2])
          .init(device),
      ),
      ResampleMode::Downsample3D => (
        None,
        Conv2dConfig::new([self.dim, self.dim], [3, 3])
          .with_stride([2, 2])
          .init(device),
      ),
    };
    QwenImageResample { resample }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageResidualBlock {
  norm1: QwenImageRMSNorm3D,
  conv1: QwenImageCausalConv3D,
  norm2: QwenImageRMSNorm3D,
  conv2: QwenImageCausalConv3D,
  conv_shortcut: Option<QwenImageCausalConv3D>,
}

impl QwenImageResidualBlock {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    // x: (B, C, T, H, W)
    let h = match &self.conv_shortcut {
      Some(shortcut) => shortcut.forward(x.clone()),
      None => x.clone(),
    };

    let x = self.norm1.forward(x);
    let x = silu(x);
    let x = self.conv1.forward(x);

    let x = self.norm2.forward(x);
    let x = silu(x);
    let x = self.conv2.forward(x);
    x + h
  }
}

#[derive(Config, Debug)]
pub struct QwenImageResidualBlockConfig {
  in_dim: usize,
  out_dim: usize,
}

impl QwenImageResidualBlockConfig {
  pub fn init(&self, device: &Device) -> QwenImageResidualBlock {
    let conv_shortcut = if self.in_dim != self.out_dim {
      Some(QwenImageCausalConv3DConfig::new(self.in_dim, self.out_dim, [1, 1]).init(device))
    } else {
      None
    };
    QwenImageResidualBlock {
      norm1: QwenImageRMSNorm3DConfig::new(self.in_dim).init(device),
      conv1: QwenImageCausalConv3DConfig::new(self.in_dim, self.out_dim, [3, 3])
        .with_padding([1, 1])
        .init(device),
      norm2: QwenImageRMSNorm3DConfig::new(self.out_dim).init(device),
      conv2: QwenImageCausalConv3DConfig::new(self.out_dim, self.out_dim, [3, 3])
        .with_padding([1, 1])
        .init(device),
      conv_shortcut,
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageAttentionBlock {
  norm: QwenImageRMSNorm2D,
  to_qkv: Conv2d,
  proj: Conv2d,
}

impl QwenImageAttentionBlock {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    // x: (B, C, T, H, W)
    let identity = x.clone();
    let [b, c, t, h, w] = x.dims();

    let x = x.permute([0, 2, 1, 3, 4]); // (B, T, C, H, W)
    let x = x.reshape([b * t, c, h, w]); // (B * T, C, H, W)
    let x = self.norm.forward(x);

    let qkv = self.to_qkv.forward(x); // (B * T, C, H, W)
    let qkv = qkv.reshape([b * t, 1, c * 3, h * w]);
    let qkv = qkv.permute([0, 1, 3, 2]); // (B * T, 1, H * W, C * 3)
    let qkv_chunks = qkv.chunk(3, 3);
    let (q, k, v) = (
      qkv_chunks[0].clone(),
      qkv_chunks[1].clone(),
      qkv_chunks[2].clone(),
    );
    // Each of q, k and v is now (B * T, 1, H * W, C)
    // expects (batch_size, num_heads, seq_len, head_dim)
    // The vendored dispatch checks options.scale.is_some() before any strategy is chosen
    // and routes straight to attention_fallback. So the VAE now runs the naive attention
    // We're just making implicit math explicit, no difference numerically, just routing to naive
    // as flash attention doesn't run here, it's just a single attention head.
    let mut attn_options = AttentionModuleOptions::default();
    attn_options.scale = Some(1.0 / (c as f64).sqrt());
    let x = attention(q, k, v, None, None, attn_options);
    let x = x.squeeze_dim::<3>(1); // (B * T, H * W, C)
    let x = x.permute([0, 2, 1]); // (B * T, C, H * W)
    let x = x.reshape([b * t, c, h, w]); // (B * T, C, H, W)

    let x = self.proj.forward(x);
    let x = x.reshape([b, t, c, h, w]);
    let x = x.permute([0, 2, 1, 3, 4]); // (B, C, T, H, W)
    x + identity
  }
}

#[derive(Config, Debug)]
pub struct QwenImageAttentionBlockConfig {
  dim: usize,
}

impl QwenImageAttentionBlockConfig {
  pub fn init(&self, device: &Device) -> QwenImageAttentionBlock {
    QwenImageAttentionBlock {
      norm: QwenImageRMSNorm2DConfig::new(self.dim).init(device),
      to_qkv: Conv2dConfig::new([self.dim, self.dim * 3], [1, 1]).init(device),
      proj: Conv2dConfig::new([self.dim, self.dim], [1, 1]).init(device),
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageMidBlock {
  resnets: Vec<QwenImageResidualBlock>,
  attentions: Vec<QwenImageAttentionBlock>,
}

impl QwenImageMidBlock {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    // x: (B, C, T, H, W)
    let mut x = self.resnets[0].forward(x);

    for (attn, resnet) in self.attentions.iter().zip(self.resnets.iter().skip(1)) {
      x = attn.forward(x);
      x = resnet.forward(x);
    }

    x
  }
}

#[derive(Config, Debug)]
pub struct QwenImageMidBlockConfig {
  dim: usize,
  #[config(default = 1)]
  num_layers: usize,
}

impl QwenImageMidBlockConfig {
  pub fn init(&self, device: &Device) -> QwenImageMidBlock {
    let mut resnets = vec![QwenImageResidualBlockConfig::new(self.dim, self.dim).init(device)];
    let mut attentions = vec![];
    for _ in 0..self.num_layers {
      attentions.push(QwenImageAttentionBlockConfig::new(self.dim).init(device));
      resnets.push(QwenImageResidualBlockConfig::new(self.dim, self.dim).init(device));
    }
    QwenImageMidBlock {
      resnets,
      attentions,
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageResidualDownBlock {
  avg_shortcut: QwenImageAvgDown3D,
  resnets: Vec<QwenImageResidualBlock>,
  downsampler: Option<QwenImageResample>,
}

impl QwenImageResidualDownBlock {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    // x: (B, C, T, H, W)
    let x_copy = x.clone();
    let mut x = x;
    for resnet in &self.resnets {
      x = resnet.forward(x);
    }
    if let Some(downsampler) = &self.downsampler {
      x = downsampler.forward(x);
    }

    x + self.avg_shortcut.forward(x_copy)
  }
}

#[derive(Config, Debug)]
pub struct QwenImageResidualDownBlockConfig {
  in_dim: usize,
  out_dim: usize,
  num_res_blocks: usize,
  #[config(default = false)]
  temporal_downsample: bool,
  #[config(default = false)]
  down_flag: bool,
}

impl QwenImageResidualDownBlockConfig {
  pub fn init(&self, device: &Device) -> QwenImageResidualDownBlock {
    let factor_t = if self.temporal_downsample { 2 } else { 1 };
    let factor_s = if self.down_flag { 2 } else { 1 };
    let mut resnets = vec![];
    let mut in_dim = self.in_dim;
    for _ in 0..self.num_res_blocks {
      resnets.push(QwenImageResidualBlockConfig::new(in_dim, self.out_dim).init(device));
      in_dim = self.out_dim;
    }
    let downsampler = match self.down_flag {
      true => {
        let mode = if self.temporal_downsample {
          ResampleMode::Downsample3D
        } else {
          ResampleMode::Downsample2D
        };
        Some(QwenImageResampleConfig::new(self.out_dim, mode).init(device))
      }
      false => None,
    };

    QwenImageResidualDownBlock {
      avg_shortcut: QwenImageAvgDown3DConfig::new(self.in_dim, self.out_dim, factor_t)
        .with_factor_s(factor_s)
        .init(),
      resnets,
      downsampler,
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageResidualUpBlock {
  avg_shortcut: Option<QwenImageDupUp3D>,
  resnets: Vec<QwenImageResidualBlock>,
  upsampler: Option<QwenImageResample>,
}

impl QwenImageResidualUpBlock {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    // x: (B, C, T, H, W)
    let x_copy = x.clone();
    let mut x = x;
    for resnet in &self.resnets {
      x = resnet.forward(x);
    }

    if let Some(upsampler) = &self.upsampler {
      x = upsampler.forward(x);
    }

    if let Some(avg_shortcut) = &self.avg_shortcut {
      x = x + avg_shortcut.forward(x_copy);
    }
    x
  }
}

#[derive(Config, Debug)]
pub struct QwenImageResidualUpBlockConfig {
  in_dim: usize,
  out_dim: usize,
  num_res_blocks: usize,
  #[config(default = false)]
  temporal_upsample: bool,
  #[config(default = false)]
  up_flag: bool,
}

impl QwenImageResidualUpBlockConfig {
  pub fn init(&self, device: &Device) -> QwenImageResidualUpBlock {
    let factor_t = if self.temporal_upsample { 2 } else { 1 };
    let factor_s = 2;
    let avg_shortcut = match self.up_flag {
      true => Some(
        QwenImageDupUp3DConfig::new(self.in_dim, self.out_dim, factor_t)
          .with_factor_s(factor_s)
          .init(),
      ),
      false => None,
    };
    let mut resnets = vec![];
    let mut current_dim = self.in_dim;
    for _ in 0..self.num_res_blocks + 1 {
      resnets.push(QwenImageResidualBlockConfig::new(current_dim, self.out_dim).init(device));
      current_dim = self.out_dim;
    }

    let upsampler = match self.up_flag {
      true => {
        let mode = if self.temporal_upsample {
          ResampleMode::Upsample3D
        } else {
          ResampleMode::Upsample2D
        };
        Some(
          QwenImageResampleConfig::new(self.out_dim, mode)
            .with_upsample_out_dim(Some(self.out_dim))
            .init(device),
        )
      }
      false => None,
    };
    QwenImageResidualUpBlock {
      avg_shortcut,
      resnets,
      upsampler,
    }
  }
}

#[derive(Module, Debug)]
pub struct QwenImageUpBlock {
  resnets: Vec<QwenImageResidualBlock>,
  upsamplers: Option<Vec<QwenImageResample>>, // Either None or ModuleList() containing exactly one Resample
}

impl QwenImageUpBlock {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    let mut x = x;
    for resnet in &self.resnets {
      x = resnet.forward(x);
    }

    if let Some(upsamplers) = &self.upsamplers {
      x = upsamplers[0].forward(x);
    }
    x
  }
}

#[derive(Config, Debug)]
pub struct QwenImageUpBlockConfig {
  in_dim: usize,
  out_dim: usize,
  num_res_blocks: usize,
  upsample_mode: Option<ResampleMode>,
}

impl QwenImageUpBlockConfig {
  pub fn init(&self, device: &Device) -> QwenImageUpBlock {
    let mut resnets = vec![];
    let mut current_dim = self.in_dim;
    for _ in 0..self.num_res_blocks + 1 {
      resnets.push(QwenImageResidualBlockConfig::new(current_dim, self.out_dim).init(device));
      current_dim = self.out_dim;
    }
    let upsamplers = match &self.upsample_mode {
      Some(mode) => Some(vec![
        QwenImageResampleConfig::new(self.out_dim, mode.clone()).init(device),
      ]),
      None => None,
    };
    QwenImageUpBlock {
      resnets,
      upsamplers,
    }
  }
}
