//! Один worker владеет HID. Очередь ограничена; WebView никогда не читает USB.
use crate::{
    changes::{self, PreparedChange, RecoveryRecord},
    device::{NativeDevice, RawSnapshot},
    monitor::MonitorState,
};
use io_core::keyboard::*;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant},
};

use crate::action_runtime::ActionRuntime;
use crate::input_ownership::InputOwnership;
use io_core::automation::{AutomationProfile, GestureEngine};

type Job = Box<dyn FnOnce(&mut Worker) + Send>;
struct Worker {
    device: Option<NativeDevice>,
    snapshot: Option<RawSnapshot>,
    prepared: Option<PreparedChange>,
    prepared_is_ownership_restore: bool,
    monitor: Arc<Mutex<MonitorState>>,
    recovery_dir: PathBuf,
    rules: GestureEngine,
    runtime: ActionRuntime,
    epoch: Instant,
    ownership: Option<InputOwnership>,
    prepared_ownership: Option<(AutomationProfile, InputOwnership)>,
}

#[derive(Clone)]
pub struct KeyboardService {
    sender: SyncSender<Job>,
    monitor: Arc<Mutex<MonitorState>>,
    heartbeat: Arc<Mutex<Instant>>,
}

impl KeyboardService {
    pub fn new(recovery_dir: PathBuf) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Job>(16);
        let monitor = Arc::new(Mutex::new(MonitorState::default()));
        let heartbeat = Arc::new(Mutex::new(Instant::now()));
        let m = Arc::clone(&monitor);
        let monitor_for_actions = Arc::clone(&monitor);
        let beat = Arc::clone(&heartbeat);
        std::thread::spawn(move || {
            let mut worker = Worker {
                device: None,
                snapshot: None,
                prepared: None,
                prepared_is_ownership_restore: false,
                monitor: m,
                recovery_dir,
                rules: GestureEngine::new(AutomationProfile::default()),
                runtime: ActionRuntime::new(Arc::clone(&monitor_for_actions)),
                epoch: Instant::now(),
                ownership: None,
                prepared_ownership: None,
            };
            loop {
                match receiver.recv_timeout(Duration::from_millis(2)) {
                    Ok(job) => job(&mut worker),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                }
                {
                    let mut state = worker.monitor.lock().expect("monitor");
                    if state.rules_enabled {
                        dispatch_actions(
                            &mut worker.rules,
                            &worker.runtime,
                            &mut state,
                            worker.epoch.elapsed().as_millis() as u64,
                        );
                    }
                }

                let failed = {
                    let state = worker.monitor.lock().expect("monitor");
                    worker.ownership.is_some() && !state.rules_enabled && state.rule_error.is_some()
                };
                if failed {
                    worker.stop_and_report();
                }
                let active = worker.monitor.lock().expect("monitor").active;
                if active {
                    if beat.lock().expect("heartbeat").elapsed() > Duration::from_secs(5) {
                        worker.stop_and_report();
                        worker.monitor.lock().expect("monitor").message =
                            Some("Наблюдение остановлено: интерфейс не отвечает.".into());
                        continue;
                    }
                    if let Some(device) = worker.device.as_mut() {
                        let result = (|| {
                            for _ in 0..64 {
                                if !device.poll_notifications(0)? {
                                    break;
                                }
                            }
                            Ok::<(), AppError>(())
                        })();
                        match result {
                            Ok(()) => {
                                let mut state = worker.monitor.lock().expect("monitor");
                                while let Some(p) = device.notifications.pop_front() {
                                    state.ingest(&p);
                                    if state.rules_enabled
                                        && p.len() >= 14
                                        && p[..2] == [0x55, 0xfb]
                                    {
                                        let depth = u16::from_le_bytes([p[10], p[11]]);
                                        let max = u16::from_le_bytes([p[12], p[13]]);
                                        if depth > 1000 || max > 1000 {
                                            continue;
                                        }
                                        let now = worker.epoch.elapsed().as_millis() as u64;
                                        worker.rules.sample(p[2], depth * 10, now);
                                        dispatch_actions(
                                            &mut worker.rules,
                                            &worker.runtime,
                                            &mut state,
                                            now,
                                        );
                                    }
                                }
                            }
                            Err(error) => {
                                worker.stop_and_report();
                                worker.device = None;
                                worker.monitor.lock().expect("monitor").message =
                                    Some(error.message);
                            }
                        }
                    }
                }
            }
            worker.stop_and_report();
        });
        Self {
            sender,
            monitor,
            heartbeat,
        }
    }

    fn request<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Worker) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(Box::new(move |w| {
                let _ = sender.send(job(w));
            }))
            .map_err(|_| {
                AppError::new(
                    "busy",
                    "Устройство занято. Дождитесь завершения текущей операции.",
                )
            })?;
        // Операция имеет аппаратные тайм-ауты. Не возвращаем ранний успех/ошибку,
        // пока очередь всё ещё может исполнять принятую запись.
        receiver
            .recv()
            .map_err(|_| AppError::new("workerStopped", "USB worker остановлен."))?
    }

    pub fn connect(&self) -> Result<KeyboardSnapshot> {
        self.request(|w| {
            w.stop()?;
            w.device = None;
            w.prepared = None;
            let mut device = NativeDevice::open()?;
            let snapshot = device.snapshot()?;
            let dto = snapshot.decode()?;
            w.snapshot = Some(snapshot);
            w.device = Some(device);
            *w.monitor.lock().expect("monitor") = MonitorState::default();
            if InputOwnership::load(&w.recovery_dir)?.is_some() {
                w.monitor.lock().expect("monitor").message = Some("Предыдущий сеанс не завершён. Нажмите «Восстановить», чтобы вернуть назначения PgUp/PgDn.".into());
            }
            Ok(dto)
        })
    }
    pub fn disconnect(&self) -> Result<()> {
        self.request(|w| {
            w.stop()?;
            w.device = None;
            w.snapshot = None;
            w.prepared = None;
            Ok(())
        })
    }
    pub fn refresh(&self) -> Result<KeyboardSnapshot> {
        self.request(|w| {
            w.stop()?;
            w.device = None;
            w.device = Some(NativeDevice::open()?);
            let snapshot = w.device()?.snapshot()?;
            let dto = snapshot.decode()?;
            w.snapshot = Some(snapshot);
            w.prepared = None;
            Ok(dto)
        })
    }
    pub fn monitor(&self, enabled: bool) -> Result<MonitorFrame> {
        *self.heartbeat.lock().expect("heartbeat") = Instant::now();
        self.request(move|w|{
            w.stop()?;
            if enabled {
                w.device=None;
                w.device=Some(NativeDevice::open()?);
                let result=w.device()?.current_colors();
                match result {Ok(colors)=>w.monitor.lock().expect("monitor").set_colors(colors),Err(e) if e.code=="ambiguousColors"=>w.monitor.lock().expect("monitor").message=Some("Текущий RGB не подтверждён: устройство вернуло эхо. Показаны заданные цвета.".into()),Err(e)=>return Err(e)}
                w.device()?.start_monitor()?;
                w.monitor.lock().expect("monitor").active=true;
            }
            Ok(w.monitor.lock().expect("monitor").frame())
        })
    }
    pub fn monitor_frame(&self) -> MonitorFrame {
        *self.heartbeat.lock().expect("heartbeat") = Instant::now();
        self.monitor.lock().expect("monitor").frame()
    }
    pub fn prepare_automation(
        &self,
        profile: AutomationProfile,
        base_revision: String,
    ) -> Result<Option<ChangePreview>> {
        profile.validate()?;
        self.request(move |w| {
            let before = w
                .prepared
                .as_ref()
                .map(|p| &p.after)
                .filter(|s| s.revision() == base_revision)
                .or_else(|| {
                    w.snapshot
                        .as_ref()
                        .filter(|s| s.revision() == base_revision)
                })
                .ok_or_else(|| {
                    AppError::new(
                        "stalePlan",
                        "Перечитайте клавиатуру и проверьте изменения заново.",
                    )
                })?;
            let ownership = InputOwnership::prepare(before, &profile)?;
            let preview = ownership.as_ref().map(|o| o.plan().preview());
            w.prepared_ownership = ownership.map(|o| (profile, o));
            Ok(preview)
        })
    }
    pub fn configure_automation(
        &self,
        profile: AutomationProfile,
        enabled: bool,
        ownership_token: Option<String>,
    ) -> Result<()> {
        profile.validate()?;
        for rule in &profile.gestures {
            if rule
                .slots
                .iter()
                .any(|slot| !crate::layout::PHYSICAL_KEYS.iter().any(|(s, _)| s == slot))
            {
                return Err(AppError::new("invalidRule", "Выберите физические клавиши."));
            }
        }
        self.request(move |w| {
            if !enabled {
                w.stop()?;
                return Ok(());
            }
            let ownership = if profile.depth_choices.is_empty() {
                None
            } else {
                let (expected, owner) = w.prepared_ownership.take().ok_or_else(|| {
                    AppError::new(
                        "missingPlan",
                        "Сначала проверьте передачу клавиш помощнику.",
                    )
                })?;
                if expected != profile
                    || ownership_token.as_deref() != Some(owner.plan().token.as_str())
                {
                    return Err(AppError::new(
                        "stalePlan",
                        "Правила изменились. Проверьте передачу клавиш заново.",
                    ));
                }
                Some(owner)
            };
            w.stop()?;
            if InputOwnership::load(&w.recovery_dir)?.is_some() {
                return Err(AppError::new(
                    "captureRecoveryRequired",
                    "Сначала восстановите назначения предыдущего сеанса.",
                ));
            }
            w.device = None;
            w.device = Some(NativeDevice::open()?);
            let setup = (|| -> Result<()> {
                if let Some(owner) = ownership {
                    let plan = owner.plan();
                    if w.device()?.snapshot()?.revision() != plan.before.revision() {
                        return Err(AppError::new(
                            "externalChange",
                            "Клавиатура изменилась. Перечитайте её перед передачей клавиш.",
                        ));
                    }
                    // Durable journal is installed before the first SET, including uncertain failures.
                    owner.persist(&w.recovery_dir)?;
                    w.ownership = Some(owner);
                    let directory = w.recovery_dir.join("input-sessions");
                    if !plan.blocks.is_empty()
                        && let Err(error) = changes::apply(w.device()?, &plan, &directory)
                    {
                        if ["externalChange", "backupFailed"].contains(&error.code.as_str()) {
                            // These errors are raised before any SET. Do not overwrite a competing edit.
                            w.ownership = None;
                            InputOwnership::clear(&w.recovery_dir)?;
                        }
                        return Err(error);
                    }
                }
                w.device()?.start_monitor()?;
                Ok(())
            })();
            if let Err(error) = setup {
                w.stop()?;
                return Err(error);
            }
            w.rules = GestureEngine::new(profile);
            let mut state = w.monitor.lock().expect("monitor");
            state.active = true;
            state.rules_enabled = w.rules.profile.has_rules();
            state.rule_firings = 0;
            state.rule_error = None;
            Ok(())
        })
    }
    pub fn clear_history(&self) {
        self.monitor.lock().expect("monitor").clear_history();
    }
    pub fn set_history_capacity(&self, count: usize) {
        self.monitor
            .lock()
            .expect("monitor")
            .set_history_capacity(count);
    }
    pub fn prepare(&self, request: ChangeRequest) -> Result<ChangePreview> {
        self.request(move |w| {
            if w.ownership.is_none() && InputOwnership::load(&w.recovery_dir)?.is_some() {
                return Err(AppError::new(
                    "captureRecoveryRequired",
                    "Сначала восстановите назначения предыдущего сеанса.",
                ));
            }
            w.prepared_is_ownership_restore = false;
            let before = w
                .snapshot
                .as_ref()
                .ok_or_else(|| AppError::new("notConnected", "Сначала прочитайте клавиатуру."))?;
            let plan = changes::prepare(before, request)?;
            let preview = plan.preview();
            w.prepared = Some(plan);
            Ok(preview)
        })
    }
    pub fn apply(&self, token: String) -> Result<ApplyResult> {
        self.request(move |w| {
            w.stop()?;
            w.device = None;
            w.device = Some(NativeDevice::open()?);
            let plan = w
                .prepared
                .take()
                .ok_or_else(|| AppError::new("missingPlan", "Сначала проверьте изменения."))?;
            if plan.token != token {
                return Err(AppError::new(
                    "stalePlan",
                    "План уже изменился. Проверьте изменения заново.",
                ));
            }
            let directory = if w.prepared_is_ownership_restore {
                w.recovery_dir.join("input-sessions")
            } else {
                w.recovery_dir.clone()
            };
            let result = changes::apply(w.device()?, &plan, &directory);
            match result {
                Ok(dto) => {
                    let current = w.device()?.snapshot()?;
                    if let Some(owner) = InputOwnership::load(&w.recovery_dir)?
                        && owner.restore_plan(&current)?.blocks.is_empty()
                    {
                        InputOwnership::clear(&w.recovery_dir)?;
                    }
                    w.snapshot = Some(current);
                    Ok(dto)
                }
                Err(error) => {
                    w.device = None;
                    Err(error)
                }
            }
        })
    }
    pub fn prepare_recovery(&self) -> Result<ChangePreview> {
        self.request(|w| {
            w.stop()?;
            w.device = None;
            w.device = Some(NativeDevice::open()?);
            if let Some(owner) = InputOwnership::load(&w.recovery_dir)? {
                let current = w.device()?.snapshot()?;
                let plan = owner.restore_plan(&current)?;
                w.prepared_is_ownership_restore = true;
                let preview = plan.preview();
                w.snapshot = Some(current);
                w.prepared = Some(plan);
                return Ok(preview);
            }
            w.prepared_is_ownership_restore = false;
            let mut paths = std::fs::read_dir(&w.recovery_dir)
                .map_err(|_| AppError::new("noRecovery", "Резервные снимки ещё не созданы."))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("recovery-"))
                        && p.extension().is_some_and(|x| x == "json")
                })
                .collect::<Vec<_>>();
            paths.sort();
            let path = paths
                .last()
                .ok_or_else(|| AppError::new("noRecovery", "Резервные снимки ещё не созданы."))?;
            if std::fs::metadata(path)
                .map_err(|_| AppError::new("recoveryRead", "Не удалось открыть снимок."))?
                .len()
                > 2_000_000
            {
                return Err(AppError::new("recoveryRead", "Снимок слишком большой."));
            }
            let record: RecoveryRecord = serde_json::from_slice(
                &std::fs::read(path)
                    .map_err(|_| AppError::new("recoveryRead", "Не удалось прочитать снимок."))?,
            )
            .map_err(|_| AppError::new("recoveryRead", "Файл восстановления повреждён."))?;
            if record.schema_version != 1 {
                return Err(AppError::new("recoveryRead", "Неизвестная версия снимка."));
            }
            record.before.decode()?;
            record.target.decode()?;
            let current = w.device()?.snapshot()?;
            if current.identity != record.before.identity {
                return Err(AppError::new(
                    "recoveryIdentity",
                    "Снимок относится к другой модели или версии.",
                ));
            }
            let mut target = current.clone();
            for name in &record.blocks {
                if ![
                    "game",
                    "base",
                    "function",
                    "lighting",
                    "colors",
                    "actuation",
                    "dks",
                    "macros",
                ]
                .contains(&name.as_str())
                {
                    return Err(AppError::new(
                        "recoveryRead",
                        "Неизвестный блок восстановления.",
                    ));
                }
                target
                    .blocks
                    .insert(name.clone(), record.before.blocks[name].clone());
            }
            let blocks = record
                .blocks
                .into_iter()
                .filter(|n| current.blocks[n] != target.blocks[n])
                .collect();
            let plan = PreparedChange {
                token: target.revision(),
                before: current.clone(),
                after: target,
                blocks,
            };
            let preview = plan.preview();
            w.snapshot = Some(current);
            w.prepared = Some(plan);
            Ok(preview)
        })
    }
}

