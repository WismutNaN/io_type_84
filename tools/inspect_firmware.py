"""Проверка Intel HEX без исполнения прошивки и без доступа к устройствам."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re

MAX_INPUT_BYTES = 8 * 1024 * 1024
MAX_DATA_BYTES = 2 * 1024 * 1024


def parse_hex(text: str) -> tuple[dict[int, int], dict[str, object]]:
    memory: dict[int, int] = {}
    types: Counter[int] = Counter()
    base = 0
    eof = False
    entry = None
    for line_number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line:
            continue
        if eof:
            raise ValueError(f"Строка {line_number}: данные после EOF")
        if not re.fullmatch(r":[0-9a-fA-F]+", line):
            raise ValueError(f"Строка {line_number}: неверная запись HEX")
        try:
            record = bytes.fromhex(line[1:])
        except ValueError as error:
            raise ValueError(f"Строка {line_number}: нечётное число hex-цифр") from error
        if len(record) < 5 or len(record) != record[0] + 5:
            raise ValueError(f"Строка {line_number}: неверная длина")
        if sum(record) & 0xFF:
            raise ValueError(f"Строка {line_number}: неверная контрольная сумма")
        length, address, kind = record[0], int.from_bytes(record[1:3], "big"), record[3]
        data = record[4:-1]
        types[kind] += 1
        if kind == 0:
            if address + length > 0x10000 or base + address + length > 0x100000000:
                raise ValueError(f"Строка {line_number}: переход границы адреса")
            for offset, value in enumerate(data):
                target = base + address + offset
                if target in memory and memory[target] != value:
                    raise ValueError(f"Строка {line_number}: конфликт перекрывающихся данных")
                memory[target] = value
            if len(memory) > MAX_DATA_BYTES:
                raise ValueError("Превышен предел объёма данных")
        elif kind in (1, 2, 3, 4, 5):
            expected = {1: 0, 2: 2, 3: 4, 4: 2, 5: 4}[kind]
            if address != 0 or length != expected:
                raise ValueError(f"Строка {line_number}: неверный формат типа {kind}")
            if kind == 1:
                eof = True
            elif kind in (2, 4):
                base = int.from_bytes(data, "big") << (4 if kind == 2 else 16)
            else:
                candidate = (
                    int.from_bytes(data, "big") if kind == 5
                    else (int.from_bytes(data[:2], "big") << 4) + int.from_bytes(data[2:], "big")
                )
                if entry is not None and entry != candidate:
                    raise ValueError("Несогласованные точки входа")
                entry = candidate
        else:
            raise ValueError(f"Строка {line_number}: неизвестный тип {kind}")
    if not eof or not memory:
        raise ValueError("Не найдены данные или завершающая запись EOF")
    return memory, {
        "records": sum(types.values()),
        "record_types": {str(k): v for k, v in sorted(types.items())},
        "entry_address": hex(entry) if entry is not None else None,
    }


def inspect(raw: bytes) -> dict[str, object]:
    if len(raw) > MAX_INPUT_BYTES:
        raise ValueError("Превышен предел размера HEX-файла")
    memory, metadata = parse_hex(raw.decode("ascii"))
    addresses = sorted(memory)
    segments = []
    start = previous = addresses[0]
    for address in addresses[1:]:
        if address != previous + 1:
            segments.append((start, previous + 1))
            start = address
        previous = address
    segments.append((start, previous + 1))

    named_strings = []
    for start, end in segments:
        block = bytes(memory[a] for a in range(start, end))
        for match in re.finditer(rb"[ -~]{8,}", block):
            value = match.group().decode("ascii")
            if re.search(r"IO Type|\[USBD\]|SONIX|SN32|Cortex", value, re.I):
                named_strings.append({"address": hex(start + match.start()), "text": value})
    words = []
    if all(a in memory for a in range(32)):
        initial = bytes(memory[a] for a in range(32))
        words = [hex(int.from_bytes(initial[i:i + 4], "little")) for i in range(0, 32, 4)]
    cortex_candidate = bool(words) and (int(words[0], 16) & 0xFF000000) == 0x20000000 and (int(words[1], 16) & 1) == 1 and (int(words[1], 16) & ~1) in memory
    return {
        "format": "Intel HEX",
        "file_bytes": len(raw),
        "sha256": hashlib.sha256(raw).hexdigest(),
        "checksums_valid": True,
        **metadata,
        "data_bytes": len(memory),
        "address_span_bytes": addresses[-1] + 1 - addresses[0],
        "segments": [{"start": hex(a), "end_exclusive": hex(b), "bytes": b - a} for a, b in segments],
        "initial_words_little_endian": words,
        "selected_ascii_strings": named_strings[:50],
        "architecture_note": ("Первые слова совместимы с таблицей векторов Cortex-M; точный MCU не определён."
                              if cortex_candidate else "Недостаточно данных для гипотезы об архитектуре."),
        "limitations": ["Нет исполнения, записи на устройство или проверки подписи производителя.",
                        "Размер адресного диапазона не равен размеру установленной flash."]
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("--output", type=Path, help="Сохранить JSON-отчёт")
    args = parser.parse_args()
    try:
        if args.input.stat().st_size > MAX_INPUT_BYTES:
            raise ValueError("Превышен предел размера HEX-файла")
        result = inspect(args.input.read_bytes())
    except (OSError, ValueError) as error:
        parser.exit(1, f"Ошибка анализа: {error}\n")
    output = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        if args.output.resolve() == args.input.resolve():
            parser.exit(1, "Выходной файл не должен совпадать с прошивкой\n")
        args.output.write_text(output, encoding="utf-8")
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
