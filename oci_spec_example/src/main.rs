use std::str::FromStr;

use oci_spec::image::{
  Descriptor, DescriptorBuilder, ImageManifest, ImageManifestBuilder, MediaType, SCHEMA_VERSION,
  Sha256Digest,
};

const MANIFEST_PATH: &str = "my-manifest.json";

fn main() -> oci_spec::Result<()> {
  let image_manifest = build_image_manifest()?;
  image_manifest.to_file_pretty(MANIFEST_PATH)?;

  let loaded_manifest = ImageManifest::from_file(MANIFEST_PATH)?;
  assert_eq!(loaded_manifest.schema_version(), SCHEMA_VERSION);
  assert_eq!(loaded_manifest.layers().len(), 3);

  println!(
    "wrote {MANIFEST_PATH} with {} layers",
    loaded_manifest.layers().len()
  );

  Ok(())
}

fn build_image_manifest() -> oci_spec::Result<ImageManifest> {
  let config = DescriptorBuilder::default()
    .media_type(MediaType::ImageConfig)
    .size(7023_u64)
    .digest(Sha256Digest::from_str(
      "b5b2b2c507a0944348e0303114d8d93aaaa081732b86451d9bce1f432a537bc7",
    )?)
    .build()?;

  let layer_data = [
    (
      32654_u64,
      "9834876dcfb05cb167a5c24953eba58c4ac89b1adf57f28f2f9d09af107ee8f0",
    ),
    (
      16724_u64,
      "3c3a4604a545cdc127456d94e421cd355bca5b528f4a9c1905b15da2eb4a4c6b",
    ),
    (
      73109_u64,
      "ec4b8955958665577945c89419d1af06b5f7636b4ac3da7f12184802ad867736",
    ),
  ];

  let layers = layer_data
    .into_iter()
    .map(|(size, digest)| {
      DescriptorBuilder::default()
        .media_type(MediaType::ImageLayerGzip)
        .size(size)
        .digest(Sha256Digest::from_str(digest)?)
        .build()
    })
    .collect::<oci_spec::Result<Vec<Descriptor>>>()?;

  ImageManifestBuilder::default()
    .schema_version(SCHEMA_VERSION)
    .config(config)
    .layers(layers)
    .build()
}
