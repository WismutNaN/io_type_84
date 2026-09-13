use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => help(),
        [arg] if arg == "--help" || arg == "-h" => help(),
        [arg] if arg == "--version" || arg == "-V" => {
            println!("io-cli {}", env!("CARGO_PKG_VERSION"));
        }
        [arg] if arg == "info" => {
            match serde_json::to_string_pretty(&io_platform::application_info()) {
                Ok(info) => println!("{info}"),
                Err(error) => {
                    eprintln!("Не удалось сформировать сведения: {error}");
                    return ExitCode::FAILURE;
                }
            }
        }
        [arg] if arg == "snapshot" || arg == "colors" => {
            let result = io_platform::device::NativeDevice::open().and_then(|mut device| {
                if arg == "colors" {
                    device
                        .current_colors()
                        .map(|v| serde_json::to_value(v).expect("DTO"))
                } else {
                    device
                        .snapshot()
                        .and_then(|s| s.decode())
                        .map(|v| serde_json::to_value(v).expect("DTO"))
                }
            });
            match result {
                Ok(value) => println!("{}", serde_json::to_string_pretty(&value).expect("JSON")),
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            }
        }
        [arg, seconds] if arg == "monitor" => {
            let Ok(seconds) = seconds.parse::<u64>() else {
                return ExitCode::from(2);
            };
            if !(1..=60).contains(&seconds) {
                eprintln!("Продолжительность: 1–60 секунд.");
                return ExitCode::from(2);
            }
            let result = io_platform::device::NativeDevice::open().and_then(|mut device| {
                device.start_monitor()?;
                eprintln!("Измерение хода включено на {seconds} с. Нажимайте клавиши; затем режим завершится автоматически.");
                let start=std::time::Instant::now();
                let mut state=io_platform::monitor::MonitorState::default();
                while start.elapsed()<std::time::Duration::from_secs(seconds) {
                    device.poll_notifications(10)?;
                    while let Some(packet)=device.notifications.pop_front() {state.ingest(&packet);}
                }
                device.stop_monitor()?;
                let frame=state.frame();
                Ok(serde_json::json!({"packets":frame.packets,"distinctSlots":frame.travel.len(),"maxTravelUm":frame.travel.iter().map(|k|k.travel_um).max(),"recentPhysicalPresses":frame.history.len(),"peakTravelUm":frame.history.iter().map(|k|k.peak_um).max(),"stopSent":true}))
            });
            match result {
                Ok(value) => println!("{value}"),
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            }
        }
        _ => {
            eprintln!("Неизвестная команда. Используйте io-cli --help.");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

fn help() {
    println!(
        "IO Type 84 — диагностический CLI\n\n  info          Сведения о приложении\n  snapshot      Прочитать конфигурацию по USB в JSON\n  colors        Запросить текущие RGB\n  monitor N     Измерять ход N секунд (1–60), затем выключить тест\n  --help        Эта справка\n  --version     Версия CLI\n\nSnapshot и colors только читают. Monitor временно включает simulation 66/67."
    );
}
