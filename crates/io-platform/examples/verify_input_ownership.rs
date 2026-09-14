//! Bounded PgUp/PgDn experiment. Only base bindings change; restore runs on errors.
//! Raw Input observes this keyboard, never suppresses or injects input.
#![cfg_attr(windows, allow(unsafe_code))]

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
}

#[cfg(windows)]
mod experiment {
    use io_platform::{
        changes::{self, PreparedChange},
        device::NativeDevice,
    };
    use serde::Serialize;
    use std::{
        collections::BTreeMap,
        io::IsTerminal,
        path::Path,
        sync::Mutex,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Input::*, WindowsAndMessaging::*},
    };

    #[derive(Default, Clone, Serialize)]
    struct Counts {
        down: u32,
        up: u32,
    }
    static COUNTS: Mutex<BTreeMap<u16, Counts>> = Mutex::new(BTreeMap::new());
    static ERRORS: Mutex<u32> = Mutex::new(0);
    static OBSERVED: Mutex<(u32, u32)> = Mutex::new((0, 0));
    static CONSUMER_HELD: Mutex<u16> = Mutex::new(0);
    static MODIFIER_PROBE: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    fn is_io(device: HANDLE) -> bool {
        let mut buffer = [0u16; 512];
        let mut length = buffer.len() as u32;
        // SAFETY: aligned writable buffer, length in UTF-16 characters.
        let n = unsafe {
            GetRawInputDeviceInfoW(
                device,
                RIDI_DEVICENAME,
                buffer.as_mut_ptr().cast(),
                &mut length,
            )
        };
        if n == u32::MAX || n as usize >= buffer.len() {
            return false;
        }
        let name = String::from_utf16_lossy(&buffer[..n as usize]).to_ascii_lowercase();
        // Raw Input can render VID/PID with '&' or '_' separators.
        name.contains("vid_0c45") && name.contains("pid_80d6")
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_INPUT {
            if let Ok(mut o) = OBSERVED.lock() {
                o.0 += 1;
            }
            let mut input: RAWINPUT = unsafe { std::mem::zeroed() };
            let mut size = size_of::<RAWINPUT>() as u32;
            let n = unsafe {
                GetRawInputData(
                    lp as HRAWINPUT,
                    RID_INPUT,
                    (&mut input as *mut RAWINPUT).cast(),
                    &mut size,
                    size_of::<RAWINPUTHEADER>() as u32,
                )
            };
            if n == u32::MAX {
                if let Ok(mut errors) = ERRORS.lock() {
                    *errors += 1;
                }
            } else if input.header.dwType == RIM_TYPEHID && is_io(input.header.hDevice) {
                if let Ok(mut o) = OBSERVED.lock() {
                    o.1 += 1;
                }
                let hid = unsafe { input.data.hid };
                let offset = size_of::<RAWINPUTHEADER>() + 8;
                if hid.dwSizeHid == 3 && hid.dwCount == 1 && n as usize >= offset + 3 {
                    // SAFETY: verified report length within fully initialized RAWINPUT buffer.
                    let bytes = unsafe {
                        std::slice::from_raw_parts(
                            (&input as *const RAWINPUT).cast::<u8>(),
                            n as usize,
                        )
                    };
                    let report = &bytes[offset..offset + 3];
                    if report[0] == 3 {
                        let usage = u16::from_le_bytes([report[1], report[2]]);
                        if let (Ok(mut held), Ok(mut counts)) =
                            (CONSUMER_HELD.lock(), COUNTS.lock())
                            && usage != *held
                        {
                            if [0xE9, 0xEA].contains(&*held) {
                                counts
                                    .entry(if *held == 0xE9 { 0xAF } else { 0xAE })
                                    .or_default()
                                    .up += 1;
                            }
                            if [0xE9, 0xEA].contains(&usage) {
                                counts
                                    .entry(if usage == 0xE9 { 0xAF } else { 0xAE })
                                    .or_default()
                                    .down += 1;
                            }
                            *held = usage;
                        }
                    }
                }
            } else if n >= (size_of::<RAWINPUTHEADER>() + size_of::<RAWKEYBOARD>()) as u32
                && input.header.dwType == RIM_TYPEKEYBOARD
                && is_io(input.header.hDevice)
            {
                if let Ok(mut o) = OBSERVED.lock() {
                    o.1 += 1;
                }
                let keyboard = unsafe { input.data.keyboard };
                let modifier = MODIFIER_PROBE.load(std::sync::atomic::Ordering::Relaxed)
                    && [
                        0x10, 0x11, 0x12, 0x20, 0x5B, 0x5C, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5,
                    ]
                    .contains(&keyboard.VKey);
                // Save counts only for two target keys and two possible markers.
                if (modifier || [0x21, 0x22, 0x7C, 0x7D, 0xAE, 0xAF].contains(&keyboard.VKey))
                    && let Ok(mut counts) = COUNTS.lock()
                {
                    let count = counts.entry(keyboard.VKey).or_default();
                    if keyboard.Flags & 1 == 0 {
                        count.down += 1;
                    } else {
                        count.up += 1;
                    }
                }
            }
        }
        // SAFETY: required default processing, including foreground WM_INPUT cleanup.
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }

    struct Observer {
        hwnd: HWND,
        class: Vec<u16>,
        instance: HINSTANCE,
    }
    impl Observer {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let class: Vec<_> = "IoStockInputProbe\0".encode_utf16().collect();
            let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                ..unsafe { std::mem::zeroed() }
            };
            if unsafe { RegisterClassW(&wc) } == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    class.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null(),
                )
            };
            let observer = Self {
                hwnd,
                class,
                instance,
            };
            if hwnd.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let rid = [
                RAWINPUTDEVICE {
                    usUsagePage: 1,
                    usUsage: 6,
                    dwFlags: RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                },
                RAWINPUTDEVICE {
                    usUsagePage: 0x0C,
                    usUsage: 1,
                    dwFlags: RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                },
            ];
            if unsafe {
                RegisterRawInputDevices(rid.as_ptr(), 2, size_of::<RAWINPUTDEVICE>() as u32)
            } == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(observer)
        }
        fn pump(&self) {
            let mut msg = unsafe { std::mem::zeroed() };
            while unsafe { PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE) } != 0 {
                unsafe {
                    DispatchMessageW(&msg);
                }
            }
        }
    }
    impl Drop for Observer {
        fn drop(&mut self) {
            let rid = [
                RAWINPUTDEVICE {
                    usUsagePage: 1,
                    usUsage: 6,
                    dwFlags: RIDEV_REMOVE,
                    hwndTarget: std::ptr::null_mut(),
                },
                RAWINPUTDEVICE {
                    usUsagePage: 0x0C,
                    usUsage: 1,
                    dwFlags: RIDEV_REMOVE,
                    hwndTarget: std::ptr::null_mut(),
                },
            ];
            unsafe {
                RegisterRawInputDevices(rid.as_ptr(), 2, size_of::<RAWINPUTDEVICE>() as u32);
                if !self.hwnd.is_null() {
                    DestroyWindow(self.hwnd);
                }
                UnregisterClassW(self.class.as_ptr(), self.instance);
            }
        }
    }

    #[derive(Default, Serialize)]
    struct Analog {
        packets: u32,
        peak_um: u32,
        minimum_um: u32,
        samples_after_peak: u32,
    }
    fn observe(
        observer: &Observer,
        device: &mut NativeDevice,
        name: &str,
        seconds: u64,
        analog: bool,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        observer.pump();
        COUNTS.lock().map_err(|_| "counter lock")?.clear();
        *OBSERVED.lock().map_err(|_| "observer lock")? = (0, 0);
        let mut travel = BTreeMap::<u8, Analog>::new();
        let start = Instant::now();
        eprintln!("PHASE {name}: {seconds}s — PgUp/PgDn, press/release repeatedly");
        while start.elapsed() < Duration::from_secs(seconds) {
            if analog {
                device.poll_notifications(5)?;
                while let Some(p) = device.notifications.pop_front() {
                    if p.len() >= 14 && p[0..2] == [0x55, 0xFB] && [105, 108].contains(&p[2]) {
                        let um = u16::from_le_bytes([p[10], p[11]]) as u32 * 10;
                        if um <= 10_000 {
                            let s = travel.entry(p[2]).or_insert_with(|| Analog {
                                minimum_um: um,
                                ..Analog::default()
                            });
                            s.packets += 1;
                            s.minimum_um = s.minimum_um.min(um);
                            if um > s.peak_um {
                                s.peak_um = um;
                                s.samples_after_peak = 0;
                            } else if um < 100 && s.peak_um > 1000 {
                                s.samples_after_peak += 1;
                            }
                        }
                    }
                }
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
            observer.pump();
            if name == "baseline"
                && COUNTS
                    .lock()
                    .map_err(|_| "counter lock")?
                    .values()
                    .filter(|c| c.down >= 2 && c.up >= 2)
                    .count()
                    >= 2
            {
                break;
            }
        }
        let result = serde_json::json!({"phase":name,"seconds":seconds,"keys":COUNTS.lock().map_err(|_| "counter lock")?.clone(),"analog":travel,"observerMessagesAndTarget":*OBSERVED.lock().map_err(|_| "observer lock")?});
        println!("{result}");
        Ok(result)
    }

    fn ready(instruction: &str) -> Result<(), Box<dyn std::error::Error>> {
        eprintln!(
            "{instruction}\nНажмите Enter, когда готовы. После записи не закрывайте процесс до восстановления."
        );
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line)? == 0 {
            return Err("Interactive input closed".into());
        }
        std::thread::sleep(Duration::from_millis(500));
        Ok(())
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let run = std::env::args().nth(1).as_deref() == Some("--run");
        let mut device = NativeDevice::open()?;
        if std::env::args().nth(1).as_deref() == Some("--modifiers") {
            MODIFIER_PROBE.store(true, std::sync::atomic::Ordering::Relaxed);
            let observer = Observer::new()?;
            observe(&observer, &mut device, "read-only-modifiers", 120, false)?;
            return Ok(());
        }
        if std::env::args().nth(1).as_deref() == Some("--observe") {
            let observer = Observer::new()?;
            observe(&observer, &mut device, "read-only-control", 20, false)?;
            return Ok(());
        }
        let before = device.snapshot()?;
        let mut after = before.clone();
        for slot in [105usize, 108] {
            after.blocks.get_mut("base").ok_or("missing base")?[slot * 4..slot * 4 + 4]
                .copy_from_slice(&[5, 0, 0, 0]);
        }
        let plan = PreparedChange {
            token: after.revision(),
            before: before.clone(),
            after,
            blocks: vec!["base".into()],
        };
        println!("{}", serde_json::to_string(&plan.preview())?);
        if !run {
            eprintln!(
                "Dry preview. --run performs bounded test and restores base block. No firmware operations."
            );
            return Ok(());
        }
        if !std::io::stdin().is_terminal() {
            return Err(
                "Hardware run requires an interactive terminal with manual phase boundaries".into(),
            );
        }
        let observer = Observer::new()?;
        let directory = Path::new("archive_data/configuration_recovery/input-ownership");
        ready("Контроль: после Enter поочерёдно нажмите/отпустите PgUp и PgDn минимум дважды.")?;
        let mut phases = vec![observe(&observer, &mut device, "baseline", 60, false)?];
        if COUNTS
            .lock()
            .map_err(|_| "counter lock")?
            .values()
            .filter(|c| c.down >= 2 && c.up >= 2)
            .count()
            < 2
        {
            return Err("No positive digital control: stopped before writing".into());
        }
        // Keep restoration outside the fallible measurement closure, including SET failures.
        let measurement = (|| -> Result<(), Box<dyn std::error::Error>> {
            ready(
                "Отпустите все клавиши. Следующий шаг временно отключает вывод только PgUp/PgDn.",
            )?;
            changes::apply(&mut device, &plan, directory)?;
            ready("После Enter нажимайте PgUp/PgDn 8 секунд: проверка без обычного вывода.")?;
            phases.push(observe(
                &observer,
                &mut device,
                "stock-no-output",
                8,
                false,
            )?);
            ready(
                "После Enter нажимайте мягко/до упора и отпускайте обе клавиши 20 секунд: измерение хода.",
            )?;
            device.start_monitor()?;
            phases.push(observe(
                &observer,
                &mut device,
                "stock-no-output-plus-analog",
                20,
                true,
            )?);
            device.stop_monitor()?;
            ready("Отпустите все клавиши. Следующий шаг восстанавливает исходные назначения.")?;
            Ok(())
        })();
        // Drop stops simulation even when measurement failed; reopen for GET/recovery.
        drop(device);
        let mut device = NativeDevice::open()?;
        let current = device.snapshot()?;
        let mut restored = current.clone();
        restored
            .blocks
            .insert("base".into(), before.blocks["base"].clone());
        let restore = PreparedChange {
            token: restored.revision(),
            before: current,
            after: restored,
            blocks: vec!["base".into()],
        };
        changes::apply(&mut device, &restore, directory)?;
        let restored_revision = device.snapshot()?.revision();
        if restored_revision != before.revision() {
            return Err("Restore differs from initial snapshot; retain recovery files".into());
        }
        phases.push(observe(&observer, &mut device, "restored", 10, false)?);
        let result = serde_json::json!({"schemaVersion":1,"hardwareAccess":true,"firmware":"White 1.17",
            "beforeRevision":before.revision(),"afterRevision":restored_revision,"restored":true,
            "measurementError":measurement.as_ref().err().map(ToString::to_string),"rawInputErrors":*ERRORS.lock().map_err(|_| "counter lock")?,"phases":phases,
            "limits":["No power-cycle persistence test","Only PgUp/PgDn and selected input routes"]});
        std::fs::write(
            "archive_data/session_snapshots/stock-input-hardware.json",
            serde_json::to_vec_pretty(&result)?,
        )?;
        println!("{result}");
        measurement
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    experiment::run()
}
