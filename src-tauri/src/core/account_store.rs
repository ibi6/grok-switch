use crate::core::paths::{atomic_write, Paths};
use crate::core::types::Account;
use crate::core::AppError;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountsIndex {
    version: u32,
    items: Vec<Account>,
}

impl Default for AccountsIndex {
    fn default() -> Self {
        Self {
            version: 1,
            items: Vec::new(),
        }
    }
}

fn read_index(paths: &Paths) -> Result<AccountsIndex, AppError> {
    let index_path = paths.accounts_index();
    if !index_path.exists() {
        return Ok(AccountsIndex::default());
    }
    let raw = fs::read_to_string(index_path)?;
    let index: AccountsIndex = serde_json::from_str(&raw)?;
    Ok(index)
}

fn write_index(paths: &Paths, index: &AccountsIndex) -> Result<(), AppError> {
    paths.ensure_app_dirs()?;
    let json = serde_json::to_string_pretty(index)?;
    atomic_write(&paths.accounts_index(), json)?;
    Ok(())
}

pub fn list_accounts(paths: &Paths) -> Result<Vec<Account>, AppError> {
    Ok(read_index(paths)?.items)
}

pub fn get_account(paths: &Paths, id: &str) -> Result<Option<Account>, AppError> {
    Ok(read_index(paths)?
        .items
        .into_iter()
        .find(|a| a.id == id))
}

/// Save account meta into `accounts/<id>/meta.json` and update `accounts/index.json`.
pub fn save_account_meta(paths: &Paths, account: Account) -> Result<(), AppError> {
    paths.ensure_app_dirs()?;
    let dir = paths.account_dir(&account.id);
    fs::create_dir_all(&dir)?;

    let meta_json = serde_json::to_string_pretty(&account)?;
    atomic_write(&paths.account_meta(&account.id), meta_json)?;

    let mut index = read_index(paths)?;
    if let Some(existing) = index.items.iter_mut().find(|a| a.id == account.id) {
        *existing = account;
    } else {
        index.items.push(account);
    }
    write_index(paths, &index)
}

/// Remove `accounts/<id>/` directory and drop the entry from the index.
pub fn delete_account_dir(paths: &Paths, id: &str) -> Result<bool, AppError> {
    let mut index = read_index(paths)?;
    let before = index.items.len();
    index.items.retain(|a| a.id != id);
    let removed_from_index = index.items.len() != before;

    let dir = paths.account_dir(id);
    let dir_existed = dir.exists();
    if dir_existed {
        fs::remove_dir_all(&dir)?;
    }

    if removed_from_index {
        write_index(paths, &index)?;
    }

    Ok(removed_from_index || dir_existed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::AccountStatus;

    fn sample_account(id: &str, name: &str) -> Account {
        Account {
            id: id.into(),
            name: name.into(),
            email: Some("u@example.com".into()),
            label_color: "#3b82f6".into(),
            status: AccountStatus::Ready,
            last_used_at: None,
            created_at: 100,
        }
    }

    #[test]
    fn account_meta_roundtrip_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();

        assert!(list_accounts(&paths).unwrap().is_empty());

        save_account_meta(&paths, sample_account("acc1", "Work")).unwrap();
        assert!(paths.account_meta("acc1").is_file());
        assert_eq!(list_accounts(&paths).unwrap().len(), 1);

        save_account_meta(&paths, sample_account("acc1", "Work Renamed")).unwrap();
        let list = list_accounts(&paths).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Work Renamed");

        save_account_meta(&paths, sample_account("acc2", "Personal")).unwrap();
        assert_eq!(list_accounts(&paths).unwrap().len(), 2);

        assert!(delete_account_dir(&paths, "acc1").unwrap());
        assert!(!paths.account_dir("acc1").exists());
        let remaining = list_accounts(&paths).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "acc2");
    }
}
