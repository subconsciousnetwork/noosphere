use anyhow::{anyhow, Result};
use serde_json::json;
use tokio::fs;
use ucan::crypto::KeyMaterial;

use noosphere::authority::{ed25519_key_to_mnemonic, generate_ed25519_key};

use crate::native::workspace::Workspace;

pub static SERVICE_NAME: &str = "noosphere";

pub async fn create_key(name: String, working_paths: &Workspace) -> Result<()> {
    working_paths.initialize_global_directories().await?;

    let key_base_path = working_paths.keys_path().join(&name);
    let private_key_path = key_base_path.with_extension("private");

    if private_key_path.exists() {
        return Err(anyhow!("A key called {:?} already exists!", name));
    }

    let did_path = key_base_path.with_extension("public");

    let key_pair = generate_ed25519_key();
    let did = key_pair.get_did().await?;

    let mnemonic = ed25519_key_to_mnemonic(&key_pair)?;

    tokio::try_join!(
        fs::write(private_key_path, mnemonic),
        fs::write(did_path, &did),
    )?;

    println!("Created key {:?} in {:?}", name, working_paths.keys_path());
    println!("Public identity {}", did);

    Ok(())
}

pub async fn list_keys(as_json: bool, working_paths: &Workspace) -> Result<()> {
    if let Err(error) = working_paths.expect_global_directories() {
        return Err(anyhow!(
            "{:?}\nTip: you may need to create a key first",
            error
        ));
    }

    let keys = working_paths.get_all_keys().await?;
    let max_name_length = keys
        .iter()
        .fold(7, |length, (key_name, _)| key_name.len().max(length));

    if as_json {
        println!("{}", serde_json::to_string_pretty(&json!(keys))?);
    } else {
        println!("{:1$}  IDENTITY", "NAME", max_name_length);
        for (name, did) in keys {
            println!("{:1$}  {did}", name, max_name_length);
        }
    }

    Ok(())
}

// use webauthn_rs::prelude::*;
// use webauthn_rs::WebauthnBuilder;
// use webauthn_rs_proto::COSEAlgorithm;
// use webauthn_rs_proto::PubKeyCredParams;

// use webauthn_authenticator_rs::u2fhid::U2FHid;
// use webauthn_authenticator_rs::WebauthnAuthenticator;

// let webauthn = WebauthnBuilder::new("noosphere", &Url::parse("https://cli.noosphere")?)?
//         // .allow_subdomains(true)
//         // .rp_name("Noosphere")
//         .build()?;
//     let id = Uuid::new_v4();
//     println!("Using ID: {:?}", id);

//     let (mut creation_challenge_response, registration) =
//         webauthn.start_passkey_registration(id.clone(), &name, &name, None)?;

//     creation_challenge_response.public_key.pub_key_cred_params = vec![PubKeyCredParams {
//         type_: "public-key".into(),
//         alg: COSEAlgorithm::EDDSA as i64,
//     }];

//     // println!("{:#?}", creation_challenge_response);
//     // println!("{:#?}", registration);

//     let mut authenticator = WebauthnAuthenticator::new(U2FHid::default());

//     let result = match authenticator.do_registration(
//         Url::parse("https://cli.noosphere")?,
//         creation_challenge_response,
//     ) {
//         Ok(result) => Ok(result),
//         Err(error) => Err(anyhow!("{:?}", error)),
//     }?;

//     let passkey = webauthn.finish_passkey_registration(&result, &registration)?;

//     println!("{:?}", passkey);
//     // println!("RESULT: {:#?}", result);

//     Ok(())

// use ctap_hid_fido2::FidoKeyHid;
// use ctap_hid_fido2::fidokey;
// use ctap_hid_fido2::public_key_credential_user_entity::PublicKeyCredentialUserEntity;
// use ctap_hid_fido2::verifier;
// use ctap_hid_fido2::Cfg;
// use ctap_hid_fido2::FidoKeyHidFactory;

// println!("Enumerate HID devices.");
// let devs = ctap_hid_fido2::get_fidokey_devices();
// for info in devs {
//     println!(
//         "- vid=0x{:04x} , pid=0x{:04x} , info={:?}",
//         info.vid, info.pid, info.info
//     );
// }

// // let key = fidokey::CredentialSupportedKeyType::
// // let user_entity = PublicKeyCredentialUserEntity::new(None, Some(name.as_str()), None);
// let challenge = verifier::create_challenge();
// // challenge.fill(8u8);
// println!("Challenge: {:?}", challenge);

// let fido_args = fidokey::MakeCredentialArgsBuilder::new("noosphere.com", &challenge)
//     // .key_type(fidokey::CredentialSupportedKeyType::default())
//     .key_type(fidokey::CredentialSupportedKeyType::Ed25519)
//     .user_entity(&PublicKeyCredentialUserEntity::new(
//         Some(name.as_bytes()),
//         Some(&name),
//         Some(&name),
//     ))
//     .resident_key()
//     .build();

// println!("FIDO args: {:#?}", fido_args);

// // println!("Config... {:?}", Cfg::init());
// let mut cfg = Cfg::init();
// cfg.enable_log = true;

// let device = FidoKeyHidFactory::create(&cfg)?;

// println!("Device...");
// let attestation = device.make_credential_with_args(&fido_args)?;

// println!("Attestation: {:#?}", attestation);

// let key_material = generate_ed25519_key();
// let did = key_material.get_did().await?;

// let private_key = key_material
//     .1
//     .ok_or_else(|| anyhow!("Unable to access generated private key"))?;

// let entry = keyring::Entry::new_with_target(&name, SERVICE_NAME, &username());

// entry.set_password(&base64_encode(private_key.as_ref())?)?;

// println!(
//     "New key '{}' generated and stored in a platform keyring",
//     name
// );
// println!("{}", did);
