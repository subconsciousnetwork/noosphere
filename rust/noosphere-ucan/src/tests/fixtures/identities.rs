use crate::crypto::KeyMaterial;
use crate::key_material::ed25519::Ed25519KeyMaterial;
use base64::Engine;
use ed25519_dalek::SigningKey as Ed25519PrivateKey;

fn base64_to_key_material(string: &str) -> anyhow::Result<Ed25519KeyMaterial> {
    let mut input: [u8; 64] = [0; 64];
    input.copy_from_slice(
        base64::engine::general_purpose::STANDARD
            .decode(string.as_bytes())?
            .as_slice(),
    );
    let private_key = Ed25519PrivateKey::from_keypair_bytes(&mut input)?;
    Ok(Ed25519KeyMaterial(
        private_key.verifying_key(),
        Some(private_key),
    ))
}

pub struct Identities {
    pub alice_key: Ed25519KeyMaterial,
    pub bob_key: Ed25519KeyMaterial,
    pub mallory_key: Ed25519KeyMaterial,

    pub alice_did: String,
    pub bob_did: String,
    pub mallory_did: String,
}

/// An adaptation of the fixtures used in the canonical ts-ucan repo
/// See: https://github.com/ucan-wg/ts-ucan/blob/main/tests/fixtures.ts
impl Identities {
    pub async fn new() -> Self {
        let alice_keypair = base64_to_key_material("U+bzp2GaFQHso587iSFWPSeCzbSfn/CbNHEz7ilKRZ1UQMmMS7qq4UhTzKn3X9Nj/4xgrwa+UqhMOeo4Ki8JUw==").unwrap();
        let bob_keypair = base64_to_key_material("G4+QCX1b3a45IzQsQd4gFMMe0UB1UOx9bCsh8uOiKLER69eAvVXvc8P2yc4Iig42Bv7JD2zJxhyFALyTKBHipg==").unwrap();
        let mallory_keypair = base64_to_key_material("LR9AL2MYkMARuvmV3MJV8sKvbSOdBtpggFCW8K62oZDR6UViSXdSV/dDcD8S9xVjS61vh62JITx7qmLgfQUSZQ==").unwrap();

        Identities {
            alice_did: alice_keypair.get_did().await.unwrap(),
            bob_did: bob_keypair.get_did().await.unwrap(),
            mallory_did: mallory_keypair.get_did().await.unwrap(),

            alice_key: alice_keypair,
            bob_key: bob_keypair,
            mallory_key: mallory_keypair,
        }
    }

    #[allow(dead_code)]
    pub fn name_for(&self, did: String) -> String {
        match did {
            _ if did == self.alice_did => "alice".into(),
            _ if did == self.bob_did => "bob".into(),
            _ if did == self.mallory_did => "mallory".into(),
            _ => did,
        }
    }
}
