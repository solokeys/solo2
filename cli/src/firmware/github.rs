//! Micro-client to fetch GitHub release assets
//!
//! We'd use one of the many existing clients, if only there were a non-async one.
use std::io::Read as _;

use anyhow::anyhow;
use serde_json::{from_value, Value};

use super::{Firmware, Result};

/// An asset that can be downloaded from GitHub
#[derive(Clone, Debug)]
pub struct AssetSpec {
    pub name: String,
    pub url: String,
    pub len: usize,
}

impl TryFrom<Value> for AssetSpec {
    type Error = crate::Error;
    fn try_from(value: Value) -> Result<Self> {
        let name: String = from_value(value["name"].clone())?;
        let url: String = from_value(value["browser_download_url"].clone())?;
        let len: usize = from_value(value["size"].clone())?;
        Ok(Self { name, url, len })
    }
}

impl AssetSpec {
    /// Attempt to download an asset from GitHub
    pub fn fetch_asset(&self) -> Result<Vec<u8>> {
        let reader = ureq::get(&self.url)
            .set("User-Agent", "solo2-cli")
            .call()?
            .into_reader();

        let pb = indicatif::ProgressBar::new(self.len as _);
        let mut buffer = Vec::new();
        pb.wrap_read(reader).read_to_end(&mut buffer)?;
        if self.len == buffer.len() {
            Ok(buffer)
        } else {
            Err(anyhow!("Truncated download from {}", &self.url))
        }
    }
}

/// A very specific set of assets on GitHub, presumed to contain
/// an SB2.1 firmware file and a corresponding SHA-256 digest of the contents.
#[derive(Clone, Debug)]
pub struct Release {
    pub tag: String,
    pub assets: Vec<AssetSpec>,
}

impl Release {
    const URL_LATEST: &'static str = "https://api.github.com/repos/solokeys/solo2/releases/latest";
    const HASH_TEMPLATE: &'static str = "solo2-firmware-{}.sha2";
    const SB2_TEMPLATE: &'static str = "solo2-firmware-{}.sb2";

    pub fn fetch_spec() -> Result<Self> {
        let response: Value = ureq::get(Self::URL_LATEST)
            .set("User-Agent", "solo2-cli")
            .call()?
            .into_json()?;
        let tag: String = from_value(response["tag_name"].clone())?;

        let assets: Vec<Value> = from_value(response["assets"].clone())?;
        let assets: Vec<AssetSpec> = assets
            .into_iter()
            .map(AssetSpec::try_from)
            .filter_map(|x| x.ok())
            .collect();

        Ok(Self { tag, assets })
    }

    pub fn fetch_hash(&self) -> Result<String> {
        let spec = self.assets.iter()
            // poor man's format!
            .find(|asset| asset.name == Self::HASH_TEMPLATE.replace("{}", &self.tag))
            .ok_or_else(|| anyhow!("Unable to find hash digest in latest SoloKeys release. Please open ticket on solokeys.com/solo2 or contact hello@solokeys.com."))?;

        let hash_data = &spec.fetch_asset()?;
        let hash = std::str::from_utf8(hash_data)
            .map_err(|_| anyhow!("Invalid hash digest in latest SoloKeys release. Please open ticket on solokeys.com/solo2 or contact hello@solokeys.com."))?;
        let hash = hash.split_whitespace().next().unwrap().to_string();
        Ok(hash)
    }

    pub fn fetch_firmware(&self) -> Result<Firmware> {
        let spec = self.assets.iter()
            // poor man's format!
            .find(|asset| asset.name == Self::SB2_TEMPLATE.replace("{}", &self.tag))
            .ok_or_else(|| anyhow!("Unable to find firmware SB2 file in latest SoloKeys release. Please open ticket on solokeys.com/solo2 or contact hello@solokeys.com."))?;

        let firmware = Firmware::new(spec.fetch_asset()?)?;
        firmware.verify_hexhash(&self.fetch_hash()?)?;
        Ok(firmware)
    }
}