impl Worker {
    fn device(&mut self) -> Result<&mut NativeDevice> {
        self.device
            .as_mut()
            .ok_or_else(|| AppError::new("notConnected", "Клавиатура не подключена к приложению."))
    }
    fn stop(&mut self) -> Result<()> {
        self.runtime.cancel();
        self.rules = GestureEngine::new(AutomationProfile::default());
        self.monitor.lock().expect("monitor").pause();
        let stopped = self.device.as_mut().map(|d| d.stop_monitor()).transpose();
        if let Some(owner) = self.ownership.clone() {
            // A timed-out handle cannot be used to decide whether a SET succeeded.
            self.device = None;
            self.device = Some(NativeDevice::open()?);
            let current = self.device()?.snapshot()?;
            let plan = owner.restore_plan(&current)?;
            if !plan.blocks.is_empty() {
                let directory = self.recovery_dir.join("input-sessions");
                changes::apply(self.device()?, &plan, &directory)?;
            }
            self.snapshot = Some(self.device()?.snapshot()?);
            InputOwnership::clear(&self.recovery_dir)?;
            self.ownership = None;
            return Ok(());
        }
        if let Err(error) = stopped {
            self.monitor.lock().expect("monitor").message = Some(error.message);
        }
        Ok(())
    }
    fn stop_and_report(&mut self) {
        if let Err(e) = self.stop() {
            // Retain the durable journal; do not retry uncertain SET in the worker loop.
            self.ownership = None;
            self.device = None;
            self.monitor.lock().expect("monitor").rule_error =
                Some(format!("Восстановление не завершено: {}", e.message));
        }
    }
}

fn dispatch_actions(
    rules: &mut GestureEngine,
    runtime: &ActionRuntime,
    state: &mut MonitorState,
    now: u64,
) {
    let fired = rules.tick(now);
    runtime.refresh_repeats(rules.repeat_windows(now));
    for event in fired {
        if let Some(action) = rules
            .profile
            .actions
            .iter()
            .find(|a| a.id == event.action_id)
        {
            let variant = match std::env::consts::OS {
                "linux" => &action.platform_commands.linux,
                "macos" => &action.platform_commands.macos,
                _ => &action.platform_commands.windows,
            };
            if let Err(e) = runtime.submit(
                variant.as_ref().unwrap_or(&action.command).clone(),
                event.repeat,
            ) {
                state.rule_error = Some(e.message);
                state.rules_enabled = false;
                runtime.cancel();
                break;
            }
        }
    }
}
