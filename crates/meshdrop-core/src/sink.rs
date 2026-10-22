//! Safe receive storage; temporary files never become visible as completed downloads.
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::{Builder, NamedTempFile};

/// Normalize untrusted names across platforms. Rejects empty names and extensions
/// too long to preserve within the 200-byte component limit.
pub fn sanitize(name: &str) -> Result<String> {
    let leaf = name.rsplit(['/', '\\']).next().unwrap_or_default();
    if matches!(leaf, "" | "." | "..") {
        bail!("invalid empty or relative filename");
    }
    let mut clean: String = leaf
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    clean = clean.trim_end_matches(['.', ' ']).to_owned();
    if clean.is_empty() {
        bail!("filename is empty after normalization");
    }
    let base = clean.split('.').next().unwrap_or_default().to_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|p| {
            base.strip_prefix(p).is_some_and(|n| {
                matches!(
                    n,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
    if reserved {
        clean.insert(0, '_');
    }
    truncate(&clean, 200)
}
fn truncate(name: &str, limit: usize) -> Result<String> {
    if name.len() <= limit {
        return Ok(name.to_owned());
    }
    let (stem, ext) = split(name);
    if ext.len() >= limit {
        bail!("extension exceeds filename limit");
    }
    let mut end = limit - ext.len();
    while !stem.is_char_boundary(end) {
        end -= 1;
    }
    Ok(format!("{}{ext}", &stem[..end]))
}
fn split(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}
/// Owns a hidden temporary file. Drop or any failed verification removes it.
/// Completion uses the filesystem's no-clobber persistence operation.
pub struct Sink {
    temp: NamedTempFile,
    directory: PathBuf,
    name: String,
    hash: Sha256,
    size: u64,
}
impl Sink {
    /// Creates a private temporary file in the destination filesystem. Fails for
    /// invalid names, missing directories, or filesystem permission errors.
    pub fn new(directory: &Path, name: &str) -> Result<Self> {
        Ok(Self {
            temp: Builder::new()
                .prefix(".meshdrop-")
                .suffix(".part")
                .tempfile_in(directory)?,
            directory: directory.into(),
            name: sanitize(name)?,
            hash: Sha256::new(),
            size: 0,
        })
    }
    /// Writes one bounded chunk before the caller reads another, preserving backpressure.
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.temp.write_all(bytes)?;
        self.hash.update(bytes);
        self.size += bytes.len() as u64;
        Ok(())
    }
    /// Verifies bytes and publishes atomically without replacing existing files.
    /// Size/digest mismatch or 10,000 occupied candidates removes the temporary
    /// file and returns an error rather than retrying indefinitely.
    pub fn finish(self, size: u64, digest: &str) -> Result<PathBuf> {
        self.finish_if(size, digest, || true)
    }
    // Recheck cancellation after slow sync_all and before each publication attempt.
    pub(crate) fn finish_if(
        mut self,
        size: u64,
        digest: &str,
        permitted: impl Fn() -> bool,
    ) -> Result<PathBuf> {
        if self.size != size || hex::encode(self.hash.clone().finalize()) != digest {
            bail!("SHA-256 or size mismatch");
        }
        if !permitted() {
            bail!("transfer cancelled");
        }
        self.temp.as_file_mut().sync_all()?;
        for number in 0..10_000 {
            let name = if number == 0 {
                self.name.clone()
            } else {
                let suffix = format!(" ({number})");
                let shortened = truncate(&self.name, 200 - suffix.len())?;
                let (short_stem, short_ext) = split(&shortened);
                format!("{short_stem}{suffix}{short_ext}")
            };
            if !permitted() {
                bail!("transfer cancelled");
            }
            let destination = self.directory.join(name);
            match self.temp.persist_noclobber(&destination) {
                Ok(_) => return Ok(destination),
                Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    self.temp = e.file;
                }
                Err(e) => return Err(e.error.into()),
            }
        }
        bail!("filename collision limit (10000 candidates) exhausted")
    }
}
