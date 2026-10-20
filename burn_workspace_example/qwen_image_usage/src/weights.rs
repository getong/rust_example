use std::{collections::BTreeSet, error::Error, path::PathBuf};

use burn::{module::Module, tensor::DType};
use burn_store::{
  FloatCastAdapter, ModuleAdapter, ModuleSnapshot, PyTorchToBurnAdapter, SafetensorsStore,
};

fn is_computed_buffer(name: &str) -> bool {
  name == "rotary_emb.inv_freqs"
    || name == "time_text_embed.time_proj.freqs"
    || name.starts_with("pos_embed.freqs_")
}

pub fn load_shards<T: Module + ModuleSnapshot>(
  model: &mut T,
  shards: &[PathBuf],
  remap: Option<&str>,
) -> Result<usize, Box<dyn Error>> {
  let mut applied = 0;
  let mut remaining: Option<BTreeSet<String>> = None;
  for shard in shards {
    let adapter = PyTorchToBurnAdapter.chain(FloatCastAdapter::to(DType::F32));
    let mut store = SafetensorsStore::from_file(shard)
      .allow_partial(true)
      .with_from_adapter(adapter);
    if let Some(pattern) = remap {
      store = store.with_key_remapping(pattern, "");
    }
    let result = model.load_from(&mut store)?;
    if let Some(error) = result.errors.first() {
      return Err(format!("{}: {error}", shard.display()).into());
    }
    applied += result.applied.len();
    let missing: BTreeSet<_> = result.missing.into_iter().map(|(name, _)| name).collect();
    remaining = Some(match remaining {
      None => missing,
      Some(previous) => previous.intersection(&missing).cloned().collect(),
    });
  }
  if applied == 0 {
    return Err(
      format!(
        "none of the model parameters matched the checkpoint in {}",
        shards[0].display()
      )
      .into(),
    );
  }
  let missing: Vec<_> = remaining
    .unwrap_or_default()
    .into_iter()
    .filter(|name| !is_computed_buffer(name))
    .collect();
  if !missing.is_empty() {
    return Err(
      format!(
        "{} model parameters have no weights, including: {}",
        missing.len(),
        missing
          .iter()
          .take(5)
          .cloned()
          .collect::<Vec<_>>()
          .join(", ")
      )
      .into(),
    );
  }
  Ok(applied)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_ignores_deterministically_computed_buffers() {
    assert!(is_computed_buffer("pos_embed.freqs_cos_height"));
    assert!(is_computed_buffer("rotary_emb.inv_freqs"));
    assert!(!is_computed_buffer("transformer_blocks.0.attn.to_q.weight"));
  }
}
