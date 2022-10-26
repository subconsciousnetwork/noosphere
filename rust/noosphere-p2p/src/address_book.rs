use anyhow::Result;
use tokio::fs;
use toml;

pub struct AddressRecord {
    pub name: String,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

pub struct AddressBook {
    addresses: Vec<AddressRecord>,
}

impl AddressBook {
    pub fn new(addresses: Vec<AddressRecord>) -> Self {
        Self { addresses }
    }

    pub async fn from_path(path: &std::path::PathBuf) -> Result<AddressBook> {
        let toml_str = fs::read_to_string(path).await?;
        let parsed = toml_str.parse::<toml::Value>()?;
        if let Some(items) = parsed.as_table() {
            Ok(AddressBook::new(
                items
                    .iter()
                    .filter_map(|(name, record)| {
                        let key_opt = record.get("key").and_then(|v| v.as_str());
                        let value_opt = record.get("value").and_then(|v| v.as_str());
                        if key_opt.is_some() && value_opt.is_some() {
                            Some(AddressRecord {
                                name: name.clone(),
                                key: key_opt.unwrap().as_bytes().to_owned(),
                                value: value_opt.unwrap().as_bytes().to_owned(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<AddressRecord>>(),
            ))
        } else {
            Ok(AddressBook::default())
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &AddressRecord> + '_ {
        self.addresses.iter()
    }
}

impl Default for AddressBook {
    fn default() -> Self {
        AddressBook { addresses: vec![] }
    }
}

impl IntoIterator for AddressBook {
    type Item = AddressRecord;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.addresses.into_iter()
    }
}
