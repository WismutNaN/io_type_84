//! Модельные изменения IO: сначала план, затем snapshot/readback/recovery.
use crate::{
    device::{NativeDevice, RawSnapshot},
    layout::PHYSICAL_KEYS,
    protocol,
};
use io_core::keyboard::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub struct PreparedChange {
    pub token: String,
    pub before: RawSnapshot,
    pub after: RawSnapshot,
    pub blocks: Vec<String>,
}

fn invalid(message: &str) -> AppError {
    AppError::new("invalidChange", message)
}
fn validate_slots(slots: &[u8]) -> Result<()> {
    if slots.is_empty()
        || slots.len() > 84
        || slots
            .iter()
            .any(|slot| !PHYSICAL_KEYS.iter().any(|(s, _)| s == slot))
        || slots.iter().collect::<BTreeSet<_>>().len() != slots.len()
    {
        return Err(invalid("Выберите уникальные физические клавиши IO."));
    }
    Ok(())
}

fn validate_binding(value: &BindingRecord, slots: &[u8]) -> Result<()> {
    let [a, b, c] = value.parameters;
    let valid = match value.page {
        0 => a == 0 && b == 0 && c == 0,
        1 => {
            ((a == 1 && [1, 2, 4, 8, 16].contains(&b)) || (a == 3 && [1, 255].contains(&b)))
                && c == 0
        }
        2 => c == 0 && (b <= 0x73 || b == 0xaf || b >= 0xe0),
        3 => c == 0,
        6 => a < 100 && ((b == 1 && c > 0) || ([0, 2].contains(&b) && c == 0)),
        7 => {
            (a == 0 || (224..=231).contains(&a))
                && (b == 0 || (224..=231).contains(&b))
                && (a != 0 || b != 0)
                && (4..=115).contains(&c)
        }
        8 => a < 64 && b == 0 && c == 0,
        9 => a != 0 && b != 0 && (1..=100).contains(&c),
        10 => a != 0 && b == 0 && c == 0,
        11 | 12 => {
            slots.len() == 2
                && ((value.page == 11 && (1..=4).contains(&a)) || (value.page == 12 && a == 0))
                && PHYSICAL_KEYS
                    .iter()
                    .any(|&(slot, key)| slot == slots[0] && key == b)
                && PHYSICAL_KEYS
                    .iter()
                    .any(|&(slot, key)| slot == slots[1] && key == c)
        }
        13 => a == 0 && b == 0 && [11, 12, 13, 14, 15, 16, 17, 22, 27, 87, 91].contains(&c),
        _ => false,
    };
    if !valid {
        return Err(invalid(
            "Это назначение или сочетание параметров пока не поддерживается.",
        ));
    }
    Ok(())
}

fn macro_bytes(values: &[HardwareMacro]) -> Result<Vec<u8>> {
    if values.len() > 100 {
        return Err(invalid("Не более 100 слотов макросов."));
    }
    let mut ids = BTreeSet::new();
    let mut bytes = vec![0; 400];
    for value in values {
        if value.id >= 100 || !ids.insert(value.id) || value.steps.len() > 27 {
            return Err(invalid(
                "Неверный слот или слишком длинный макрос для проверяемого лимита.",
            ));
        }
        if value.steps.is_empty() {
            continue;
        }
        let mut held = BTreeSet::new();
        for step in &value.steps {
            if ![1, 3].contains(&step.kind)
                || step.key_code == 0
                || (step.kind == 3 && ![1, 2, 4, 8, 16].contains(&step.key_code))
            {
                return Err(invalid("Макрос поддерживает клавиатуру и кнопки мыши."));
            }
            let key = (step.kind, step.key_code);
            if step.pressed {
                if !held.insert(key) {
                    return Err(invalid("Повторное нажатие без отпускания в макросе."));
                }
            } else if !held.remove(&key) {
                return Err(invalid("Отпускание без нажатия в макросе."));
            }
        }
        if !held.is_empty() {
            return Err(invalid("В конце макроса все клавиши должны быть отпущены."));
        }
        let address = bytes.len() as u32;
        bytes[usize::from(value.id) * 4..usize::from(value.id) * 4 + 4]
            .copy_from_slice(&address.to_le_bytes());
        bytes.extend_from_slice(&(value.steps.len() as u16 * 2).to_le_bytes());
        bytes.extend([0, 0]);
        for s in &value.steps {
            bytes.extend(s.delay_ms.to_le_bytes());
            // White 1.17 0x3310: wire 3 dispatches keys, wire 1 mouse buttons.
            // Keep existing profile/domain kinds stable at this adapter boundary.
            let wire_kind = if s.kind == 1 { 3 } else { 1 };
            bytes.extend([
                s.key_code,
                (wire_kind << 4) | if s.pressed { 0x80 } else { 0 },
            ]);
        }
    }
    if bytes.len() > 512 {
        return Err(invalid(
            "Пока используется консервативный лимит 512 байт вместе с каталогом. Сократите макросы.",
        ));
    }
    Ok(bytes)
}

