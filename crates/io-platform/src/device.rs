//! Native USB и снимок IO. Один владелец handle; никаких SET при подключении.

use crate::protocol;
use hidapi::{HidApi, HidDevice};
use io_core::keyboard::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

const DESCRIPTOR: &[u8] = &[
    0x06, 0x68, 0xff, 0x09, 0x61, 0xa1, 0x01, 0x09, 0x62, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08,
    0x95, 0x40, 0x81, 0x02, 0x09, 0x63, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x40, 0x91,
    0x02, 0xc0,
];
pub const BLOCKS: [(&str, u8, usize); 9] = [
    ("game", 0x11, 56),
    ("base", 0x12, 512),
    ("function", 0x16, 512),
    ("lighting", 0x13, 16),
    ("colors", 0x14, 512),
    ("actuation", 0x17, 1024),
    ("dks", 0x18, 1024),
    ("macros", 0x15, 400),
    ("info", 0x10, 48),
];

fn hid_error(_: hidapi::HidError) -> AppError {
    AppError::new(
        "deviceIo",
        "Нет ответа от USB-устройства. Закройте другие редакторы и переподключите клавиатуру.",
    )
}

pub struct NativeDevice {
    handle: HidDevice,
    pub identity: DeviceIdentity,
    pub notifications: VecDeque<Vec<u8>>,
    poisoned: bool,
    stop_simulation_on_drop: bool,
}

impl NativeDevice {
    pub fn open() -> Result<Self> {
        let api = HidApi::new().map_err(hid_error)?;
        let matches: Vec<_> = api
            .device_list()
            .filter(|d| {
                d.vendor_id() == 0x0c45
                    && d.product_id() == 0x80d6
                    && d.usage_page() == 0xff68
                    && d.usage() == 0x61
                    && d.interface_number() == 2
                    && d.product_string()
                        .is_some_and(|s| s.trim() == "IO Type 84 Magnetic White")
            })
            .collect();
        if matches.len() != 1 {
            return Err(AppError::new(
                "deviceSelection",
                if matches.is_empty() {
                    "Подключите IO Type 84 Magnetic White по USB."
                } else {
                    "Найдено несколько IO White. Оставьте подключённой одну клавиатуру."
                },
            ));
        }
        let handle = api.open_path(matches[0].path()).map_err(hid_error)?;
        let mut descriptor = [0; 4096];
        let n = handle
            .get_report_descriptor(&mut descriptor)
            .map_err(hid_error)?;
        if &descriptor[..n] != DESCRIPTOR {
            return Err(AppError::new(
                "descriptorChanged",
                "HID-интерфейс отличается от проверенной модели.",
            ));
        }
        let mut device = Self {
            handle,
            identity: DeviceIdentity {
                name: "IO Type 84 Magnetic White".into(),
                vendor_id: 0x0c45,
                product_id: 0x80d6,
                firmware: String::new(),
                frame_version: 0,
                rt_precision: 0,
            },
            notifications: VecDeque::new(),
            poisoned: false,
            stop_simulation_on_drop: false,
        };
        let info = device.read_block(0x10, 0, 48, None)?;
        if info[4..10] != [0x45, 0x0c, 0xd6, 0x80, 0x17, 0x01]
            || info[12..16] != [0x66, 0x01, 0x0c, 0x11]
            || info[29..32] != [0, 0, 0]
        {
            return Err(AppError::new(
                "unsupportedFirmware",
                "Этот режим и версия прошивки ещё не проверены. Нужна отдельная проверка протокола.",
            ));
        }
        device.identity.firmware = "1.17".into();
        Ok(device)
    }

    fn send(&self, frame: &[u8; 64]) -> Result<()> {
        let mut report = [0; 65];
        report[1..].copy_from_slice(frame);
        if self.handle.write(&report).map_err(hid_error)? != 65 {
            return Err(AppError::new(
                "shortWrite",
                "USB-пакет передан не полностью.",
            ));
        }
        Ok(())
    }

