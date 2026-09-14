# Разработка приложения

Актуально: 2026-09-14. Запуск и назначение — [README](../README.md), фактические ограничения — [STATUS](STATUS.md).

## Код и границы

| Путь                                               | Ответственность                                                                       |
| -------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `crates/io-core/src/keyboard.rs`                   | Независимые serde/ts-rs DTO конфигурации, Edit, ChangePreview, MonitorFrame, AppError |
| `crates/io-platform/src/protocol.rs`               | Строгие AA/55 кадры и проверка ответов                                                |
| `crates/io-platform/src/device.rs`                 | hidapi, точная модель White 1.17, allowlist GET, raw snapshot/decode                  |
| `crates/io-platform/src/changes.rs`                | Модельная валидация и patch сохраняемых байтов, backup, SET/readback                  |
| `crates/io-platform/src/service.rs`                | Единственный владелец HID, bounded queue, watchdog, recovery                          |
| `crates/io-platform/src/monitor.rs`                | Разбор FB, свежесть, гистерезис и 20 событий в памяти                                 |
| `crates/io-cli`                                    | Диагностика info/snapshot/colors/monitor N; без настроечных SET                       |
| `src-tauri/src/lib.rs`                             | Composition root, async IPC через spawn_blocking, остановка worker при Exit           |
| `src/features/editor/workspace.ts`                 | Состояние UI, группы undo/redo, IPC, local profiles                                   |
| `src/features/editor/model.ts`                     | Проекция Edit в черновик, имена действий, проверка собственной JSON-схемы             |
| `src/features/editor/io-vision-profile.ts`         | Частичный импорт сайта: только известные поля, отдельный отчёт                        |
| `src/features/editor/*Editor.vue`, `Inspector.vue` | Формы макросов, DKS и настройки выделенной группы                                     |
| `src/shared/keyboard-view`                         | Реальная геометрия 84 клавиш, logical code, sparse slot и отображение                 |
| `src/App.vue`                                      | Рабочее пространство, редактор света, профили/параметры, применение                   |

Core не зависит от Tauri/Win32/hidapi. IO-specific byte layout и ограничения относятся к platform adapter. Разделение будущих доменных агрегатов описано в `modules/`, но наличие предложенного типа там не означает его реализации. Pinia/router и отдельный процесс агента пока не требуются.

## IPC и поток данных

Rust — источник DTO: после изменения `keyboard.rs`/`application.rs` выполнить `npm run contracts`. Контрактный тест проверяет совпадение с committed TS.

Команды: `app_info`, `connect_device`, `disconnect_device`, `refresh_device`, `set_monitor`, `monitor_frame`, `clear_history`, `prepare_changes`, `apply_changes`, `prepare_recovery`. Произвольного raw HID, shell и filesystem IPC нет. `app_info.deviceAccess = available` означает реализацию транспорта, а не подключённую клавиатуру.

Worker принимает до 16 работ, последовательно исполняет команды, дренирует до 64 входящих уведомлений за итерацию. UI получает агрегированное состояние раз в 60 мс без перекрытия polling. Каждый HID-обмен ограничен 1 с. После timeout — новый handle; автоматического повтора SET нет. После начала apply UI ждёт фактического завершения, ранняя «отмена» не предоставляется.

## Хранение

- Локальные профили: WebView localStorage `io.profiles.v1`, до 40 документов; экспорт schemaVersion 2 с конфигурацией устройства, каталогом и жестами, чтение v1 поддержано. Суффикс ключа localStorage не является версией JSON. Browser preview и desktop имеют разные origin/storage. Сцены ещё не входят в формат. Чтение повреждённых полей отклоняется до замены состояния.
- Recovery: Tauri `app_data_dir()/recovery/recovery-<nanoseconds>.json`. На Windows обычно `%APPDATA%/io.github.wismutnan.io-type-84/recovery`. Raw before/target и список блоков, schema_version 1. Файл создаётся уникальным именем, `create_new` + `write_all` + `sync_all` до SET; существующие файлы не перезаписываются. Автоматической очистки пока нет.
- При росте макрообласти дополнительные затрагиваемые байты сначала читаются и добавляются в backup. Восстановление последней операции — тоже отдельный план с новым backup и GET-сверкой.
- История нажатий и ADC не пишутся в профили/на диск. CLI monitor печатает ограниченную диагностику в stdout; постоянного фонового логирования нет.

Для самостоятельного фона спроектирован перенос применённого профиля в доступный
Rust repository с миграцией и откатом; он ещё не реализован. [Companion](../modules/companion.md).

## Проверки и запуск

```powershell
npm ci
npm run check
npm run desktop:dev
npm run desktop:build
cargo run -p io-cli -- info
cargo run -p io-cli -- snapshot
cargo run -p io-cli -- monitor 20
```

