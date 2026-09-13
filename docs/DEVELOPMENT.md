# Разработка каркаса

Актуально на 2026-09-13. Назначение продукта и запуск — в [корневом README](../README.md); последующие задачи — в [PLAN](PLAN.md). Этот файл описывает реализованную основу.

## Что находится в коде

| Путь | Назначение |
|---|---|
| crates/io-core | Независимые Rust-контракты прикладного слоя; сейчас AppInfo / DeviceAccess |
| crates/io-platform | Сведения о платформе; место будущих HID/Windows/storage adapters |
| crates/io-cli | help / version / info; HID ещё отсутствует |
| src-tauri | Окно, сборка ресурсов и команда app_info |
| src/App.vue | Навигация, состояния прототипа и проверка связи с Rust |
| src/shared/keyboard-view | 84 клавиши, координаты макета, локальный выбор |
| src/shared/contracts | Сгенерированные DTO и один Tauri adapter |
| tools | Отдельные исследовательские утилиты Python, не зависимости приложения |

Папки остальных доменов из архитектуры добавляются с реализацией. Пустые классы Device/Rule/Scene не выдаются за работающие агрегаты. Core не зависит от Tauri, hidapi или Win32. `default-members` workspace позволяют выполнять `cargo test` для core/platform/cli без desktop; `--workspace` включает Tauri.

## Контракты

Rust — источник DTO. После изменения `crates/io-core/src/application.rs` выполнить `npm run contracts` и закоммитить `src/shared/contracts/generated.ts` вместе с Rust-кодом. Контрактный тест обнаруживает расхождение. Версии приложения пока синхронизируются вручную в Cargo workspace, package.json, tauri.conf.json и UI.

`app_info` возвращает версию, ОС и `deviceAccess: notImplemented`. Это состояние реализации, а не результат USB discovery. `connectShell()` отличает browser preview, ответ desktop, загрузку и ошибку. Произвольного HID IPC нет. Shell/filesystem plugins не подключены.

## Геометрия клавиатуры

Источник положения клавиш — скриншот владельца от 2026-09-13. В архиве: `archive_data/layout/owner-layout-2026-09-13.png`, SHA-256 `e92c99f1f4266c1485df324271d007f288aa6232ee7e326b5250cba7ea7e7762`. Цвета скриншота не интерпретируются как подсветка. Модель отображается нейтрально до чтения RGB.

Координаты относительные, в масштабе изображения, не миллиметры и не стандартные U. Сохранены группы F-клавиш, положение Print/Home/End, Ins/PgUp/Del/PgDn, длинные Space и R-Shift, отдельный блок стрелок. `slot` сопоставляется с [проверенной картой](evidence/layout-reference.json) независимо от геометрии. LED-панель не размещается на схеме по догадке.

## Проверки

```powershell
npm ci
npm run check
cargo run -p io-cli -- --help
cargo run -p io-cli -- info
npm run desktop:build
python -m unittest discover -s tools -p 'test_*.py'
```

Python 3.10+ нужен только для исследовательских утилит. Rust-тест упаковки выполняется с `io-desktop/custom-protocol`, проверяет включение HTML/JS/CSS и отсутствие dev-режима. Он не заменяет проверку настоящего WebView и IPC.

Для ручной проверки закрыть dev-сервер, открыть `target/release/io-desktop.exe`, перейти в «О приложении»: ожидаются «Настольное приложение», платформа `windows` и «Ответ получен». Затем проверить выбор клавиши и раздел LED-панели. Фактический статус этого smoke test записывается в [STATUS](STATUS.md); сам список действий не означает, что он выполнен.

## Сборка ресурсов

Скрипты `desktop:build` и `desktop:bundle` явно включают Cargo feature `custom-protocol`, передаваемую Tauri. Режим `desktop:dev` использует Vite на localhost:1420. Обычный `cargo build -p io-desktop` не является командой упаковки автономного приложения. Проверяйте именно EXE из последней сборки Tauri: debug-файлы могут пересобираться другими Cargo-командами.

При изменении UI повторная native-сборка обязательна: готовый EXE содержит прежние ресурсы, пока не пересобран. Первый native build требует C++ toolchain и WebView2. Проверены VS 2022 Build Tools и WebView2 150.0.4078.65; это сведения об окружении, не минимальные версии.

Pinia, router, HID, runtime правил, хранилище и плагины добавляются при появлении соответствующих сценариев. Сейчас один Vue shell и локальное состояние не требуют этих зависимостей.
