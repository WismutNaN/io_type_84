# AK820 Pro Modder

Источник: [wsclx/ak820pro-modder](https://github.com/wsclx/ak820pro-modder), локальный commit `fe93c13972445f92b00b485875da2811173eb940`. Корневой `LICENSE` — MIT, copyright 2026 wsclx. Исходники остаются в `archive_data/reference_projects/ak820pro-modder/`. Это Rust + Tauri + React, а не Vue. Заявленные проверки автора относятся к AK820 Pro, не IO.

## Что читать разработчику адаптера

| Файл в референсе | Полезное | Ограничение для IO |
|---|---|---|
| `crates/ak820-protocol/src/protocol.rs` | AA/55, 64 байта, header 8, payload 56, LE address, last flag | Комментарий FF67 устарел относительно фактического выбора FF68 в `device.rs` |
| `src/device.rs` того же crate | hidapi report ID=0, FF68 в начале списка, порционное чтение | Fallback на interface 3 и произвольный vendor usage нам не подходит |
| `src/commands/system.rs` | Поля identity по тем же смещениям; raw macro capacity | Не все поля означают реальные возможности этой модели |
| `src/commands/keymap.rs` | Четырёхбайтовые назначения и специальные page types | Проверять семантику расширенных действий по IO |
| `src/commands/lighting.rs` | Общий блок эффекта, индивидуальные RGB | `Custom=0x80`; у IO наблюдался mode=20. Не переносить enum как общий |
| `docs/PROTOCOL.md`, `docs/HANDOFF.md`, корневой README | TFT, протокол, источники аппаратного разбора | Часть заметок и комментариев описывает более ранние выводы |

`src/lib.rs` выбирает PID `8009`, `FEFE`, `7140`, не `80D6`. Успех offline framing-теста не делает приложение готовым драйвером IO. 81 физическая клавиша AK820 и наш макет 84 также требуют разных карт.

## Что улучшить в собственной реализации

В исследованном `device.rs` проверяется magic и command, но не адрес/размер ответа. Ответы короче запрошенного могут приниматься как успешные; `write_output_report` не проверяет число переданных байтов; `set` игнорирует ошибку ожидания ACK. `build_frame` обрезает избыточный payload, а разбор DeviceInfo дополняет короткий ответ нулями. Это конкретные причины использовать наш строгий codec и правила [PROTOCOL](../PROTOCOL.md), а не переносить транспорт целиком.

Чтение 128 × 4, размер header и common command IDs — полезные независимые подтверждения. RT/DKS для магнитной IO этот механический профиль не доказывает. Его TFT через крупный endpoint и команды `50/41` также не идентифицирует нашу LED-панель: экран AK820 — другая световая поверхность.

## Цепочка к железу и прошивке

[fpb/ajazz-ak820-pro](https://github.com/fpb/ajazz-ak820-pro) содержит аппаратное исследование, stock BIN и рабочие варианты QMK для AK820. Автор указывает HFD80CP100, предполагаемое родство с SN32F299, отдельный wireless MCU CH582F и внешнюю flash. Это сведения об AK820, не измерение IO.

Следующие первичные референсы: [ветки QMK автора](https://github.com/fpb/qmk_firmware), [SonixFlasherC](https://github.com/SonixQMK/SonixFlasherC), [официальный SN32F299 manual](https://www.sonix.com.tw/webapi/fl219869/SN32F299_V1.8_EN.pdf). Flasher ориентирован на Sonix SN32F2xx, предлагает отдельный HFD reboot и таблицу bootloader IDs, включая `0C45:7140` для SN32F29x. Наш текущий `80D6` — application ID; bootloader IO не наблюдался.

Не переносить из AK820 адреса прошивки, erase ranges, pin-short вход в bootloader или обещание восстановления. В IO HEX найден признак другого ISA; [сопоставление](../FIRMWARE_RESEARCH.md#сопоставление-с-ak820-и-sonix). Совпадение AA/55 или VID поддерживает родство configurator SDK, но не устанавливает производителя платы и процессор.

Также найдены предшественники `gohv/EPOMAKER-Ajazz-AK820-Pro` и `TaxMachine/ajazz-keyboard-software-linux`. Описанный ими другой framing с report ID 4/другими маркерами не соответствует нашим сохранённым запросам. Текущий Rust-референс сам объясняет отказ от этого пути.

Для переноса MIT-кода сохранять его уведомления. В этой итерации переноса в продукт нет; [offline harness](../../tools/compare_reference_codecs.py) компилирует только локальную `build_frame` во временный тест без Cargo, зависимостей и device-кода.