    fn queue_notification(&mut self, packet: &[u8]) {
        if self.notifications.len() == 2048 {
            self.notifications.pop_front();
        }
        self.notifications.push_back(packet.to_vec());
    }

    fn exchange_inner(&mut self, packet: &[u8; 64]) -> Result<Vec<u8>> {
        self.send(packet)?;
        let start = Instant::now();
        let mut input = [0; 64];
        while start.elapsed() < Duration::from_millis(1000) {
            let n = self
                .handle
                .read_timeout(&mut input, 20)
                .map_err(hid_error)?;
            if n == 0 {
                continue;
            }
            if n >= 2 && input[0] == 0x55 && input[1] >= 0xfa {
                self.queue_notification(&input[..n]);
                continue;
            }
            return protocol::response(packet, &input[..n]).map(Vec::from);
        }
        Err(AppError::new(
            "timeout",
            format!(
                "Команда {:02X} не ответила за 1 с. Сеанс остановлен; переподключите устройство.",
                packet[1]
            ),
        ))
    }

    pub(crate) fn exchange(&mut self, packet: &[u8; 64]) -> Result<Vec<u8>> {
        if self.poisoned {
            return Err(AppError::new(
                "sessionLost",
                "Сеанс требует переподключения.",
            ));
        }
        let result = self.exchange_inner(packet);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    pub fn read_block(
        &mut self,
        command: u8,
        address: u16,
        size: usize,
        query: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        if ![0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x33].contains(&command)
            || size == 0
            || size > 60000
            || usize::from(address) + size > 65536
            || query.is_some_and(|q| q.len() != size)
        {
            return Err(AppError::new("invalidRead", "Недопустимый запрос чтения."));
        }
        let mut out = Vec::with_capacity(size);
        for offset in (0..size).step_by(protocol::PAYLOAD_SIZE) {
            let count = (size - offset).min(protocol::PAYLOAD_SIZE);
            let payload = query.map_or(&[][..], |q| &q[offset..offset + count]);
            let frame = protocol::frame(
                command,
                address + offset as u16,
                count,
                payload,
                offset + count == size,
            )?;
            out.extend(self.exchange(&frame)?);
        }
        Ok(out)
    }

    pub fn snapshot(&mut self) -> Result<RawSnapshot> {
        let mut blocks = BTreeMap::new();
        for (name, command, size) in BLOCKS {
            blocks.insert(name.into(), self.read_block(command, 0, size, None)?);
        }
        let info = &blocks["info"];
        let limit = usize::from(u16::from_le_bytes([info[2], info[3]])).clamp(512, 60000);
        let directory = &blocks["macros"];
        let mut end = 400;
        for record in directory.chunks_exact(4) {
            let address = u32::from_le_bytes(record.try_into().expect("4 bytes")) as usize;
            if address == 0 {
                continue;
            }
            if address < 400 || address + 4 > limit {
                return Err(AppError::new(
                    "invalidMacro",
                    "Каталог макросов содержит адрес вне исследованного диапазона.",
                ));
            }
            let header = self.read_block(0x15, address as u16, 4, None)?;
            let count = usize::from(u16::from_le_bytes([header[0], header[1]]));
            if count % 2 != 0 || address + 4 + count * 2 > limit {
                return Err(AppError::new(
                    "invalidMacro",
                    "Повреждён размер аппаратного макроса.",
                ));
            }
            end = end.max(address + 4 + count * 2);
        }
        if end > 400 {
            blocks
                .get_mut("macros")
                .expect("macro block")
                .extend(self.read_block(0x15, 400, end - 400, None)?);
        }
        let snapshot = RawSnapshot {
            identity: self.identity.clone(),
            blocks,
        };
        snapshot.decode()?;
        Ok(snapshot)
    }

    pub fn start_monitor(&mut self) -> Result<()> {
        self.stop_simulation_on_drop = true;
        self.exchange(&protocol::frame(0x66, 0, 0, &[], false)?)?;
        Ok(())
    }

    pub fn stop_monitor(&mut self) -> Result<()> {
        if self.stop_simulation_on_drop {
            self.send(&protocol::frame(0x67, 0, 0, &[], false)?)?;
            self.stop_simulation_on_drop = false;
        }
        Ok(())
    }

    pub fn poll_notifications(&mut self, timeout_ms: i32) -> Result<()> {
        let mut input = [0; 64];
        let n = self
            .handle
            .read_timeout(&mut input, timeout_ms)
            .map_err(hid_error)?;
        if n >= 2 && input[0] == 0x55 && input[1] >= 0xfa {
            self.queue_notification(&input[..n]);
        }
        Ok(())
    }

    pub fn current_colors(&mut self) -> Result<Vec<LiveColor>> {
        let query: Vec<u8> = (0..128).flat_map(|id| [id, 0, 0, 0]).collect();
        let bytes = self.read_block(0x33, 0, 512, Some(&query))?;
        if bytes == query {
            return Err(AppError::new(
                "ambiguousColors",
                "Устройство вернуло только эхо запроса цветов. Текущие RGB не подтверждены.",
            ));
        }
        let mut seen = [false; 128];
        bytes
            .chunks_exact(4)
            .map(|b| {
                if b[0] >= 128 || seen[usize::from(b[0])] {
                    return Err(AppError::new(
                        "invalidColors",
                        "Неизвестные или повторяющиеся LED-адреса.",
                    ));
                }
                seen[usize::from(b[0])] = true;
                Ok(LiveColor {
                    led_id: b[0],
                    color: rgb(&b[1..4]),
                })
            })
            .collect()
    }
}

impl Drop for NativeDevice {
    fn drop(&mut self) {
        let _ = self.stop_monitor();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSnapshot {
    pub identity: DeviceIdentity,
    pub blocks: BTreeMap<String, Vec<u8>>,
}

fn rgb(b: &[u8]) -> Rgb {
    Rgb {
        r: b[0],
        g: b[1],
        b: b[2],
    }
}
fn binding(b: &[u8]) -> BindingRecord {
    BindingRecord {
        page: b[0],
        parameters: [b[1], b[2], b[3]],
    }
}
fn word(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

impl RawSnapshot {
    pub fn revision(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(self.identity.firmware.as_bytes());
        for (name, bytes) in &self.blocks {
            hash.update(name);
            hash.update(bytes);
        }
        format!("{:x}", hash.finalize())
    }

    pub fn decode(&self) -> Result<KeyboardSnapshot> {
        for (name, _, size) in BLOCKS {
            let bytes = self
                .blocks
                .get(name)
                .ok_or_else(|| AppError::new("invalidSnapshot", format!("Нет блока {name}")))?;
            if (name == "macros" && bytes.len() < size) || (name != "macros" && bytes.len() != size)
            {
                return Err(AppError::new(
                    "invalidSnapshot",
                    format!("Неверный размер блока {name}"),
                ));
            }
        }
        let r = &self.blocks;
        let keys = (0..128)
            .map(|slot| {
                let a = &r["actuation"][slot * 8..slot * 8 + 8];
                let c = &r["colors"][slot * 4..slot * 4 + 4];
                KeyConfiguration {
                    slot: slot as u8,
                    base: binding(&r["base"][slot * 4..slot * 4 + 4]),
                    function: binding(&r["function"][slot * 4..slot * 4 + 4]),
                    actuation: Actuation {
                        trigger_um: word(a, 2).saturating_mul(10),
                        press_um: word(a, 4).saturating_mul(10),
                        release_um: word(a, 6).saturating_mul(10),
                        rapid_trigger: a[1] & 1 != 0,
                        rampage: a[1] & 2 != 0,
                        axis_type: a[0],
                    },
                    led_id: c[0],
                    color: rgb(&c[1..]),
                }
            })
            .collect();
        let l = &r["lighting"];
        let g = &r["game"];
        let macro_data = &r["macros"];
        let mut macros = Vec::new();
        for (id, entry) in macro_data[..400].chunks_exact(4).enumerate() {
            let address = u32::from_le_bytes(entry.try_into().expect("4 bytes")) as usize;
            if address == 0 {
                continue;
            }
            if address < 400 || address + 4 > macro_data.len() {
                return Err(AppError::new("invalidMacro", "Недопустимый адрес макроса."));
            }
            let count = usize::from(word(macro_data, address));
            if count % 2 != 0 || address + 4 + count * 2 > macro_data.len() {
                return Err(AppError::new(
                    "invalidMacro",
                    "Макрос прочитан не полностью.",
                ));
            }
            let steps = macro_data[address + 4..address + 4 + count * 2]
                .chunks_exact(4)
                .map(|b| MacroStep {
                    key_code: b[2],
                    pressed: b[3] & 0x80 != 0,
                    delay_ms: word(b, 0),
                    kind: (b[3] >> 4) & 7,
                })
                .collect();
            macros.push(HardwareMacro {
                id: id as u8,
                steps,
            });
        }
        let dks = r["dks"]
            .chunks_exact(16)
            .enumerate()
            .map(|(index, b)| DksConfiguration {
                index: index as u8,
                thresholds: b[..4].try_into().expect("4 bytes"),
                actions: [b[5], b[7], b[9], b[11]],
                states: b[12..16].try_into().expect("4 bytes"),
            })
            .collect();
        Ok(KeyboardSnapshot {
            revision: self.revision(),
            identity: self.identity.clone(),
            keys,
            lighting: LightingSettings {
                mode: l[0],
                color: rgb(&l[1..4]),
                secondary_color: rgb(&l[5..8]),
                color_mode: l[8],
                brightness: l[9],
                speed: l[10],
                direction: l[11],
            },
            performance: PerformanceSettings {
                report_rate: g[5],
                top_dead_zone_um: u16::from(g[8]) * 10,
                bottom_dead_zone_um: u16::from(g[9]) * 10,
                key_delay: g[4],
            },
            macros,
            dks,
            macro_bytes_used: macro_data.len() as u32,
            macro_write_limit: 512,
        })
    }
}

#[cfg(test)]
pub(crate) fn fixture() -> RawSnapshot {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/evidence/read-snapshot.json")).unwrap();
    let names = [
        ("game", "game_mode"),
        ("base", "keymap"),
        ("function", "fn_keymap"),
        ("lighting", "led_effect"),
        ("colors", "custom_led"),
        ("actuation", "rt"),
        ("dks", "dks"),
        ("macros", "macro_directory"),
        ("info", "device_info"),
    ];
    RawSnapshot {
        identity: DeviceIdentity {
            name: "IO Type 84 Magnetic White".into(),
            vendor_id: 0xc45,
            product_id: 0x80d6,
            firmware: "1.17".into(),
            frame_version: 0,
            rt_precision: 0,
        },
        blocks: names
            .into_iter()
            .map(|(name, key)| {
                (
                    name.into(),
                    value["blocks"][key]["hex"]
                        .as_str()
                        .unwrap()
                        .split_whitespace()
                        .map(|v| u8::from_str_radix(v, 16).unwrap())
                        .collect(),
                )
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_fixture_and_refuses_corrupt_macro_offsets() {
        let mut raw = fixture();
        let dto = raw.decode().unwrap();
        assert_eq!(dto.keys.len(), 128);
        assert_eq!(dto.keys[49].actuation.trigger_um, 2230);
        assert_eq!(dto.lighting.mode, 20);
        assert!(dto.macros.is_empty());
        raw.blocks.get_mut("macros").unwrap()[0..4].copy_from_slice(&399u32.to_le_bytes());
        assert!(raw.decode().is_err());
    }
}