fn validate_existing_macros(raw: &RawSnapshot) -> Result<()> {
    raw.decode()?;
    let bytes = &raw.blocks["macros"];
    for entry in bytes[..400].chunks_exact(4) {
        let address = u32::from_le_bytes(entry.try_into().expect("word")) as usize;
        if address == 0 {
            continue;
        }
        let size = usize::from(u16::from_le_bytes([bytes[address], bytes[address + 1]])) * 2;
        if bytes[address + 2..address + 4] != [0, 0]
            || bytes[address + 4..address + 4 + size]
                .chunks_exact(4)
                .any(|s| s[3] & 15 != 0 || ![1, 3].contains(&((s[3] >> 4) & 7)))
        {
            return Err(invalid(
                "В текущих макросах есть неизвестные поля. Их запись отключена, чтобы сохранить исходные данные.",
            ));
        }
    }
    Ok(())
}

fn write_payload(name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut payload = bytes.to_vec();
    if name == "lighting" {
        payload[4] = 255;
        payload[14] = 0xaa;
        payload[15] = 0x55;
    }
    payload
}

pub fn prepare(before: &RawSnapshot, request: ChangeRequest) -> Result<PreparedChange> {
    if before.revision() != request.base_revision {
        return Err(AppError::new(
            "staleDraft",
            "Исходный снимок изменился. Перечитайте конфигурацию перед применением.",
        ));
    }
    if request.edits.is_empty() || request.edits.len() > 512 {
        return Err(invalid("План должен содержать от 1 до 512 изменений."));
    }
    let mut after = before.clone();
    for edit in request.edits {
        match edit {
            Edit::Binding {
                slots,
                function_layer,
                binding,
            } => {
                validate_slots(&slots)?;
                validate_binding(&binding, &slots)?;
                let block = after
                    .blocks
                    .get_mut(if function_layer { "function" } else { "base" })
                    .expect("validated snapshot");
                // Разрыв пары очищает обе записи, а не оставляет половину SOCD/RS.
                for slot in &slots {
                    let i = usize::from(*slot) * 4;
                    let old = block[i..i + 4].to_vec();
                    if [11, 12].contains(&old[0]) {
                        for record in block.chunks_exact_mut(4) {
                            if record == old {
                                record.fill(0);
                            }
                        }
                    }
                }
                for slot in slots {
                    let i = usize::from(slot) * 4;
                    block[i] = binding.page;
                    block[i + 1..i + 4].copy_from_slice(&binding.parameters);
                }
            }
            Edit::Actuation { slots, value } => {
                validate_slots(&slots)?;
                if !(100..=3200).contains(&value.trigger_um)
                    || [value.trigger_um, value.press_um, value.release_um]
                        .iter()
                        .any(|x| x % 10 != 0 || *x > 3200)
                    || (value.rapid_trigger && (value.press_um < 10 || value.release_um < 10))
                {
                    return Err(invalid("Ход: 0,10–3,20 мм, шаг 0,01 мм. RT: 0,01–3,20 мм."));
                }
                let block = after.blocks.get_mut("actuation").expect("block");
                for slot in slots {
                    let i = usize::from(slot) * 8;
                    // Модель оси и неизвестные флаги сохраняются для каждой позиции.
                    block[i + 1] = (block[i + 1] & !3)
                        | u8::from(value.whole_travel)
                        | (u8::from(value.rampage) << 1);
                    for (offset, um) in [
                        (2, value.trigger_um),
                        (
                            4,
                            if value.rapid_trigger {
                                value.press_um
                            } else {
                                0
                            },
                        ),
                        (
                            6,
                            if value.rapid_trigger {
                                value.release_um
                            } else {
                                0
                            },
                        ),
                    ] {
                        block[i + offset..i + offset + 2].copy_from_slice(&(um / 10).to_le_bytes());
                    }
                }
            }
            Edit::Lighting { value } => {
                if value.mode > 20
                    || value.color_mode > 1
                    || value.brightness > 5
                    || value.speed > 5
                    || value.direction > 1
                {
                    return Err(invalid("Недопустимые параметры эффекта."));
                }
                let b = after.blocks.get_mut("lighting").expect("block");
                b[0] = value.mode;
                b[1..4].copy_from_slice(&[value.color.r, value.color.g, value.color.b]);
                b[5..8].copy_from_slice(&[
                    value.secondary_color.r,
                    value.secondary_color.g,
                    value.secondary_color.b,
                ]);
                b[8..12].copy_from_slice(&[
                    value.color_mode,
                    value.brightness,
                    value.speed,
                    value.direction,
                ]);
                // Write-only маркеры SET добавляются в wire payload при отправке.
                // GET их не возвращает; это не часть сохранённой конфигурации.
            }
            Edit::Color { slots, color } => {
                validate_slots(&slots)?;
                let b = after.blocks.get_mut("colors").expect("block");
                for slot in slots {
                    let i = usize::from(slot) * 4;
                    if b[i] != slot {
                        return Err(invalid(
                            "LED-адрес этой позиции отличается: требуется проверка отображения.",
                        ));
                    }
                    b[i + 1..i + 4].copy_from_slice(&[color.r, color.g, color.b]);
                }
            }
            Edit::Performance { value } => {
                if ![3, 5, 6].contains(&value.report_rate)
                    || value.top_dead_zone_um > 500
                    || value.bottom_dead_zone_um > 500
                    || value.top_dead_zone_um % 10 != 0
                    || value.bottom_dead_zone_um % 10 != 0
                {
                    return Err(invalid(
                        "Частота: 1000/4000/8000 Гц; мёртвые зоны до 0,50 мм с шагом 0,01.",
                    ));
                }
                let b = after.blocks.get_mut("game").expect("block");
                b[5] = value.report_rate;
                b[8] = (value.top_dead_zone_um / 10) as u8;
                b[9] = (value.bottom_dead_zone_um / 10) as u8;
            }
            Edit::Macros { values } => {
                validate_existing_macros(&after)?;
                after.blocks.insert("macros".into(), macro_bytes(&values)?);
            }
            Edit::Dks { value } => {
                let clearing =
                    value.thresholds == [0; 4] && value.states == [0; 4] && value.actions == [0; 4];
                if value.index >= 64
                    || value.thresholds.iter().any(|x| *x > 32)
                    || (!clearing && value.thresholds[0] == 0)
                    || value.thresholds[0] > value.thresholds[1]
                    || value.thresholds[2] < value.thresholds[3]
                    || value.states.iter().any(|s| s & 15 & (s >> 4) != 0)
                    || value.actions.iter().enumerate().any(|(action, code)| {
                        *code == 0
                            && value
                                .states
                                .iter()
                                .any(|state| state & (0x11 << action) != 0)
                    })
                {
                    return Err(invalid("Проверьте порядок порогов и состояния DKS."));
                }
                let b = after.blocks.get_mut("dks").expect("block");
                let i = usize::from(value.index) * 16;
                b[i..i + 4].copy_from_slice(&value.thresholds);
                for (j, action) in value.actions.iter().enumerate() {
                    b[i + 5 + j * 2] = *action;
                }
                b[i + 12..i + 16].copy_from_slice(&value.states);
            }
        }
    }
    let decoded = after.decode()?;
    let advanced = |b: &BindingRecord| (8..=12).contains(&b.page);
    let physical = decoded
        .keys
        .iter()
        .filter(|k| PHYSICAL_KEYS.iter().any(|(s, _)| *s == k.slot))
        .collect::<Vec<_>>();
    if physical
        .iter()
        .any(|k| advanced(&k.base) && advanced(&k.function))
        || physical
            .iter()
            .map(|k| usize::from(advanced(&k.base)) + usize::from(advanced(&k.function)))
            .sum::<usize>()
            > 40
    {
        return Err(invalid(
            "До 40 расширенных назначений; одной физической клавише нельзя назначить их в обоих слоях.",
        ));
    }
    // Ссылки на макросы/таблицы после всей группы изменений.
    for key in &decoded.keys {
        for binding in [&key.base, &key.function] {
            if binding.page == 6
                && !decoded
                    .macros
                    .iter()
                    .any(|m| m.id == binding.parameters[0] && !m.steps.is_empty())
            {
                return Err(invalid(
                    "Назначение ссылается на отсутствующий макрос. Сначала исправьте ссылку.",
                ));
            }
            if binding.page == 8
                && !decoded.dks.iter().any(|d| {
                    d.index == binding.parameters[0] && d.thresholds.iter().any(|x| *x > 0)
                })
            {
                return Err(invalid("Назначение ссылается на пустую запись DKS."));
            }
        }
    }
    let blocks = [
        "macros",
        "dks",
        "actuation",
        "base",
        "function",
        "colors",
        "lighting",
        "game",
    ]
    .into_iter()
    .filter(|name| before.blocks[*name] != after.blocks[*name])
    .map(String::from)
    .collect();
    let token = after.revision();
    Ok(PreparedChange {
        token,
        before: before.clone(),
        after,
        blocks,
    })
}