`check`: Prettier, Vue/TS, Vite, TS-тесты модели/импорта и старения телеметрии, rustfmt, Rust workspace tests, Clippy `-D warnings`. Rust-тест упаковки требует `io-desktop/custom-protocol` и проверяет наличие HTML/JS/CSS. Native UI проверяется отдельно.

Аппаратные исследовательские examples `verify_lighting` и `verify_configuration` исполняются только с явным `--run`. Они временно меняют конкретные блоки, сохраняют резервные снимки в игнорируемый архив и восстанавливают исходное. Не включать их в CI. При ошибке восстановления остановиться и использовать сохранённый снимок; не повторять SET вслепую.

Release EXE содержит frontend. Изменение Vue требует повторного `desktop:build`. Обычный `cargo build -p io-desktop` не заменяет упаковку Tauri. `desktop:dev` использует Vite на 1420; `npm run dev` даёт browser preview без native HID. Python нужен только для `tools/`.

## Ручные сценарии

Подключить IO и проверить 1.17 → выбрать A/WASD → подготовить черновик → отменить/вернуть группу. В свете создать градиент, проверить заданные цвета и отсутствие выдуманного текущего RGB. Сохранить/экспортировать профиль; импортировать повреждённый JSON и убедиться, что рабочий снимок остаётся. Для apply подготовить конкретный diff, проверить GET, затем recovery.

Live-тесты выполнять физическими нажатиями, а не инъекцией клавиатуры через ОС: последняя не проверяет магнитный датчик. Не выдавать произвольные два входящих адреса за доказательство всей раскладки.

Геометрия взята со скриншота владельца и связана с [картой слотов](evidence/layout-reference.json). Условный значок панели не утверждает её топологию. При ширине окна меньше 1280 px инспектор размещается ниже клавиатуры, чтобы не уменьшать подписи до нечитаемого размера. Диалоги удерживают фокус, поддерживается клавиатурный выбор и reduced motion.

## Взаимодействие, темы и компьютерные действия

Текущий код и границы: [каталог действий](../modules/action-catalog.md), [модуль редактора](../modules/editor-interaction.md). IPC `validate_automation(profile)` и `configure_automation(profile, enabled)` заменяют `configure_rules`. Новые DTO генерируются из `io-core/automation.rs`. `set_history_capacity(count)` ограничивает серверную историю по геометрии интерфейса.

`npm run test:ui` использует Playwright и Microsoft Edge (`channel: msedge`), поднимает Vite при необходимости. Скриншоты тестового профиля сохраняются в игнорируемом archive_data. Подставной IPC только в тестах, не в production-приложении. `io.profiles.v1` теперь хранит документы schemaVersion 2 с automation; v1 читается с миграцией. Старые `io.rules.v1` преобразуются при отсутствии `io.automation.v1`. Перезапуск не включает runtime. `io.locale`/`io.theme` — предпочтения приложения.

## Проверка штатного ввода

[Метод и результаты](firmware/stock-input-v1.17.md). Offline-стенд:
`tools/verify_stock_input.py`; примитивы выполняются исходным ARM-кодом.
Аппаратный пример `cargo run -p io-platform --example verify_input_ownership`
по умолчанию только читает и показывает diff. `--observe` / `--modifiers` читают
Raw Input без SET; `--run` требует интерактивного терминала с ручным Enter между
фазами, временно меняет base PgUp/PgDn, измеряет и восстанавливает snapshot.
При ошибке измерения пытается восстановить назначения; при обрыве процесса/USB
нужен сохранённый recovery-файл. Полного backup OS-mode/всех областей нет.
Интерактивная процедура пройдена: [приёмка](evidence/stock-capture-acceptance-v1.17.json). Аппаратные тесты не входят в CI.

Доменный `MacroStep.kind`: 1 клавиатура, 3 мышь. Wire White 1.17: соответственно
3 и 1. Это разные словари; преобразование выполняют encoder/decoder adapter.


## Выбор light/deep

[Контракт](../modules/exclusive-depth.md). `AutomationProfile.depthChoices` с serde
и TS-миграцией отсутствующего поля. `prepare_automation(profile, baseRevision)`
возвращает отдельный ChangePreview захвата; `configure_automation` принимает
`ownershipToken`, включает monitor самостоятельно и восстанавливает назначения
при выключении. Обычные аппаратные Edit и временный захват имеют разные планы.
Исполнение не стартует от импорта/чтения. Долговременный фон пока не реализован.
Журнал: `recovery/input-ownership.json`; снимки временных операций:
`recovery/input-sessions/`, отдельно от пользовательской истории восстановления.
`verify_depth_choice --run` использует production service и наблюдает отмеченный
SendInput; его hook существует только в исследовательском executable.
