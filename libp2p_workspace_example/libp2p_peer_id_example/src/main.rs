use libp2p::{
  identity::{self, PublicKey},
  PeerId,
};

fn main() {
  generate_ed25519();

  generate_secp256k1();

  generate_ecdsa();
}

fn generate_ed25519() {
  // Generate a keypair
  let keypair = identity::Keypair::generate_ed25519();

  // Derive the PeerId from the keypair
  let peer_id = PeerId::from(keypair.public());

  let public_key = keypair.public();
  let public_key_bytes = public_key.encode_protobuf();
  println!(
    "string from public_key_bytes {:?} is {:?}",
    public_key_bytes,
    String::from_utf8_lossy(&public_key_bytes)
  );

  let public_key_str = hex::encode(public_key_bytes.clone());
  println!("public_key_str: {:?}", public_key_str);

  if let Ok(new_public_key_bytes) = hex::decode(public_key_str.clone()) {
    if let Ok(new_public_key) = PublicKey::try_decode_protobuf(&new_public_key_bytes) {
      let new_peer_id = new_public_key.to_peer_id();
      println!("new peer id equals peer id :{:?}", peer_id == new_peer_id);
    } else {
      println!("Line {}, can not decode publickey", line!());
    }
  } else {
    println!("Line {}, can not decode hex", line!());
  }

  let bytes = String::from_utf8_lossy(&public_key_bytes);
  let new_public_key_bytes = bytes.as_bytes();
  if let Ok(new_public_key) = PublicKey::try_decode_protobuf(&new_public_key_bytes) {
    let new_peer_id = new_public_key.to_peer_id();
    println!("new peer id equals peer id :{:?}", peer_id == new_peer_id);
  } else {
    println!("Line {}, can not decode publickey", line!());
  }

  println!("ed25519 keypair.public(): {:?}", public_key);

  // PeerId::from_bytes(p.trim().as_bytes()).ok()

  println!("Generated ed25519 PeerId: {:?}", peer_id);

  let private_key = keypair.to_protobuf_encoding().unwrap();
  let hex_string = hex::encode(private_key);

  println!("ed25519 private key in hex: {}", hex_string);

  if let Ok(data) = hex::decode(hex_string) {
    if let Ok(keypair) = identity::Keypair::from_protobuf_encoding(&data) {
      println!(
        "from string ed25519 PeerId: {:?}",
        PeerId::from(keypair.public())
      );
    } else {
      println!("ed25519 from bytes failed");
    }
  } else {
    println!("ed25519 hex decode failed");
  }

  println!();
}

fn generate_secp256k1() {
  // Generate a keypair
  let keypair = identity::Keypair::generate_secp256k1();

  // Derive the PeerId from the keypair
  let peer_id = PeerId::from(keypair.public());

  println!("Generated secp256k1 PeerId: {:?}", peer_id);

  let private_key = keypair.to_protobuf_encoding().unwrap();
  let hex_string = hex::encode(private_key);

  println!("secp256k1 : {}", hex_string);

  if let Ok(data) = hex::decode(hex_string) {
    if let Ok(keypair) = identity::Keypair::from_protobuf_encoding(&data) {
      println!(
        "from string secp256k1 PeerId: {:?}",
        PeerId::from(keypair.public())
      );
    } else {
      println!("secp256k1 from bytes failed");
    }
  } else {
    println!("secp256k1 hex decode failed");
  }

  println!();
}

fn generate_ecdsa() {
  // Generate a keypair
  let keypair = identity::Keypair::generate_ecdsa();

  // Derive the PeerId from the keypair
  let peer_id = PeerId::from(keypair.public());

  println!("Generated ecdsa PeerId: {:?}", peer_id);

  let private_key = keypair.to_protobuf_encoding().unwrap();
  let hex_string = hex::encode(private_key);

  println!("ecdsa : {}", hex_string);

  if let Ok(data) = hex::decode(hex_string) {
    if let Ok(keypair) = identity::Keypair::from_protobuf_encoding(&data) {
      println!(
        "from string ecdsa PeerId: {:?}",
        PeerId::from(keypair.public())
      );
    } else {
      println!("ecdsa from bytes failed");
    }
  } else {
    println!("ecdsa hex decode failed");
  }
  println!();
}

// Generated ed25519 PeerId: PeerId("12D3KooWDsFm6sQ3EPGmFQkv3xkVwKK12m1PaEgUuCUvaHVUFBw6")
// ed25519 private key in hex: 08011240b119784c5e448588bfd780e9fb73a992bc55156b67b7e8e9fe892c38f3248c853c2c3eb8979aa9e059a7132a320312584aa75441ec25e75dabe65d2ead2ee989
// from string ed25519 PeerId: PeerId("12D3KooWDsFm6sQ3EPGmFQkv3xkVwKK12m1PaEgUuCUvaHVUFBw6")

// Generated secp256k1 PeerId: PeerId("16Uiu2HAmJQii4E93Jpb8waH6YTjNiZeYVRBfCagNRT6RtQ1eqkK8")
// secp256k1 : 08021220c370059db892bdbfb421ad2cccca2d00c0415665645e9c33b2e4538b013ad012
// from string secp256k1 PeerId: PeerId("16Uiu2HAmJQii4E93Jpb8waH6YTjNiZeYVRBfCagNRT6RtQ1eqkK8")

// Generated ecdsa PeerId: PeerId("QmWKp3jfkrpg8Rp7j1DCZGs5DzgXxsnnmLR97jADKwqH7F")
// ecdsa : 08031279307702010104205c4bf9f43a2efb1bcb17828e9a3a6eb00ed8981fbc3a43edc61cc74f4de79d7aa00a06082a8648ce3d030107a1440342000436e73e7c3e9dec072b8ab9a2d4a6522b4cb009c090ef3881092ad44b0d2582bd7e2b5f59286d0b11df47da9f79ba1087b48a19f5683fe87e9a7388846759ab03
// from string ecdsa PeerId: PeerId("QmWKp3jfkrpg8Rp7j1DCZGs5DzgXxsnnmLR97jADKwqH7F")
