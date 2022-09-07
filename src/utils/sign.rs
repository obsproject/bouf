use std::path::PathBuf;
use std::{env, fs};

use anyhow::Result;
use rsa::Hash::SHA2_512;
use rsa::{pkcs8::DecodePrivateKey, PaddingScheme, RsaPrivateKey};
use sha2::Digest;

#[derive(Default)]
pub struct Signer<'a> {
    key_file: Option<&'a PathBuf>,
    private_key: Option<RsaPrivateKey>,
}

impl<'a> Signer<'a> {
    pub fn init(key_file: Option<&'a PathBuf>) -> Self {
        Self {
            key_file,
            ..Default::default()
        }
    }

    fn load_key(&mut self) -> Result<()> {
        let pem: String;

        if let Some(_path) = &self.key_file {
            pem = fs::read_to_string(_path)?;
        } else {
            let b64key = env::var("UPDATER_PRIVATE_KEY")?;
            let decoded = base64::decode(b64key)?;
            pem = String::from_utf8(decoded)?;
        }

        let pkey = RsaPrivateKey::from_pkcs8_pem(pem.as_str())?;
        self.private_key = Some(pkey);

        Ok(())
    }

    pub fn sign_file(&mut self, path: &PathBuf) -> Result<()> {
        if self.private_key.is_none() {
            self.load_key()?
        }

        // Create digest
        let data = fs::read(path)?;
        let mut hasher = sha2::Sha512::new();
        hasher.update(data);
        let res = hasher.finalize();
        let pad = PaddingScheme::PKCS1v15Sign { hash: Some(SHA2_512) };
        let signature = self.private_key.as_ref().unwrap().sign(pad, &res)?;

        let new_ext = format!("{}.sig", path.extension().unwrap().to_str().unwrap());
        let signature_file = path.with_extension(new_ext);
        fs::write(signature_file, signature)?;

        Ok(())
    }

    pub fn check_key(key_file: Option<&'a PathBuf>) -> Result<()> {
        let mut signer = Self {
            key_file,
            ..Default::default()
        };

        signer.load_key()
    }
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
        let mut signer = Signer::init(Some(&key_path));
        signer.sign_file(&file_path).expect("Signing failed");
        let finfo = hash_file(&signature_path);
        assert_eq!(finfo.hash, "4aae469c5a90903a40f1757c7b50d38c5ddfb364");

        // Try with env var
        let b64_key = base64::encode(fs::read(key_path).unwrap());
        env::set_var("UPDATER_PRIVATE_KEY", b64_key);

        let mut signer = Signer::init(None);
        signer.sign_file(&file_path).expect("Signing failed");
        let finfo = hash_file(&signature_path);
        assert_eq!(finfo.hash, "4aae469c5a90903a40f1757c7b50d38c5ddfb364");
    }
}
