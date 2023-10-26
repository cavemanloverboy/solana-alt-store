use {
    crate::rpc_loader::load_address_lookup_tables,
    serde::{Deserialize, Serialize},
    solana_sdk::{
        address_lookup_table::state::AddressLookupTable,
        message::{
            v0::{LoadedAddresses, MessageAddressTableLookup},
            AddressLoaderError,
        },
        pubkey::Pubkey,
        transaction::AddressLoader,
    },
    std::{
        collections::HashMap,
        error::Error,
        fs::File,
        io::{BufReader, BufWriter},
        path::{Path, PathBuf},
    },
};

/// Store for ALT data by Pubkey.
#[derive(Clone, Debug)]
pub struct Store {
    path: PathBuf,
    inner: StoreInner,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoreInner(HashMap<Pubkey, Vec<u8>>);

// IO operations
impl Store {
    pub fn load_or_create(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let path = path.as_ref().to_path_buf();
        if path.exists() {
            Self::load_from_path(path)
        } else {
            Self::new_with_path(path)
        }
    }

    /// Create a new Store at the given path, assuming it does not already exist.
    fn new_with_path(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        std::fs::write(path.as_ref(), &[])?; // Create the file
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            inner: StoreInner(HashMap::new()),
        })
    }

    /// Load a Store from the given path, assuming it already exists.
    fn load_from_path(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            inner: {
                let file = File::open(&path)?;
                let reader = BufReader::new(file);
                bincode::deserialize_from(reader)?
            },
        })
    }

    /// Save the Store to disk.
    pub fn save_to_path(&self) -> Result<(), Box<dyn Error>> {
        let file = File::options().write(true).append(false).open(&self.path)?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, &self.inner)?;
        Ok(())
    }
}

/// How to handle store w/ already present data.
#[derive(Default)]
pub enum UpdateMode {
    /// Only load data for addresses not already present in the store.
    #[default]
    Append,
    /// Update store with new data, regardless of existing data.
    Overwrite,
}

// Update operations
impl Store {
    /// Check if the Store contains data for the given Pubkey.
    pub fn contains_key(&self, pubkey: &Pubkey) -> bool {
        self.inner.0.contains_key(pubkey)
    }

    /// Fetch and update the Store with new data for the given Pubkeys.
    pub fn update(
        &mut self,
        pubkeys: &[Pubkey],
        update_mode: UpdateMode,
    ) -> Result<(), Box<dyn Error>> {
        let fetch_pubkeys: Vec<_> = match update_mode {
            UpdateMode::Append => pubkeys
                .iter()
                .filter(|pubkey| !self.contains_key(pubkey))
                .cloned()
                .collect(),
            UpdateMode::Overwrite => pubkeys.to_vec(),
        };

        if !fetch_pubkeys.is_empty() {
            let fetched_alt_data = load_address_lookup_tables(&fetch_pubkeys)?;
            for (pubkey, data) in fetched_alt_data {
                self.insert_table_data(pubkey, data);
            }
            self.save_to_path()?;
        }

        Ok(())
    }

    /// Insert new data into the Store.
    fn insert_table_data(&mut self, pubkey: Pubkey, data: Vec<u8>) {
        self.inner.0.insert(pubkey, data);
    }
}

impl AddressLoader for &Store {
    fn load_addresses(
        self,
        lookups: &[MessageAddressTableLookup],
    ) -> Result<LoadedAddresses, AddressLoaderError> {
        let mut writable = vec![];
        let mut readonly = vec![];

        for lookup in lookups {
            let Some(data) = self.inner.0.get(&lookup.account_key) else {
                return Err(AddressLoaderError::LookupTableAccountNotFound);
            };

            let alt = AddressLookupTable::deserialize(data)
                .map_err(|_| AddressLoaderError::InvalidAccountData)?;

            for index in &lookup.writable_indexes {
                writable.push(
                    *alt.addresses
                        .get(*index as usize)
                        .ok_or(AddressLoaderError::InvalidLookupIndex)?,
                );
            }

            for index in &lookup.readonly_indexes {
                readonly.push(
                    *alt.addresses
                        .get(*index as usize)
                        .ok_or(AddressLoaderError::InvalidLookupIndex)?,
                );
            }
        }

        Ok(LoadedAddresses { writable, readonly })
    }
}
