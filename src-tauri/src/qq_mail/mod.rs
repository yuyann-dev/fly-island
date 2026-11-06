//! QQ Mail over IMAP: commands, snapshot types and the background service.

mod accounts;
mod content;
mod credentials;
mod service;
mod session;

#[cfg(test)]
mod tests;

use serde::Serialize;
use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Instant,
};
use tauri::{AppHandle, Manager};

use accounts::*;
use content::*;
use credentials::*;
use service::*;
use session::*;

const EVENT: &str = "qq-mail-changed";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailMessage {
    id: String,
    uid: u32,
    sender: String,
    subject: String,
    received_at: Option<i64>,
    unread: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailContent {
    id: String,
    content: String,
    content_type: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MailStatus {
    Disconnected,
    Connecting,
    Connected,
    Paused,
    AuthRequired,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailSnapshot {
    revision: u64,
    email: Option<String>,
    configured: bool,
    enabled: bool,
    syncing: bool,
    status: MailStatus,
    unread_count: Option<usize>,
    items: Vec<MailMessage>,
    last_synced_at: Option<i64>,
    error: Option<String>,
}

impl Default for MailSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            email: None,
            configured: false,
            enabled: false,
            syncing: false,
            status: MailStatus::Disconnected,
            unread_count: None,
            items: Vec::new(),
            last_synced_at: None,
            error: None,
        }
    }
}

pub fn init(app: &AppHandle) -> Result<(), String> {
    if SERVICE.get().is_some() {
        return Ok(());
    }
    // 记录多账号配置文件所在目录
    if let Ok(app_dir) = app.path().app_data_dir() {
        configure_accounts_dir(&app_dir);
    }
    let (sender, receiver) = mpsc::channel();
    let service = Arc::new(MailService {
        app: app.clone(),
        state: Mutex::new(MailState::default()),
        sender,
        idle_socket: Mutex::new(None),
    });
    SERVICE
        .set(service.clone())
        .map_err(|_| "QQ 邮箱服务已启动。")?;
    thread::Builder::new()
        .name("qq-mail".into())
        .spawn(move || worker(service, receiver))
        .map_err(|_| "无法启动 QQ 邮箱服务。".to_string())?;
    Ok(())
}

fn service() -> Result<&'static Arc<MailService>, String> {
    SERVICE.get().ok_or_else(|| "QQ 邮箱服务尚未就绪。".into())
}

#[tauri::command]
pub fn get_qq_mail_state() -> Result<MailSnapshot, String> {
    Ok(service()?.lock()?.snapshot.clone())
}

#[tauri::command]
pub async fn connect_qq_mail(
    email: String,
    authorization_code: String,
    imap_host: Option<String>,
    imap_port: Option<u16>,
    proxy: Option<String>,
    mail_type: Option<String>,
) -> Result<MailSnapshot, String> {
    let email = normalize_email(&email)?;
    let secret = authorization_code.trim().to_string();
    if secret.is_empty() || secret.len() > 256 || secret.chars().any(char::is_control) {
        return Err("请输入邮箱设置中生成的授权码。".into());
    }
    let proxy = proxy.unwrap_or_default().trim().to_string();
    let mail_type = mail_type.unwrap_or_else(|| "auto".to_string());
    // 未显式指定主机时，根据邮箱域名选择默认 IMAP 服务器
    let (host, port) = match (imap_host, imap_port) {
        (Some(h), Some(p)) if !h.trim().is_empty() => (h.trim().to_string(), p),
        (Some(h), None) if !h.trim().is_empty() => (h.trim().to_string(), DEFAULT_PORT),
        _ => {
            let (h, p) = default_imap_host(&email);
            (h.to_string(), p)
        }
    };
    // 保存/更新账号配置（多账号），旧账号保留在列表中
    upsert_account(MailAccountProfile {
        email: email.clone(),
        host: host.clone(),
        port,
        proxy: proxy.clone(),
        mail_type,
    })?;
    let service = service()?;
    let (generation, snapshot) = {
        let mut state = service.lock()?;
        // 连接新账号时直接切换当前连接，不再要求先断开
        state.invalidate();
        state.host = host.clone();
        state.port = port;
        state.proxy = proxy.clone();
        state.snapshot.email = Some(email.clone());
        state.snapshot.enabled = true;
        state.snapshot.syncing = true;
        state.snapshot.status = MailStatus::Connecting;
        state.snapshot.error = None;
        (state.generation, state.changed())
    };
    service.publish(snapshot, None);
    let (reply, receiver) = mpsc::channel();
    service
        .send(Work::Connect {
            generation,
            email,
            secret,
            host,
            port,
            proxy,
            reply,
        })
        .map_err(|_| "邮箱服务不可用。".to_string())?;
    await_reply(receiver).await
}

