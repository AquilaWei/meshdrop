//! Certificate identities and durable explicit trust, independent of discovery.
use anyhow::{Context, Result, bail};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
/// Persistent private identity. The containing directory must remain private.
#[derive(Serialize, Deserialize)]
pub struct Identity {
    certificate: Vec<u8>,
    key: Vec<u8>,
}
impl Identity {
    /// Generates a self-signed identity once. Corrupt existing identities fail
    /// rather than silently rotating the device fingerprint.
    pub fn load(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory)?;
        let path = directory.join("identity.json");
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).context("invalid stored identity"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let generated = rcgen::generate_simple_self_signed(vec!["meshdrop".into()])?;
                let identity = Self {
                    certificate: generated.cert.der().to_vec(),
                    key: generated.signing_key.serialize_der(),
                };
                let mut file = tempfile::NamedTempFile::new_in(directory)?;
                use std::io::Write;
                file.write_all(&serde_json::to_vec(&identity)?)?;
                file.as_file().sync_all()?;
                file.persist_noclobber(path)?;
                Ok(identity)
            }
            Err(e) => Err(e.into()),
        }
    }
    /// SHA-256 of the certificate DER identifies the device without a central CA.
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.certificate)
    }
    pub(crate) fn certificate(&self) -> CertificateDer<'static> {
        self.certificate.clone().into()
    }
    pub(crate) fn key(&self) -> PrivateKeyDer<'static> {
        PrivatePkcs8KeyDer::from(self.key.clone()).into()
    }
}
/// Computes a stable certificate pin; never use the advertised device name as identity.
pub fn fingerprint(der: &[u8]) -> String {
    hex::encode(Sha256::digest(der))
}
/// Address-to-fingerprint bindings prevent silently accepting rotated certificates.
#[derive(Default, Serialize, Deserialize)]
pub struct TrustStore(pub BTreeMap<String, String>);
impl TrustStore {
    /// Loads paired identities; malformed files are errors, never empty trust.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    /// Persists confirmation atomically. Existing conflicting bindings fail closed.
    pub fn remember(&mut self, path: &Path, address: &str, pin: &str) -> Result<()> {
        if self.0.get(address).is_some_and(|old| old != pin) {
            bail!("fingerprint mismatch for {address}");
        }
        let mut updated = self.0.clone();
        updated.insert(address.into(), pin.into());
        let mut file =
            tempfile::NamedTempFile::new_in(path.parent().context("trust path has no parent")?)?;
        use std::io::Write;
        file.write_all(&serde_json::to_vec_pretty(&updated)?)?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        self.0 = updated;
        Ok(())
    }
    /// Unknown peers and changed pins produce distinct actionable errors.
    pub fn verify(&self, address: &str, pin: &str) -> Result<()> {
        match self.0.get(address) {
            Some(expected) if expected == pin => Ok(()),
            Some(_) => bail!("fingerprint mismatch for {address}"),
            None => bail!("unpaired device {address}"),
        }
    }
}
/// Both peers derive the same short authentication string from their certificates.
pub fn short_code(first: &str, second: &str) -> String {
    let joined = if first < second {
        format!("{first}:{second}")
    } else {
        format!("{second}:{first}")
    };
    let hash = Sha256::digest(joined.as_bytes());
    format!(
        "{:06}",
        u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]) % 1_000_000
    )
}