impl PreparedChange {
    pub fn preview(&self) -> ChangePreview {
        ChangePreview {
            token: self.token.clone(),
            changes: self
                .blocks
                .iter()
                .map(|name| ChangeSummary {
                    block: name.clone(),
                    changed_bytes: self.before.blocks[name]
                        .iter()
                        .zip(&self.after.blocks[name])
                        .filter(|(a, b)| a != b)
                        .count() as u32
                        + self.before.blocks[name]
                            .len()
                            .abs_diff(self.after.blocks[name].len())
                            as u32,
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RecoveryRecord {
    pub schema_version: u8,
    pub before: RawSnapshot,
    pub target: RawSnapshot,
    pub blocks: Vec<String>,
}

pub fn save_recovery(directory: &Path, plan: &PreparedChange) -> Result<PathBuf> {
    fs::create_dir_all(directory)
        .map_err(|_| AppError::new("backupFailed", "Не удалось создать каталог восстановления."))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("Не удалось определить время операции."))?
        .as_nanos();
    let path = directory.join(format!("recovery-{stamp}.json"));
    let record = RecoveryRecord {
        schema_version: 1,
        before: plan.before.clone(),
        target: plan.after.clone(),
        blocks: plan.blocks.clone(),
    };
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|_| {
            AppError::new(
                "backupFailed",
                "Не удалось создать резервный снимок; запись отменена.",
            )
        })?;
    let bytes =
        serde_json::to_vec(&record).map_err(|_| invalid("Не удалось сериализовать снимок."))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| {
            AppError::new(
                "backupFailed",
                "Не удалось сохранить резервный снимок; запись отменена.",
            )
        })?;
    Ok(path)
}

pub fn apply(
    device: &mut NativeDevice,
    plan: &PreparedChange,
    directory: &Path,
) -> Result<ApplyResult> {
    if plan.blocks.is_empty() {
        return Err(invalid("Нет отличий от устройства."));
    }
    let current = device.snapshot()?;
    if current.revision() != plan.before.revision() {
        return Err(AppError::new(
            "externalChange",
            "Клавиатура изменилась после чтения. Запись отменена; перечитайте настройки.",
        ));
    }
    let mut backup_plan = plan.clone();
    if plan.blocks.iter().any(|n| n == "macros") {
        let old_len = backup_plan.before.blocks["macros"].len();
        let new_len = plan.after.blocks["macros"].len();
        if new_len > old_len {
            let tail = device.read_block(0x15, old_len as u16, new_len - old_len, None)?;
            backup_plan
                .before
                .blocks
                .get_mut("macros")
                .expect("block")
                .extend(tail);
        }
    }
    save_recovery(directory, &backup_plan)?;
    for name in &plan.blocks {
        let (get, set) = match name.as_str() {
            "game" => (0x11, 0x21),
            "base" => (0x12, 0x22),
            "function" => (0x16, 0x26),
            "lighting" => (0x13, 0x23),
            "colors" => (0x14, 0x24),
            "actuation" => (0x17, 0x27),
            "dks" => (0x18, 0x28),
            "macros" => (0x15, 0x25),
            _ => return Err(invalid("Неизвестный блок записи.")),
        };
        let bytes = &plan.after.blocks[name];
        // Сайт делит macro directory и тела в двух проходах с общим last flag.
        let ranges = if name == "macros" && bytes.len() > 400 {
            vec![(0, 400), (400, bytes.len())]
        } else {
            vec![(0, bytes.len())]
        };
        for (start, end) in ranges {
            for offset in (start..end).step_by(56) {
                let count = (end - offset).min(56);
                let payload = write_payload(name, &bytes[offset..offset + count]);
                let packet = protocol::frame(
                    set,
                    offset as u16,
                    count,
                    &payload,
                    offset + count == bytes.len(),
                )?;
                device.exchange(&packet).map_err(|error|AppError::new("applyUncertain",format!("Результат записи блока {name} неизвестен: {error} Резервный снимок сохранён.")))?;
            }
        }
        let readback = device.read_block(get, 0, bytes.len(), None)?;
        if readback != *bytes {
            return Err(AppError::new(
                "readbackMismatch",
                format!(
                    "Блок {name} после записи отличается. Дальнейшая запись остановлена; доступен резервный снимок."
                ),
            ));
        }
    }
    Ok(ApplyResult {
        snapshot: device.snapshot()?.decode()?,
        recovery_available: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::fixture;
    #[test]
    fn edit_one_key_preserves_unknown_bytes_and_other_slots() {
        let mut raw = fixture();
        raw.blocks.get_mut("actuation").unwrap()[49 * 8 + 1] = 0x80;
        let mut a = raw.decode().unwrap().keys[49].actuation.clone();
        a.trigger_um = 1200;
        a.rapid_trigger = true;
        a.press_um = 100;
        a.release_um = 200;
        let plan = prepare(
            &raw,
            ChangeRequest {
                base_revision: raw.revision(),
                edits: vec![Edit::Actuation {
                    slots: vec![49],
                    value: a,
                }],
            },
        )
        .unwrap();
        assert_eq!(plan.blocks, vec!["actuation"]);
        assert_eq!(plan.after.blocks["actuation"][49 * 8 + 1], 0x80);
        for i in 0..1024 {
            if !(49 * 8..49 * 8 + 8).contains(&i) {
                assert_eq!(
                    raw.blocks["actuation"][i],
                    plan.after.blocks["actuation"][i]
                );
            }
        }
        assert_eq!(
            plan.after.decode().unwrap().keys[49].actuation.trigger_um,
            1200
        );
    }
    #[test]
    fn stale_snapshot_invalid_slot_and_unbalanced_macro_are_rejected() {
        let raw = fixture();
        assert!(
            prepare(
                &raw,
                ChangeRequest {
                    base_revision: "old".into(),
                    edits: vec![]
                }
            )
            .is_err()
        );
        assert!(
            prepare(
                &raw,
                ChangeRequest {
                    base_revision: raw.revision(),
                    edits: vec![Edit::Color {
                        slots: vec![127],
                        color: Rgb { r: 1, g: 2, b: 3 }
                    }]
                }
            )
            .is_err()
        );
        assert!(
            macro_bytes(&[HardwareMacro {
                id: 0,
                steps: vec![MacroStep {
                    key_code: 4,
                    pressed: true,
                    delay_ms: 10,
                    kind: 1
                }]
            }])
            .is_err()
        );
    }
    #[test]
    fn macro_encoding_round_trip() {
        let values = vec![HardwareMacro {
            id: 2,
            steps: vec![
                MacroStep {
                    key_code: 4,
                    pressed: true,
                    delay_ms: 10,
                    kind: 1,
                },
                MacroStep {
                    key_code: 4,
                    pressed: false,
                    delay_ms: 20,
                    kind: 1,
                },
            ],
        }];
        let mut raw = fixture();
        raw.blocks
            .insert("macros".into(), macro_bytes(&values).unwrap());
        assert_eq!(raw.decode().unwrap().macros, values);
        validate_existing_macros(&raw).unwrap();
        raw.blocks.get_mut("macros").unwrap()[407] |= 1;
        assert!(validate_existing_macros(&raw).is_err());
    }
    #[test]
    fn macro_wire_types_match_original_firmware_not_profile_numbers() {
        let make = |kind| HardwareMacro {
            id: 0,
            steps: vec![
                MacroStep {
                    key_code: 4,
                    pressed: true,
                    delay_ms: 1,
                    kind,
                },
                MacroStep {
                    key_code: 4,
                    pressed: false,
                    delay_ms: 1,
                    kind,
                },
            ],
        };
        // Original ARM 0x3310 verified in stock-input-emulation-v1.17.json.
        let keyboard = macro_bytes(&[make(1)]).unwrap();
        assert_eq!(
            &keyboard[400..412],
            &[4, 0, 0, 0, 1, 0, 4, 0xB0, 1, 0, 4, 0x30]
        );
        let mouse = macro_bytes(&[make(3)]).unwrap();
        assert_eq!(
            &mouse[400..412],
            &[4, 0, 0, 0, 1, 0, 4, 0x90, 1, 0, 4, 0x10]
        );
        // Decode a firmware fixture independently of our encoder.
        let mut raw = fixture();
        let mut wire = vec![0; 400];
        wire[..4].copy_from_slice(&400u32.to_le_bytes());
        wire.extend([4, 0, 0, 0, 1, 0, 4, 0xB0, 1, 0, 4, 0x30]);
        raw.blocks.insert("macros".into(), wire);
        assert_eq!(raw.decode().unwrap().macros, vec![make(1)]);
    }
    #[test]
    fn dks_action_validation_uses_all_phases_for_each_action() {
        let raw = fixture();
        let request = |actions, states| ChangeRequest {
            base_revision: raw.revision(),
            edits: vec![Edit::Dks {
                value: DksConfiguration {
                    index: 0,
                    thresholds: [16, 30, 29, 16],
                    actions,
                    states,
                },
            }],
        };
        // Action 1 at final release: phase 4 does not require action 4.
        assert!(prepare(&raw, request([75, 0, 0, 0], [0, 0, 0, 1])).is_ok());
        // Phase 1 references missing action 2, even if action 1 is present.
        assert!(prepare(&raw, request([75, 0, 0, 0], [2, 0, 0, 0])).is_err());
        assert!(prepare(&raw, request([75, 185, 0, 0], [0, 2, 0, 1])).is_ok());
    }
    #[test]
    fn lighting_write_markers_are_not_expected_in_readback() {
        let raw = fixture();
        let mut lighting = raw.decode().unwrap().lighting;
        lighting.brightness = if lighting.brightness == 5 { 4 } else { 5 };
        let plan = prepare(
            &raw,
            ChangeRequest {
                base_revision: raw.revision(),
                edits: vec![Edit::Lighting { value: lighting }],
            },
        )
        .unwrap();
        assert_eq!(plan.preview().changes[0].changed_bytes, 1);
        let wire = write_payload("lighting", &plan.after.blocks["lighting"]);
        assert_eq!([wire[4], wire[14], wire[15]], [255, 0xaa, 0x55]);
        for i in [4, 14, 15] {
            assert_eq!(plan.after.blocks["lighting"][i], raw.blocks["lighting"][i]);
        }
    }
}
