use std::fmt::format;
use std::path::PathBuf;
use std::{env, fs};

use base64::decode;
use rsa::Hash::SHA2_512;
use rsa::{pkcs8::DecodePrivateKey, PaddingScheme, RsaPrivateKey};
use sha2::{Digest, Sha512};

use crate::utils::errors::SomeError;

pub fn load_key(key_file: Option<PathBuf>) -> Result<RsaPrivateKey, Box<dyn std::error::Error>> {
    let mut pem: String;

    if let Some(_path) = key_file {
        pem = fs::read_to_string(_path)?;
    } else {
        let b64key = env::var("UPDATER_PRIVATE_KEY")?;
        let decoded = base64::decode(b64key)?;
        pem = String::from_utf8(decoded)?;
    }

    Ok(RsaPrivateKey::from_pkcs8_pem(pem.as_str())?)
}

pub fn sign_file(key: &RsaPrivateKey, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Create digest
    let data = fs::read(path)?;
    let mut hasher = sha2::Sha512::new();
    hasher.update(data);
    let res = hasher.finalize();
    let pad = PaddingScheme::PKCS1v15Sign { hash: Some(SHA2_512) };
    let signature = key.sign(pad, &res)?;

    let new_ext = format!("{}.sig", path.extension().unwrap().to_str().unwrap());
    let signature_file = path.with_extension(new_ext);
    fs::write(signature_file, signature)?;

    Ok(())
}

#[cfg(test)]
mod rsa_tests {
    use super::*;
    use crate::utils::hash::hash_file;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn test_rsa_sign() {
        let key_path = PathBuf::from("extra/test_files/privatekey.pem");
        let file_path = PathBuf::from("extra/test_files/in.txt");
        let signature_path = PathBuf::from("extra/test_files/in.txt.sig");

        // Try with key file
        let key = load_key(Some(key_path.to_owned())).unwrap();
        sign_file(&key, &file_path);
        let finfo = hash_file(&signature_path);
        assert_eq!(finfo.hash, "4aae469c5a90903a40f1757c7b50d38c5ddfb364");

        // Try with env var
        let b64_key = base64::encode(fs::read(key_path).unwrap());
        env::set_var("UPDATER_PRIVATE_KEY", b64_key);

        let key = load_key(None).unwrap();
        sign_file(&key, &file_path);
        let finfo = hash_file(&signature_path);
        assert_eq!(finfo.hash, "4aae469c5a90903a40f1757c7b50d38c5ddfb364");
    }
}
