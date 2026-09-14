//! End-to-end stock capture -> FB -> production engine/runtime -> observed Windows output.
//! The diagnostic hook only counts four codes and always calls the next hook.
#![cfg_attr(windows, allow(unsafe_code))]
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
}

#[cfg(windows)]
mod test {
    use io_core::{
        automation::{ActionCommand, ActionDefinition, AutomationProfile, PlatformCommands},
        depth::ComputerAction,
        exclusive_depth::DepthChoice,
    };
    use io_platform::{INJECTED_INPUT_TAG, service::KeyboardService};
    use std::{
        io::IsTerminal,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU32, Ordering},
        },
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*,
    };
    static DOWN: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
    static UP: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
    static OTHER: AtomicU32 = AtomicU32::new(0);
    unsafe extern "system" fn hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            // SAFETY: WH_KEYBOARD_LL supplies a live KBDLLHOOKSTRUCT for HC_ACTION.
            let event = unsafe { &*(lp as *const KBDLLHOOKSTRUCT) };
            if let Some(index) = [0x21, 0x22, 0xAF, 0xAE]
                .iter()
                .position(|&k| k == event.vkCode)
            {
                if event.dwExtraInfo == INJECTED_INPUT_TAG && event.flags & LLKHF_INJECTED != 0 {
                    let counts = if event.flags & LLKHF_UP != 0 {
                        &UP
                    } else {
                        &DOWN
                    };
                    counts[index].fetch_add(1, Ordering::Relaxed);
                } else {
                    OTHER.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        // Never consumes, rewrites or delays the user's input.
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wp, lp) }
    }
    fn counts() -> serde_json::Value {
        serde_json::json!({"order":["PageUp","PageDown","VolumeUp","VolumeDown"],
            "down":DOWN.iter().map(|c|c.load(Ordering::Relaxed)).collect::<Vec<_>>(),
            "up":UP.iter().map(|c|c.load(Ordering::Relaxed)).collect::<Vec<_>>(),
            "otherTargetEvents":OTHER.load(Ordering::Relaxed)})
    }
    fn clear() {
        for c in DOWN.iter().chain(UP.iter()).chain([&OTHER]) {
            c.store(0, Ordering::Relaxed);
        }
    }
    fn ready(message: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("{message}\nНажмите Enter, когда готовы.");
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line)? == 0 {
            return Err("Input closed".into());
        }
        Ok(())
    }
    fn profile() -> AutomationProfile {
        let commands = [
            (
                "pageUp",
                ActionCommand::Key {
                    key: 75,
                    modifiers: 0,
                },
            ),
            (
                "pageDown",
                ActionCommand::Key {
                    key: 78,
                    modifiers: 0,
                },
            ),
            (
                "volumeUp",
                ActionCommand::Media {
                    action: ComputerAction::VolumeUp,
                },
            ),
            (
                "volumeDown",
                ActionCommand::Media {
                    action: ComputerAction::VolumeDown,
                },
            ),
        ];
        AutomationProfile {
            actions: commands
                .into_iter()
                .map(|(id, command)| ActionDefinition {
                    id: id.into(),
                    name: id.into(),
                    command,
                    platform_commands: PlatformCommands::default(),
                })
                .collect(),
            gestures: vec![],
            depth_choices: [(105, "pageUp", "volumeUp"), (108, "pageDown", "volumeDown")]
                .into_iter()
                .map(|(slot, light, deep)| DepthChoice {
                    id: format!("depth-{slot}"),
                    slot,
                    light_um: 600,
                    deep_um: 3000,
                    release_um: 200,
                    light_action_id: light.into(),
                    deep_action_id: deep.into(),
                })
                .collect(),
        }
    }
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let directory =
            std::path::PathBuf::from("archive_data/configuration_recovery/depth-choice");
        let service = KeyboardService::new(directory);
        let before = service.connect()?;
        let profile = profile();
        let plan = service
            .prepare_automation(profile.clone(), before.revision.clone())?
            .ok_or("no plan")?;
        println!("{}", serde_json::to_string(&plan)?);
        if std::env::args().nth(1).as_deref() != Some("--run") {
            return Ok(());
        }
        if !std::io::stdin().is_terminal() {
            return Err("Interactive terminal required".into());
        }
        let finished = Arc::new(AtomicBool::new(false));
        let end = finished.clone();
        let heartbeat = service.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let observer = std::thread::spawn(move || {
            let handle = unsafe {
                SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(hook),
                    GetModuleHandleW(std::ptr::null()),
                    0,
                )
            };
            let _ = tx.send(!handle.is_null());
            if handle.is_null() {
                return;
            }
            while !end.load(Ordering::Relaxed) {
                let mut msg = unsafe { std::mem::zeroed() };
                while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0
                {
                    unsafe {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                heartbeat.monitor_frame();
                std::thread::sleep(Duration::from_millis(2));
            }
            unsafe {
                UnhookWindowsHookEx(handle);
            }
        });
        if !rx.recv()? {
            finished.store(true, Ordering::Relaxed);
            observer.join().ok();
            return Err("Observer unavailable".into());
        }
        let mut phases = Vec::new();
        let measurement = (|| -> Result<(), Box<dyn std::error::Error>> {
            ready(
                "Проверка ГОТОВОГО режима. Отпустите клавиши. После Enter приложение временно заберёт PgUp/PgDn и начнёт выбирать действия по глубине.",
            )?;
            service.configure_automation(profile, true, Some(plan.token))?;
            for (name, instruction) in [
                (
                    "light",
                    "15 секунд: несколько раз МЯГКО нажмите и отпустите PgUp/PgDn, не до упора. Ожидаются только Page Up / Page Down.",
                ),
                (
                    "deep",
                    "15 секунд: несколько раз нажмите PgUp/PgDn ДО УПОРА, подержите и полностью отпустите. Ожидается только громкость, один шаг за нажатие.",
                ),
            ] {
                ready(instruction)?;
                clear();
                let start = Instant::now();
                let mut peak = [0; 2];
                while start.elapsed() < Duration::from_secs(15) {
                    let frame = service.monitor_frame();
                    if let Some(error) = frame.rule_error {
                        return Err(error.into());
                    }
                    if !frame.rules_enabled {
                        return Err("Rules stopped".into());
                    }
                    for (i, slot) in [105, 108].iter().enumerate() {
                        if let Some(sample) = frame
                            .travel
                            .iter()
                            .find(|s| s.slot == *slot && s.age_ms < 600)
                        {
                            peak[i] = peak[i].max(sample.travel_um);
                        }
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                let result = serde_json::json!({"phase":name,"outputs":counts(),"peakUm":peak,"runtime":service.monitor_frame().rule_firings});
                println!("{result}");
                phases.push(result);
            }
            Ok(())
        })();
        // Always restore using the same production stop path, even when measurement fails.
        let restore = service.disconnect();
        finished.store(true, Ordering::Relaxed);
        observer.join().ok();
        restore?;
        let after = service.connect()?;
        service.disconnect()?;
        let restored = before.revision == after.revision;
        let result = serde_json::json!({"schemaVersion":1,"beforeRevision":before.revision,"afterRevision":after.revision,"restored":restored,"phases":phases,"measurementError":measurement.as_ref().err().map(ToString::to_string),"source":"production KeyboardService / GestureEngine / ActionRuntime; non-consuming Windows hook"});
        std::fs::write(
            "archive_data/session_snapshots/depth-choice-hardware.json",
            serde_json::to_vec_pretty(&result)?,
        )?;
        println!("{result}");
        if !restored {
            return Err("Snapshot restoration mismatch".into());
        }
        measurement
    }
}
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    test::run()
}