#[tauri::command]
pub async fn set_qq_mail_enabled(email: String, enabled: bool) -> Result<MailSnapshot, String> {
    let email = normalize_email(&email)?;
    tauri::async_runtime::spawn_blocking(move || {
        let service = service()?;
        let mut state = service.lock()?;
        if state
            .snapshot
            .email
            .as_ref()
            .is_some_and(|current| current != &email)
        {
            return Err("请先断开当前邮箱，再连接其他账号。".into());
        }
        if state.snapshot.email.as_ref() == Some(&email)
            && state.snapshot.configured
            && state.snapshot.enabled == enabled
        {
            return Ok(state.snapshot.clone());
        }
        let configured = read_secret(&email)?.is_some();
        state.invalidate();
        if state.host.is_empty() {
            let (h, p) = default_imap_host(&email);
            state.host = h.to_string();
            state.port = p;
        }
        state.snapshot.email = Some(email);
        state.snapshot.configured = configured;
        state.snapshot.enabled = enabled && configured;
        state.snapshot.status = if !configured {
            MailStatus::AuthRequired
        } else if enabled {
            MailStatus::Connecting
        } else {
            MailStatus::Paused
        };
        state.snapshot.error = if configured {
            None
        } else {
            Some("未找到保存的邮箱授权码，请重新连接。".into())
        };
        if enabled && configured {
            state.next_check = Some(Instant::now());
        }
        let snapshot = state.changed();
        drop(state);
        service.publish(snapshot.clone(), None);
        let _ = service.send(Work::Wake);
        Ok(snapshot)
    })
    .await
    .map_err(|_| "无法更新 QQ 邮箱连接。".to_string())?
}

#[tauri::command]
pub async fn refresh_qq_mail() -> Result<MailSnapshot, String> {
    let service = service()?;
    let Some((generation, _)) = service.begin_sync() else {
        let snapshot = service.lock()?.snapshot.clone();
        return if snapshot.syncing {
            Ok(snapshot)
        } else {
            Err("请先连接并开启 QQ 邮箱接收。".into())
        };
    };
    let (reply, receiver) = mpsc::channel();
    service
        .send(Work::Refresh { generation, reply })
        .map_err(|_| "QQ 邮箱服务不可用。".to_string())?;
    await_reply(receiver).await
}

#[tauri::command]
pub async fn read_qq_mail(message_id: String) -> Result<MailContent, String> {
    parse_message_id(&message_id)?;
    let service = service()?;
    let (generation, email) = {
        let state = service.lock()?;
        if !state.snapshot.configured || state.snapshot.status == MailStatus::AuthRequired {
            return Err("请先连接 QQ 邮箱。".into());
        }
        if !state
            .snapshot
            .items
            .iter()
            .any(|item| item.id == message_id)
        {
            return Err("邮件已不在最近列表中，请刷新收件箱。".into());
        }
        (
            state.generation,
            state.snapshot.email.clone().ok_or("请先连接 QQ 邮箱。")?,
        )
    };
    let service = service.clone();
    tauri::async_runtime::spawn_blocking(move || service.read(generation, &email, &message_id))
        .await
        .map_err(|_| "无法读取 QQ 邮件。".to_string())?
}

