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
        _ => {
            eprintln!("Неизвестная команда. Используйте io-cli --help.");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

fn help() {
    println!(
        "IO Type 84 — диагностический CLI\n\nИспользование: io-cli [info | --help | --version]\n\n  info       Сведения о приложении и платформе в JSON\n  --help     Эта справка\n  --version  Версия CLI\n\nHID-транспорт пока не реализован. Команды не обращаются к клавиатуре."
    );
}
