# Модуль: инфраструктурные адаптеры

**Реализация 2026-09-14:** hidapi 2.6.7, IO AA/55 v0 codec, actor, JSON recovery — `crates/io-platform`. Tauri IPC не принимает raw HID. Windows WebView локальные профили и IO Vision импорт реализованы в Vue; ProfileRepository/OS runtime из таблиц ниже пока целевой дизайн. [Фактическая структура](../docs/DEVELOPMENT.md), [проверки SET/GET](../docs/evidence/native-write-roundtrip.json).

**Ответственность:** native HID, формат IO, Windows API, хранилище и привязка к Tauri. **Расположение:** `crates/io-platform/src/`, `crates/io-cli/src/`, `src-tauri/`.

## Публичный интерфейс

| Адаптер | Реализуемая граница | Особенности |
|---|---|---|
| NativeHidTransport | HidTransport | open_path, report I/O, тайм-ауты, освобождение handle |
| IoVisionV0Protocol | DevicePort | Codec GET/SET, проверка raw, сохранение неизвестных полей |
| WindowsInputSource | InputSource | Raw Input, идентичность источника, background window |
| WindowsActionExecutor | ActionExecutor | Конкретные API громкости/приложений/ввода |
| JsonProfileRepository | ProfileRepository | Атомарная замена, backup, проверка schemaVersion |
| TauriCommands | Прикладные use cases | Типизированные DTO, capabilities, проверка параметров |
| DiagnosticCli / SimulatedDevice | Те же порты | Реальное чтение / воспроизводимые сценарии ошибок |

## Инварианты

- HID worker владеет handle и единственной очередью операций одного физического устройства. UI не выполняет блокирующее чтение.
- Одна транзакция в полёте. Тайм-аут изолирует поздние ответы; reply сопоставляется по команде, адресу, длине и поколению сеанса.
- Не предполагать transaction ID, atomic commit или save-команду, пока они не найдены.
- Первоначальный CLI содержит allowlist GET. SET/калибровка/сброс/OTA отсутствуют в диагностическом интерфейсе.
- Все записи сохраняют исходные неизвестные байты; не копировать сериализатор сайта, обнуляющий резервное поле, без проверки.
- Подписки и native callbacks быстро кладут события в bounded queue. Действия/скрипты не выполняются в callback.
- Генерация TS-контрактов проверяется сборкой или контрактным тестом; ручные несовместимые копии структур не допускаются.
- Файлы записи создаются рядом с целевым файлом; Windows replace/flush и recovery проверяются при завершении процесса в неудобный момент.

## Зависимости

Зависит от core, Rust hidapi, выбранных Windows bindings и сериализации. Tauri — composition root, без бизнес-правил. `io-core` никогда не импортирует этот модуль.

## Намеренно не обрабатывает

Новые доменные политики, автоматическое разрешение конфликтов правил и неизвестную прошивку через угадывание команд.

## Заметки для реализации

Спецификация [IO](../docs/PROTOCOL.md), [снимки](../docs/evidence/read-snapshot.json), [hidapi API](https://docs.rs/hidapi/latest/hidapi/struct.HidDevice.html). Реальное открытие работает без установки нашего драйвера. Проверить отдельный набор сценариев: занятость, два экземпляра, сон, unplug, поздний ответ, короткий пакет и недоступный MI_03. Симулятор не должен изображать неподдержанную функцию успешным пустым результатом.
