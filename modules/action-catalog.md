# Каталог и исполнение действий

Актуальное дополнение 2026-09-14: [исключающий light/deep](exclusive-depth.md)
реализован как отдельный DepthChoice с каталогом и временным stock-захватом
PgUp/PgDn (base/Fn). Прежние описания ниже о независимых GestureRule сохраняются
для добавочного режима; ограничение «исходный ввод не подавляется» не относится
к этому принятому stock-маршруту. Полная модель objects.md ещё не реализована.


Актуально: 2026-09-14. Решение и расширения: [ADR-0006](../docs/adr/0006-actions-gestures-scenes.md).

Следующая модель по objects.md разделяет ActionType, настроенный ActionPreset,
ActionCall, FlowDefinition и ExecutionInstance/Lease — [OBJECT_MODEL](../docs/OBJECT_MODEL.md).
Это проект: текущий `ActionDefinition::Macro` и profile v2 ещё не изменены.
Сохранение ID/семантики старых действий и перенос macro в Flow —
[миграция](execution-plan.md); долговременное владение выходами требует gate G2.

## Входные файлы

- `io-core/src/automation.rs`: ActionDefinition, PlatformCommands, GestureRule, AutomationProfile; валидация, GestureEngine.
- `io-platform/src/action_runtime.rs`: отдельный worker, Windows SendInput/ShellExecuteW; очередь 16, срок ожидания 500 мс, отмена по поколению.
- `io-platform/src/service.rs`: проверка реальных слотов, последовательность HID/monitor/configure_automation.
- `features/editor/computer-rules.ts`: встроенный каталог, TS-валидация, миграция старых media-правил.
- `features/editor/workspace.ts`: общий черновик, применение, profile v2; `ActionCatalog.vue`, `CommandEditor.vue`, `DepthActions.vue`: интерфейс.

## Контракты

`validate_automation(profile)` проверяет модель, без исполнения. `configure_automation(profile, enabled)` проверяет также физические слоты и требует активного monitor при запуске. Флаг запуска не сохраняется. Автомат получает глубину через FB, время от Instant; выдаёт action ID. Sample и tick разделены для удержания без зависимости от частоты UI.

Лимиты: 256 действий и 256 жестов, до 8 клавиш в жесте; глубина 0,30–3,20 мм, возврат минимум на 0,10 мм выше. Удержание до 10 с (UI 0,25–3 с). До 128 шагов последовательности, 30 с суммарных задержек; тексты до 2000 символов, суммарно 4000 на последовательность. Это программные лимиты, не лимиты flash клавиатуры. Импорт проверяет вложенные шаги и варианты всех ОС.

`io.automation.v1` — применённый каталог/жесты. Если отсутствует, `io.rules.v1` мигрирует в действия с ID и жесты. `io.profiles.v1` остаётся ключом списка файлов, но новый документ имеет `schemaVersion:2` и поле `automation`. Чтение v1 поддержано. Старый редактор v1 не сможет импортировать документ v2; для rollback использовать экспорт/коммит до миграции. ADC и история на диск не пишутся.

## Применение и отказ

Аппаратные Edit проходят существующие prepare/apply/readback/recovery. Программная часть сохраняется отдельно в том же пользовательском потоке. Если аппаратная запись успешна, а программная часть отказала, новый аппаратный snapshot сохраняется, незавершённая программная часть остаётся в черновике. Не заявлять атомарную транзакцию Windows + USB. Ошибка runtime останавливает очередь, выводится в UI. Кнопка остановки жестов доступна в шапке из любого раздела.

SendInput исполняет обычный ввод в текущем активном окне. Исходные физические клавиши не подавляются. Выходные клавиши и модификаторы пользователя не отпускаются; конфликт прерывает действие. Приложения: фиксированный каталог Word/Notepad/Calculator, без shell-команд и передаваемых аргументов. Linux/macOS override сохраняются, но соответствующие adapters отсутствуют.

## Проверки

Core: непрерывность сочетания, удержание, rearm, потеря сигнала, неизвестные ссылки и лимиты. TS: миграция v1, профиль v2, вложенные повреждения. Playwright: PgUp → undo/redo → A+S hold Word → профиль → apply; проверяется включение monitor/configure_automation и отсутствие аппаратных SET при программном черновике. Это не физический тест выполнения Word или громкости.

Источники поведения Win32: [SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput), [ShellExecuteW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew).
