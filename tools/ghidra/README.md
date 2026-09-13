# Статический проект Ghidra

Скрипты предназначены только для официального IO White 1.17 HEX с SHA-256
`8f5ef507771c6795258eb7521cfc1b46269a5b301548767ae87a3f1a83a50d45`.
У них нет HID, USB или прошивальщика. Они создают аналитические области RAM/MMIO,
размечают точки входа и экспортируют результат декомпилятора.

Проверено с Ghidra 12.1.3 и Temurin JDK 21.0.12.1. Сторонние зависимости, проекты
Ghidra и весь декомпилированный код остаются в игнорируемом `archive_data/`.
Общие инструкции Ghidra: https://github.com/NationalSecurityAgency/ghidra.

Из корня репозитория, PowerShell; `$env:JAVA_HOME` должен указывать на JDK 21:

```powershell
New-Item -ItemType Directory -Force archive_data/firmware_research/2026-09-14/ghidra-project
& 'archive_data/firmware_research/2026-09-14/runtime/ghidra_12.1.3_PUBLIC/support/analyzeHeadless.bat' `
  'archive_data/firmware_research/2026-09-14/ghidra-project' IO_White_V117 `
  -import 'archive_data/firmware/2026-09-13/IO_Type_84_Magnetic_White_V1.17.hex' `
  -processor ARM:LE:32:Cortex -cspec default -scriptPath tools/ghidra `
  -preScript SeedIoFirmware.java `
  -postScript ExportIoFirmware.java 'G:/APPS/io_type_84/archive_data/firmware_research/2026-09-14/decompiled' `
  -max-cpu 4 -analysisTimeoutPerFile 300
```

Для уже импортированного проекта заменить `-import ...` на
`-process IO_Type_84_Magnetic_White_V1.17.hex`.
Скрипт проверяет SHA-256 исходного файла в метаданных Ghidra перед разметкой.

`functions.json` — найденные кандидаты функций и связи вызовов;
`functions/*.c` — C-подобный псевдокод; `listing.asm` — инструкции.
Имена, типы и границы функций могут быть неверны. Анализ не находит автоматически
все косвенные вызовы. Это **не исходный код и не собираемая прошивка**.
Нельзя компилировать экспорт или переносить его типы в реализацию без проверки
ассемблера. Область RAM 32 КиБ в Seed — минимум, необходимый текущему анализу,
а не утверждение о физической ёмкости MCU.

Контекст и выводы: [исследование](../../docs/FIRMWARE_RESEARCH.md).
