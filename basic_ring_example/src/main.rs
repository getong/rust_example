use ring::error::Unspecified;
use ring::rand::SystemRandom;
// use ring::signature;
use ring::signature::EcdsaKeyPair;
use ring::signature::Ed25519KeyPair;
use ring::signature::KeyPair;
use ring::signature::UnparsedPublicKey;
use ring::signature::ECDSA_P256_SHA256_ASN1;
use ring::signature::ECDSA_P256_SHA256_ASN1_SIGNING;
use ring::signature::ED25519;

fn main() -> Result<(), Unspecified> {
    // generate a new ECDSA key pair
    let rand = SystemRandom::new();
    let pkcs8_bytes = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rand)?; // pkcs8 format used for persistent storage
    let key_pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8_bytes.as_ref())
        .map_err(|_| Unspecified)?;

    // create a message and sign using the key pair
    const MESSAGE: &[u8] = b"hello, world";
    let sig = key_pair.sign(&rand, MESSAGE)?;

    // get the public key as bytes
    let peer_public_key_bytes = key_pair.public_key().as_ref();
    // verify the signature using the public key and message
    let peer_public_key = UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, peer_public_key_bytes);
    peer_public_key.verify(MESSAGE, sig.as_ref())?;

    // generate a new Ed25519 key pair
    let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&rand)?; // pkcs8 format used for persistent storage
    let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref()).map_err(|_| Unspecified)?;

    // create a message and sign using the key pair
    let sig = key_pair.sign(MESSAGE);

    // get the public key as bytes
    let peer_public_key_bytes = key_pair.public_key().as_ref();

    // verify the signature using the public key and message
    let peer_public_key = UnparsedPublicKey::new(&ED25519, peer_public_key_bytes);
    peer_public_key.verify(MESSAGE, sig.as_ref())
}

// copy from https://medium.com/@web3developer/signing-verifying-messages-with-digital-signatures-in-rust-using-ring-41f76febd7e7
