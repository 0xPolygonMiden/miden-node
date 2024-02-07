use anyhow::{anyhow, Result};
use miden_objects::{accounts::AccountData, utils::serde::Deserializable};
use std::{env, fs, path::PathBuf, str::FromStr};

pub fn import_account_from_args() -> Result<AccountData> {
    let args: Vec<String> = env::args().collect();

    let path = match args.get(1) {
        Some(s) => match PathBuf::from_str(s) {
            Ok(path) => path,
            Err(e) => return Err(anyhow!("Failed to turn string to path: {e}")),
        },
        None => return Err(anyhow!("Invalid file path")),
    };

    let account_data_file_contents =
        fs::read(path).map_err(|e| anyhow!("Failed to read file: {e}"))?;
    let account_data = AccountData::read_from_bytes(&account_data_file_contents)
        .map_err(|e| anyhow!("Failed to deserialize file: {e}"))?;

    Ok(account_data)
}
