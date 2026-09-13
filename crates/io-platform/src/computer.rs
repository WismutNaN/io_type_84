//! Узкий порт действий компьютера. Не принимает произвольные virtual-key/скрипты из IPC.
use io_core::{
    depth::ComputerAction,
    keyboard::{AppError, Result},
};

#[cfg(windows)]
#[allow(unsafe_code)]
pub fn execute(action: ComputerAction) -> Result<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let key = match action {
        ComputerAction::VolumeUp => VK_VOLUME_UP,
        ComputerAction::VolumeDown => VK_VOLUME_DOWN,
        ComputerAction::Mute => VK_VOLUME_MUTE,
        ComputerAction::PlayPause => VK_MEDIA_PLAY_PAUSE,
    };
    let make = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [make(0), make(KEYEVENTF_KEYUP)];
    // SAFETY: две полностью инициализированные INPUT живут до возврата SendInput;
    // длина/размер точные, указатель только читается Win32.
    let count = unsafe { SendInput(2, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    if count != 2 {
        // Возможная частичная вставка: пробуем отпустить ту же media-клавишу.
        let release = make(KEYEVENTF_KEYUP);
        unsafe {
            SendInput(1, &release, size_of::<INPUT>() as i32);
        }
        return Err(AppError::new(
            "computerActionFailed",
            "Windows не выполнила действие. Программные действия остановлены.",
        ));
    }
    Ok(())
}
#[cfg(not(windows))]
pub fn execute(_: ComputerAction) -> Result<()> {
    Err(AppError::new(
        "unsupportedPlatform",
        "Действия компьютера пока доступны только в Windows.",
    ))
}