#[tauri::command]
pub async fn disconnect_qq_mail() -> Result<MailSnapshot, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let service = service()?;
        let active_email = service.lock()?.snapshot.email.clone();
        let mut state = service.lock()?;
        state.invalidate();
        state.snapshot.enabled = false;
        if let Some(email) = &active_email {
            if delete_secret(email).is_err() {
                state.snapshot.status = MailStatus::Paused;
                state.snapshot.error =
                    Some("已停止接收，但删除 Windows 邮箱凭据失败，请重试断开。".into());
                let snapshot = state.changed();
                drop(state);
                service.publish(snapshot, None);
                let _ = service.send(Work::Wake);
                return Err("删除邮箱凭据失败，请重试断开。".into());
            }
            // 同时从多账号列表移除
            let _ = remove_account(email);
        }
        state.host = String::new();
        state.port = DEFAULT_PORT;
        state.proxy = String::new();
        let revision = state.snapshot.revision;
        state.snapshot = MailSnapshot {
            revision,
            ..MailSnapshot::default()
        };
        let snapshot = state.changed();
        drop(state);
        service.publish(snapshot.clone(), None);
        let _ = service.send(Work::Wake);
        Ok(snapshot)
    })
    .await
    .map_err(|_| "无法断开邮箱。".to_string())?
}

/// 账号列表项：账号配置 + 当前是否为活动连接。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailAccountEntry {
    profile: MailAccountProfile,
    active: bool,
}

#[tauri::command]
pub fn list_mail_accounts() -> Result<Vec<MailAccountEntry>, String> {
    let service = service()?;
    let active_email = service.lock()?.snapshot.email.clone();
    Ok(load_accounts()
        .into_iter()
        .map(|profile| MailAccountEntry {
            active: active_email.as_deref() == Some(profile.email.as_str()),
            profile,
        })
        .collect())
}

#[tauri::command]
pub async fn switch_mail_account(email: String) -> Result<MailSnapshot, String> {
    let email = normalize_email(&email)?;
    let profile = find_account(&email).ok_or("未找到该邮箱账号，请重新添加。")?;
    let secret = read_secret(&email)?
        .ok_or("未找到该邮箱的授权码，请重新连接。")?;
    let service = service()?;
    let (generation, snapshot) = {
        let mut state = service.lock()?;
        state.invalidate();
        state.host = profile.host.clone();
        state.port = profile.port;
        state.proxy = profile.proxy.clone();
        state.snapshot.email = Some(profile.email.clone());
        state.snapshot.enabled = true;
        state.snapshot.syncing = true;
        state.snapshot.status = MailStatus::Connecting;
        state.snapshot.error = None;
        (state.generation, state.changed())
    };
    service.publish(snapshot, None);
    let (reply, receiver) = mpsc::channel();
    service
        .send(Work::Connect {
            generation,
            email: profile.email,
            secret,
            host: profile.host,
            port: profile.port,
            proxy: profile.proxy,
            reply,
        })
        .map_err(|_| "邮箱服务不可用。".to_string())?;
    await_reply(receiver).await
}

#[tauri::command]
pub async fn remove_mail_account(email: String) -> Result<MailSnapshot, String> {
    let email = normalize_email(&email)?;
    tauri::async_runtime::spawn_blocking(move || {
        let service = service()?;
        let is_active = service.lock()?.snapshot.email.as_deref() == Some(email.as_str());
        let _ = delete_secret(&email);
        let _ = remove_account(&email);
        if is_active {
            let mut state = service.lock()?;
            state.invalidate();
            state.host = String::new();
            state.port = DEFAULT_PORT;
            state.proxy = String::new();
            let revision = state.snapshot.revision;
            state.snapshot = MailSnapshot {
                revision,
                ..MailSnapshot::default()
            };
            let snapshot = state.changed();
            drop(state);
            service.publish(snapshot.clone(), None);
            let _ = service.send(Work::Wake);
            return Ok(snapshot);
        }
        Ok(service.lock()?.snapshot.clone())
    })
    .await
    .map_err(|_| "无法删除邮箱账号。".to_string())?
}

async fn await_reply<T: Send + 'static>(
    receiver: mpsc::Receiver<Result<T, String>>,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        receiver
            .recv()
            .unwrap_or_else(|_| Err("QQ 邮箱服务不可用。".into()))
    })
    .await
    .map_err(|_| "QQ 邮箱连接任务未完成。".to_string())?
}
