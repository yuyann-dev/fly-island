//! Taskbar flash source for QQ: detects HSHELL_FLASH without showing toasts.

use crate::win32::process;
use std::{cell::RefCell, path::PathBuf};
use windows::{
    core::{w, Error},
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DeregisterShellHookWindow, DestroyWindow,
            DispatchMessageW, GetForegroundWindow, PeekMessageW, RegisterClassW,
            RegisterShellHookWindow, RegisterWindowMessageW, TranslateMessage, UnregisterClassW,
            HSHELL_REDRAW, HSHELL_WINDOWACTIVATED, HSHELL_WINDOWCREATED, MSG, PM_REMOVE, WNDCLASSW,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
};

const HSHELL_FLASH: u32 = HSHELL_REDRAW | 0x8000;
const HSHELL_RUDEAPPACTIVATED: u32 = HSHELL_WINDOWACTIVATED | 0x8000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QQFlashSignal {
    Attention,
    Activated,
}

struct ListenerContext {
    message: u32,
    executable: String,
    callback: Box<dyn FnMut(QQFlashSignal)>,
}

thread_local! {
    static CONTEXT: RefCell<Option<ListenerContext>> = const { RefCell::new(None) };
}

fn normalized_path(path: &str) -> String {
    path.trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase()
}

unsafe fn matches_window(window: HWND, executable: &str) -> bool {
    if window.is_invalid() {
        return false;
    }
    let path = process::image_path(process::window_process_id(window));
    path.is_some_and(|p| normalized_path(&p) == executable)
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    kind: WPARAM,
    target: LPARAM,
) -> LRESULT {
    CONTEXT.with(|slot| {
        let Ok(mut context) = slot.try_borrow_mut() else {
            return;
        };
        let Some(context) = context.as_mut() else {
            return;
        };
        if message != context.message {
            return;
        }
        let signal = match kind.0 as u32 {
            HSHELL_FLASH => QQFlashSignal::Attention,
            HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED => QQFlashSignal::Activated,
            HSHELL_WINDOWCREATED => return,
            _ => return,
        };
        if matches_window(HWND(target.0 as *mut _), &context.executable) {
            if signal == QQFlashSignal::Attention
                && matches_window(GetForegroundWindow(), &context.executable)
            {
                return;
            }
            (context.callback)(signal);
        }
    });
    DefWindowProcW(window, message, kind, target)
}

pub struct QQTaskbarListener {
    window: HWND,
    instance: HINSTANCE,
}

impl QQTaskbarListener {
    pub fn new(
        executable: PathBuf,
        callback: impl FnMut(QQFlashSignal) + 'static,
    ) -> Result<Self, String> {
        if CONTEXT.with(|slot| slot.borrow().is_some()) {
            return Err("QQ 任务栏监听器已在此线程运行。".into());
        }
        let executable = executable
            .canonicalize()
            .map_err(|error| format!("无法确认 QQ 安装路径：{error}"))?;
        unsafe {
            let message = RegisterWindowMessageW(w!("SHELLHOOK"));
            if message == 0 {
                return Err(format!("无法注册 QQ 任务栏事件：{}", Error::from_win32()));
            }
            let instance = HINSTANCE(GetModuleHandleW(None).map_err(|error| error.to_string())?.0);
            let class = WNDCLASSW {
                hInstance: instance,
                lpszClassName: w!("FocuSDQQTaskbarSource"),
                lpfnWndProc: Some(window_proc),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err(Error::from_win32().to_string());
            }
            let window = match CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.lpszClassName,
                w!(""),
                WS_POPUP,
                0, 0, 0, 0,
                None, None, Some(instance), None,
            ) {
                Ok(window) => window,
                Err(error) => {
                    let _ = UnregisterClassW(class.lpszClassName, Some(instance));
                    return Err(format!("无法创建 QQ 任务栏接收器：{error}"));
                }
            };
            CONTEXT.with(|slot| {
                *slot.borrow_mut() = Some(ListenerContext {
                    message,
                    executable: normalized_path(&executable.to_string_lossy()),
                    callback: Box::new(callback),
                })
            });
            let listener = Self { window, instance };
            RegisterShellHookWindow(window)
                .ok()
                .map_err(|error| format!("无法监听 QQ 任务栏闪烁：{error}"))?;
            Ok(listener)
        }
    }

    pub fn poll(&self) {
        let mut message = MSG::default();
        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

impl Drop for QQTaskbarListener {
    fn drop(&mut self) {
        unsafe {
            let _ = DeregisterShellHookWindow(self.window);
            let _ = DestroyWindow(self.window);
            let _ = UnregisterClassW(w!("FocuSDQQTaskbarSource"), Some(self.instance));
        }
        CONTEXT.with(|slot| *slot.borrow_mut() = None);
    }
}
