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

type Job = Box<dyn FnOnce(&mut Worker) + Send>;
struct Worker {
    device: Option<NativeDevice>,
    snapshot: Option<RawSnapshot>,
    prepared: Option<PreparedChange>,
    monitor: Arc<Mutex<MonitorState>>,
    recovery_dir: PathBuf,
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
        let beat = Arc::clone(&heartbeat);
        std::thread::spawn(move || {
            let mut worker = Worker {
                device: None,
                snapshot: None,
                prepared: None,
                monitor: m,
                recovery_dir,
            };
            loop {
                match receiver.recv_timeout(Duration::from_millis(2)) {
                    Ok(job) => job(&mut worker),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                }
                let active = worker.monitor.lock().expect("monitor").active;
                if active {
                    if beat.lock().expect("heartbeat").elapsed() > Duration::from_secs(5) {
                        worker.stop();
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
                                }
                            }
                            Err(error) => {
                                worker.stop();
                                worker.device = None;
                                worker.monitor.lock().expect("monitor").message =
                                    Some(error.message);
                            }
                        }
                    }
                }
            }
            worker.stop();
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
            w.stop();
            w.device = None;
            w.prepared = None;
            let mut device = NativeDevice::open()?;
            let snapshot = device.snapshot()?;
            let dto = snapshot.decode()?;
            w.snapshot = Some(snapshot);
            w.device = Some(device);
            *w.monitor.lock().expect("monitor") = MonitorState::default();
            Ok(dto)
        })
    }
    pub fn disconnect(&self) -> Result<()> {
        self.request(|w| {
            w.stop();
            w.device = None;
            w.snapshot = None;
            w.prepared = None;
            Ok(())
        })
    }
    pub fn refresh(&self) -> Result<KeyboardSnapshot> {
        self.request(|w| {
            w.stop();
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
            w.stop();
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
    pub fn clear_history(&self) {
        self.monitor.lock().expect("monitor").clear_history();
    }
    pub fn prepare(&self, request: ChangeRequest) -> Result<ChangePreview> {
        self.request(move |w| {
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
            w.stop();
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
            let directory = w.recovery_dir.clone();
            let result = changes::apply(w.device()?, &plan, &directory);
            match result {
                Ok(dto) => {
                    w.snapshot = Some(w.device()?.snapshot()?);
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
            w.stop();
            w.device = None;
            w.device = Some(NativeDevice::open()?);
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
    fn stop(&mut self) {
        if let Some(device) = self.device.as_mut()
            && let Err(e) = device.stop_monitor()
        {
            self.monitor.lock().expect("monitor").message = Some(e.message);
        }
        self.monitor.lock().expect("monitor").pause();
    }
}
