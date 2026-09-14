//! Очередь действий компьютера отдельно от HID; отмена освобождает синтетические клавиши.
use crate::monitor::MonitorState;
use io_core::{
    automation::{ActionCommand, ActionStep, ApplicationId},
    keyboard::{AppError, Result},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, SyncSender},
};
use std::time::{Duration, Instant};

pub struct ActionRuntime {
    sender: SyncSender<(u64, Instant, ActionCommand)>,
    generation: Arc<AtomicU64>,
}
impl ActionRuntime {
    pub fn new(monitor: Arc<Mutex<MonitorState>>) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<(u64, Instant, ActionCommand)>(16);
        let generation = Arc::new(AtomicU64::new(0));
        let version = Arc::clone(&generation);
        std::thread::spawn(move || {
            while let Ok((epoch, queued_at, command)) = receiver.recv() {
                let cancelled = || version.load(Ordering::SeqCst) != epoch;
                if cancelled() {
                    continue;
                }
                if queued_at.elapsed() > Duration::from_millis(500) {
                    let mut state = monitor.lock().expect("monitor");
                    state.rule_error =
                        Some("Действие просрочено в очереди. Жесты остановлены.".into());
                    state.rules_enabled = false;
                    version.fetch_add(1, Ordering::SeqCst);
                    continue;
                }
                let result = execute(&command, &cancelled);
                if cancelled() {
                    continue;
                }
                let mut state = monitor.lock().expect("monitor");
                match result {
                    Ok(()) => state.rule_firings = state.rule_firings.saturating_add(1),
                    Err(e) => {
                        state.rule_error = Some(e.message);
                        state.rules_enabled = false;
                        version.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        });
        Self { sender, generation }
    }
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
    pub fn submit(&self, command: ActionCommand) -> Result<()> {
        self.sender
            .try_send((
                self.generation.load(Ordering::SeqCst),
                Instant::now(),
                command,
            ))
            .map_err(|_| {
                AppError::new(
                    "actionQueueFull",
                    "Очередь действий заполнена. Действия остановлены.",
                )
            })
    }
}
impl Drop for ActionRuntime {
    fn drop(&mut self) {
        self.cancel();
    }
}
fn execute(command: &ActionCommand, cancelled: &dyn Fn() -> bool) -> Result<()> {
    match command {
        ActionCommand::Media { action } => crate::computer::execute(*action),
        ActionCommand::Key { key, modifiers } => key_press(*key, *modifiers),
        ActionCommand::Text { text } => type_text(text, cancelled),
        ActionCommand::Application { application } => launch(*application),
        ActionCommand::Macro { steps } => {
            for step in steps {
                if cancelled() {
                    break;
                }
                match step {
                    ActionStep::Key { key, modifiers } => key_press(*key, *modifiers)?,
                    ActionStep::Text { text } => type_text(text, cancelled)?,
                    ActionStep::Media { action } => crate::computer::execute(*action)?,
                    ActionStep::Delay { ms } => {
                        let deadline = Instant::now() + Duration::from_millis(u64::from(*ms));
                        while Instant::now() < deadline && !cancelled() {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                    }
                }
            }
            Ok(())
        }
    }
}
#[cfg(windows)]
fn virtual_key(key: u8) -> u16 {
    match key {
        4..=29 => u16::from(key - 4) + 0x41,
        30..=38 => u16::from(key - 30) + 0x31,
        39 => 0x30,
        40 => 0x0d,
        41 => 0x1b,
        42 => 0x08,
        43 => 0x09,
        44 => 0x20,
        45 => 0xbd,
        46 => 0xbb,
        47 => 0xdb,
        48 => 0xdd,
        49 | 50 => 0xdc,
        51 => 0xba,
        52 => 0xde,
        53 => 0xc0,
        54 => 0xbc,
        55 => 0xbe,
        56 => 0xbf,
        57 => 0x14,
        58..=69 => u16::from(key - 58) + 0x70,
        73 => 0x2d,
        74 => 0x24,
        75 => 0x21,
        76 => 0x2e,
        77 => 0x23,
        78 => 0x22,
        79 => 0x27,
        80 => 0x25,
        81 => 0x28,
        82 => 0x26,
        224 => 0xa2,
        225 => 0xa0,
        226 => 0xa4,
        227 => 0x5b,
        228 => 0xa3,
        229 => 0xa1,
        230 => 0xa5,
        231 => 0x5c,
        _ => 0,
    }
}
#[cfg(windows)]
#[allow(unsafe_code)]
fn send_key(key: u16, scan: u16, flags: u32) -> Result<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: crate::INJECTED_INPUT_TAG,
            },
        },
    };
    // SAFETY: INPUT и union полностью инициализированы и живут до возврата Win32.
    if unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) } != 1 {
        return Err(AppError::new(
            "computerActionFailed",
            "Windows не выполнила действие. Программные действия остановлены.",
        ));
    }
    Ok(())
}
#[cfg(windows)]
#[allow(unsafe_code)]
fn key_press(key: u8, modifiers: u8) -> Result<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let vk = virtual_key(key);
    if vk == 0 {
        return Err(AppError::new(
            "unsupportedKey",
            "Клавиша не поддерживается исполнителем.",
        ));
    }
    let mut keys = (0..8)
        .filter(|bit| modifiers & (1 << bit) != 0)
        .map(|bit| virtual_key(224 + bit))
        .collect::<Vec<_>>();
    if !keys.contains(&vk) {
        keys.push(vk);
    }
    // Не отпускать физически удерживаемый модификатор пользователя и не смешивать его с макросом.
    let conflicting = keys
        .iter()
        .copied()
        .chain([VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN])
        .any(|k| {
            // SAFETY: GetAsyncKeyState принимает обычный числовой VK, указателей нет.
            unsafe { GetAsyncKeyState(i32::from(k)) as u16 & 0x8000 != 0 }
        });
    if conflicting {
        return Err(AppError::new(
            "inputConflict",
            "Отпустите выходные клавиши и модификаторы перед выполнением действия.",
        ));
    }
    let mut pressed = Vec::new();
    let mut result = Ok(());
    for k in keys {
        result = send_key(k, 0, 0);
        if result.is_err() {
            break;
        }
        pressed.push(k);
    }
    for k in pressed.into_iter().rev() {
        let release = send_key(k, 0, KEYEVENTF_KEYUP);
        if result.is_ok() {
            result = release;
        }
    }
    result
}
#[cfg(windows)]
fn type_text(text: &str, cancelled: &dyn Fn() -> bool) -> Result<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    for character in text.chars() {
        if cancelled() {
            break;
        }
        for unit in character.encode_utf16(&mut [0; 2]).iter() {
            send_key(0, *unit, KEYEVENTF_UNICODE)?;
            send_key(0, *unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)?;
        }
    }
    Ok(())
}
#[cfg(windows)]
#[allow(unsafe_code)]
fn launch(application: ApplicationId) -> Result<()> {
    use windows_sys::Win32::{
        System::Com::*,
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    let file = match application {
        ApplicationId::Word => "winword.exe",
        ApplicationId::Notepad => "notepad.exe",
        ApplicationId::Calculator => "calc.exe",
    };
    let file = file.encode_utf16().chain([0]).collect::<Vec<_>>();
    let verb = "open\0".encode_utf16().collect::<Vec<_>>();
    // SAFETY: null parent/parameters, две живые NUL-terminated строки из фиксированного каталога.
    let code = unsafe {
        let initialized = CoInitializeEx(
            std::ptr::null(),
            COINIT_APARTMENTTHREADED as u32 | COINIT_DISABLE_OLE1DDE as u32,
        ) >= 0;
        let result = ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        ) as isize;
        if initialized {
            CoUninitialize();
        }
        result
    };
    if code <= 32 {
        return Err(AppError::new(
            "applicationUnavailable",
            "Приложение не найдено или Windows отказала в запуске.",
        ));
    }
    Ok(())
}
#[cfg(not(windows))]
fn key_press(_: u8, _: u8) -> Result<()> {
    unsupported()
}
#[cfg(not(windows))]
fn type_text(_: &str, _: &dyn Fn() -> bool) -> Result<()> {
    unsupported()
}
#[cfg(not(windows))]
fn launch(_: ApplicationId) -> Result<()> {
    unsupported()
}
#[cfg(not(windows))]
fn unsupported() -> Result<()> {
    Err(AppError::new(
        "unsupportedPlatform",
        "Действия компьютера пока доступны только в Windows.",
    ))
}
