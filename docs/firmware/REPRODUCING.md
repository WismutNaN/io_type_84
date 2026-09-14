# Воспроизведение анализа White 1.17

Дата: 2026-09-14. Все команды ниже работают с файлами. Они не открывают HID/USB,
не входят в ISP и не создают изменённую прошивку. Корневой каталог — репозиторий.

## Входные данные

Источники и SHA-256 перечислены в [манифесте](../evidence/firmware-research-sources.json).
Основные материалы:

- [Официальный HEX IO](https://web.io.vision/IO_Type_84_Magnetic_White_V1.17.hex), SHA-256
  `8f5ef507771c6795258eb7521cfc1b46269a5b301548767ae87a3f1a83a50d45`.
- [WASM загрузчика](https://web.io.vision/assets/sn_isp_lib_bg-DOswDDJh.wasm), SHA-256
  `8816f7c9a112265943f0137d1d54b4c9fc13d87850803918d5e53a9569aacba6`.
- [SONiX.SN34F2_DFP.2.0.4.pack](https://liveupdate.sonix.com.tw/sonix/develop_tool/MCU/DFP/SONiX.SN34F2_DFP.2.0.4.pack),
  ZIP-контейнер SDK: startup, заголовок, SVD, схема starter kit и ELF алгоритма flash.
- [OpenAula](https://github.com/cvanelteren/OpenAula/tree/fc3a9200bffd62ac91ca3bf5b3aac56a906a530a),
  ревизия `fc3a9200bffd62ac91ca3bf5b3aac56a906a530a`. Приложения, patcher и flasher
  референса не запускаются. Названия `test`/`dry run` не означают отсутствие записи.

HEX находится в `archive_data/firmware/2026-09-13/`.
Остальные бинарные материалы и результаты — в
`archive_data/firmware_research/2026-09-14/`, OpenAula — в
`archive_data/reference_projects/OpenAula/`. Архив игнорируется Git.

## Что повторять

### Реальное преобразование HEX штатным WASM

```powershell
node tools/inspect_isp_payload.mjs `
  archive_data/firmware_research/2026-09-14/sn_isp_lib_bg-DOswDDJh.wasm `
  archive_data/firmware/2026-09-13/IO_Type_84_Magnetic_White_V1.17.hex `
  archive_data/firmware_research/2026-09-14/isp-payload.json
```

Скрипт проверяет хеши обоих входов. Все 53 host imports WASM заменены функциями,
которые завершают вызов ошибкой; разрешены только `__wbindgen_malloc` и
`snx_isp_load_hex_buffer`. У проверенного модуля нет start section.
`start_isp`, `wasm_initialize`, `__wbindgen_start` не вызываются.

Получено: 516 096 байт, SHA-256 payload
`0a2b5c43407c12a15539ccb094ea05a93fdfa788dacf6839498987622e352dfe`,
сумма LE u16 по модулю 65536 — `0x9BEE`, обращений к host imports — **0**.
Отчёт содержит метаданные и небольшие таблицы, не образ для записи.
[Сохранённый результат](../evidence/firmware-isp-payload.json).

### Независимая проверка структуры и атлас

```powershell
python tools/analyze_firmware_safety.py `
  archive_data/firmware/2026-09-13/IO_Type_84_Magnetic_White_V1.17.hex `
  --dfp archive_data/firmware_research/2026-09-14/sonix-dfp/SONiX.SN34F2_DFP.2.0.4 `
  --decompiled archive_data/firmware_research/2026-09-14/decompiled `
  --atlas archive_data/firmware_research/2026-09-14/readable `
  --output docs/evidence/firmware-safety-v1.17.json
```

Без SDK/декомпиляции можно опустить `--dfp`, `--decompiled`, `--atlas`.
Независимая Python-модель заполнения пропусков совпала с результатом реального WASM
по длине, SHA-256 и сумме. [Структурные свидетельства](../evidence/firmware-safety-v1.17.json).

### Ghidra и ручной разбор

Использованы Ghidra 12.1.3, Temurin JDK 21.0.12.1 и WABT 1.0.41.
[Команды импорта и экспорта](../../tools/ghidra/README.md).
`functions.json` содержит **436 кандидатов функций**, для них экспортирован псевдокод.
Это количество успешно обработанных кандидатов, а не процент восстановленного кода.
Скрипт атласа распределяет их по подсистемам и раскрывает значения literal pool.

| Локальный результат | Содержание |
|---|---|
| `readable/README.md` | Навигация по функциям и подсистемам |
| `decompiled/functions.json` | Адреса, результат декомпиляции, связи вызовов |
| `decompiled/listing.asm` | Ассемблер для проверки псевдокода |
| `ghidra-project/IO_White_V117.gpr` | Интерактивный проект IO |
| `isp-decompiled.dcmp`, `isp.wat` | Статическое представление WASM, основной ISP flow — `f_bb` |
| `flash-algorithm/` | Декомпиляция официального `SN34F280_504.FLM`; алгоритм не исполнялся |

Адреса таблиц и литералы можно подтвердить непосредственно в HEX. Имена 32 точек разметки
назначены исследователем; типы, сигнатуры и некоторые границы могут быть ошибочны.
Исходные имена, комментарии, build system и Boot ROM в HEX отсутствуют.
Экспорт **не является собираемым исходным кодом** и не даёт побайтово воспроизводимую сборку.

Повторный seed сначала столкнулся с уже размеченным TMode; скрипт исправлен для
существующего проекта, финальный запуск завершён без этой ошибки. Ранние имена
`flash_handle_lock/unlock` исправлены на `watchdog_stop/reload` после сверки с SDK.
Точка `0x51AE` — фрагмент switch маршрутизации действий, а не отдельная функция
смены эффекта; её имя также уточнено, автоматическая граница функции не принимается.
RAM 32 КиБ в Ghidra seed — минимальная аналитическая область, не ёмкость чипа.

## Проверки

```powershell
$env:PYTHONPATH = 'G:\APPS\io_type_84\archive_data\analysis_runtime;G:\APPS\io_type_84\tools'
python -m unittest discover -s tools -p 'test_*.py'
node --check tools/inspect_isp_payload.mjs
```

На исследовательской машине прошли 15 Python-тестов. Новые проверки: отказ от
неизвестного образа, запрет молча отбрасывать адреса вне USER ROM и сравнение с
независимым WASM-результатом. Для полного набора нужны локальный HEX и Capstone;
без исследовательских входов соответствующие тесты пропускаются, что надо указывать
в отчёте CI. Эти тесты не проверяют физическую запись или восстановление клавиатуры.

## Начальная RAM и подмена кадра полосы

Дополнительная offline-зависимость: Unicorn 2.1.4 из `requirements-research.txt`.
Его версия проверена по [официальному выпуску](https://github.com/unicorn-engine/unicorn/releases/tag/2.1.4).
Оба скрипта ниже проверяют SHA-256 HEX. В эмуляторе доступны только ROM/RAM;
инструкции ограничены конкретными подпрограммами, выполнение — счётчиком и тайм-аутом.
USB, SPI, FLASH controller и Boot ROM не отображены, основной цикл не запускается.

```powershell
python tools/decode_firmware_startup.py `
  archive_data/firmware/2026-09-13/IO_Type_84_Magnetic_White_V1.17.hex `
  --output docs/evidence/firmware-startup-v1.17.json `
  --ram-output archive_data/firmware_research/2026-09-14/initialized-ram.bin `
  --verify-emulation
python tools/verify_panel_compositor.py `
  archive_data/firmware/2026-09-13/IO_Type_84_Magnetic_White_V1.17.hex `
  --output docs/evidence/panel-compositor-v1.17.json
```

Первый сравнивает Python-распаковку с исполнением оригинальных `0x2F4/0x115C4`:
32 328 байт совпали, 52 274 инструкции. Восстановлены USB-интерфейсы и callbacks.
Второй исполняет оригинальные `0x13028/0x6C90`: подмена меняет только байты полосы,
сохраняет RGB-плоскости и восстанавливает исходный вывод следующим штатным кадром.
Затем в `0xD7C4` проверяется CAPS-пример: эхо stock firmware, сохранность флага
макроса и allowlist записей только в ответ/ready-флаги/стек. Сам USB не исполняется.
Расширенный Python-набор — 19 тестов при наличии обоих исследовательских пакетов.
Это проверка отдельных routines, не эмуляция полной платы и не аппаратная приёмка.

Ранний экспорт Ghidra не учитывал распаковку RAM, поэтому ссылки из сжатой таблицы
callbacks могли выглядеть как отсутствующие callers. `initialized-ram.bin` —
восстановленное начальное состояние, не дамп рабочего устройства. В первоначальный
Ghidra-проект оно автоматически не импортируется; границы `0x2D0/0x2F4` в старом
псевдокоде также нужно разделять по ассемблеру.
