//! 多账号配置：持久化保存每个邮箱的连接参数（授权码单独存放在 Windows 凭据管理器）。

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

const ACCOUNTS_FILE: &str = "mail-accounts.json";

static ACCOUNTS_PATH: OnceLock<PathBuf> = OnceLock::new();

/// 单个邮箱账号的连接配置。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MailAccountProfile {
    pub(super) email: String,
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) proxy: String,
    pub(super) mail_type: String,
}

/// 在 init 时记录账号配置文件的绝对路径。
pub(super) fn configure_accounts_dir(app_dir: &Path) {
    let _ = ACCOUNTS_PATH.set(app_dir.join(ACCOUNTS_FILE));
}

fn accounts_path() -> PathBuf {
    ACCOUNTS_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from(ACCOUNTS_FILE))
}

/// 读取全部已保存账号；文件缺失或损坏时返回空列表。
pub(super) fn load_accounts() -> Vec<MailAccountProfile> {
    match fs::read_to_string(accounts_path()) {
        Ok(content) => serde_json::from_str::<Vec<MailAccountProfile>>(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn persist_accounts(accounts: &[MailAccountProfile]) -> Result<(), String> {
    let path = accounts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "无法创建邮箱配置目录。")?;
    }
    let content = serde_json::to_string_pretty(accounts).map_err(|_| "无法序列化邮箱配置。")?;
    fs::write(path, content).map_err(|_| "无法保存邮箱账号配置。")?;
    Ok(())
}

/// 新增或更新一个账号（按 email 去重），并返回最新账号列表。
pub(super) fn upsert_account(profile: MailAccountProfile) -> Result<Vec<MailAccountProfile>, String> {
    let mut accounts = load_accounts();
    if let Some(existing) = accounts.iter_mut().find(|item| item.email == profile.email) {
        *existing = profile;
    } else {
        accounts.push(profile);
    }
    persist_accounts(&accounts)?;
    Ok(accounts)
}

/// 删除指定账号配置，返回（最新列表, 被删账号是否存在）。
pub(super) fn remove_account(email: &str) -> Result<(Vec<MailAccountProfile>, bool), String> {
    let mut accounts = load_accounts();
    let existed = accounts.iter().any(|item| item.email == email);
    accounts.retain(|item| item.email != email);
    persist_accounts(&accounts)?;
    Ok((accounts, existed))
}

/// 读取单个账号配置。
pub(super) fn find_account(email: &str) -> Option<MailAccountProfile> {
    load_accounts().into_iter().find(|item| item.email == email)
}
