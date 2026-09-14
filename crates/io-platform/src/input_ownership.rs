//! Scoped stock bindings for exclusive host rules. No arbitrary packet API.
use crate::{changes::PreparedChange, device::RawSnapshot};
use io_core::{
    automation::AutomationProfile,
    keyboard::{AppError, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const SILENT: [u8; 4] = [5, 0, 0, 0];
const LAYERS: [&str; 2] = ["base", "function"];

#[derive(Clone, Serialize, Deserialize)]
pub struct InputOwnership {
    before: RawSnapshot,
    slots: Vec<u8>,
}

impl InputOwnership {
    pub fn prepare(before: &RawSnapshot, profile: &AutomationProfile) -> Result<Option<Self>> {
        profile.validate()?;
        if profile.depth_choices.is_empty() {
            return Ok(None);
        }
        before.decode()?;
        if before.identity.firmware != "1.17"
            || before.identity.vendor_id != 0x0c45
            || before.identity.product_id != 0x80d6
        {
            return Err(AppError::new(
                "unsupportedCapture",
                "Этот режим проверяется только на IO White 1.17.",
            ));
        }
        let slots = profile
            .depth_choices
            .iter()
            .map(|r| r.slot)
            .collect::<Vec<_>>();
        if slots.iter().any(|s| ![105, 108].contains(s)) {
            return Err(AppError::new(
                "unsupportedCapture",
                "Выбор по глубине сейчас доступен для PgUp и PgDn.",
            ));
        }
        // Do not split coupled RS/SOCD/advanced assignments while taking ownership.
        for layer in LAYERS {
            for &slot in &slots {
                let page = before.blocks[layer][usize::from(slot) * 4];
                if !matches!(page, 0..=3 | 5) {
                    return Err(AppError::new(
                        "captureBindingConflict",
                        "Сначала уберите аппаратное расширенное назначение с выбранной клавиши и её Fn-слоя.",
                    ));
                }
            }
        }
        Ok(Some(Self {
            before: before.clone(),
            slots,
        }))
    }

    pub fn plan(&self) -> PreparedChange {
        let mut after = self.before.clone();
        for layer in LAYERS {
            for &slot in &self.slots {
                let offset = usize::from(slot) * 4;
                after.blocks.get_mut(layer).expect("validated block")[offset..offset + 4]
                    .copy_from_slice(&SILENT);
            }
        }
        let blocks = LAYERS
            .into_iter()
            .filter(|name| self.before.blocks[*name] != after.blocks[*name])
            .map(str::to_owned)
            .collect();
        PreparedChange {
            token: after.revision(),
            before: self.before.clone(),
            after,
            blocks,
        }
    }

    pub fn restore_plan(&self, current: &RawSnapshot) -> Result<PreparedChange> {
        current.decode()?;
        if current.identity != self.before.identity {
            return Err(AppError::new(
                "captureIdentity",
                "Подключена другая модель или версия клавиатуры.",
            ));
        }
        let mut after = current.clone();
        for layer in LAYERS {
            for &slot in &self.slots {
                let offset = usize::from(slot) * 4;
                let range = offset..offset + 4;
                let original = &self.before.blocks[layer][range.clone()];
                let actual = &current.blocks[layer][range.clone()];
                if actual != SILENT && actual != original {
                    return Err(AppError::new(
                        "captureRestoreConflict",
                        "Назначение изменено другим редактором. Автоматическое восстановление остановлено; резервный снимок сохранён.",
                    ));
                }
                after.blocks.get_mut(layer).expect("validated block")[range]
                    .copy_from_slice(original);
            }
        }
        let blocks = LAYERS
            .into_iter()
            .filter(|name| current.blocks[*name] != after.blocks[*name])
            .map(str::to_owned)
            .collect();
        Ok(PreparedChange {
            token: after.revision(),
            before: current.clone(),
            after,
            blocks,
        })
    }

    fn path(directory: &Path) -> PathBuf {
        directory.join("input-ownership.json")
    }
    pub fn persist(&self, directory: &Path) -> Result<()> {
        fs::create_dir_all(directory).map_err(storage_error)?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|_| {
            AppError::new("captureJournal", "Не удалось подготовить восстановление.")
        })?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(Self::path(directory))
            .map_err(storage_error)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(storage_error)
    }
    pub fn load(directory: &Path) -> Result<Option<Self>> {
        let path = Self::path(directory);
        if !path.exists() {
            return Ok(None);
        }
        if fs::metadata(&path).map_err(storage_error)?.len() > 2_000_000 {
            return Err(AppError::new(
                "captureJournal",
                "Файл восстановления слишком большой.",
            ));
        }
        let value: Self = serde_json::from_slice(&fs::read(path).map_err(storage_error)?)
            .map_err(|_| AppError::new("captureJournal", "Файл восстановления повреждён."))?;
        value.before.decode()?;
        if value.slots.is_empty()
            || value.slots.len() > 2
            || value.slots.iter().any(|s| ![105, 108].contains(s))
        {
            return Err(AppError::new(
                "captureJournal",
                "Неверные адреса восстановления.",
            ));
        }
        Ok(Some(value))
    }
    pub fn clear(directory: &Path) -> Result<()> {
        fs::remove_file(Self::path(directory)).map_err(storage_error)
    }
}
fn storage_error(_: std::io::Error) -> AppError {
    AppError::new(
        "captureJournal",
        "Не удалось сохранить или завершить восстановление назначений. Проверьте файл input-ownership.json в каталоге recovery.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn journal_is_durable_unique_and_can_recover_without_live_owner() {
        let directory = std::env::temp_dir().join(format!(
            "io-ownership-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let owner = InputOwnership {
            before: crate::device::fixture(),
            slots: vec![105, 108],
        };
        assert!(InputOwnership::load(&directory).unwrap().is_none());
        owner.persist(&directory).unwrap();
        assert!(owner.persist(&directory).is_err()); // never overwrite an unfinished session
        let saved = InputOwnership::load(&directory).unwrap().unwrap();
        let restored = saved.restore_plan(&owner.plan().after).unwrap();
        assert_eq!(restored.after.revision(), owner.before.revision());
        InputOwnership::clear(&directory).unwrap();
        assert!(InputOwnership::load(&directory).unwrap().is_none());
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn restoration_only_touches_owned_keys_and_rejects_competing_edits() {
        let before = crate::device::fixture();
        let owner = InputOwnership {
            before: before.clone(),
            slots: vec![105, 108],
        };
        let plan = owner.plan();
        for layer in LAYERS {
            for offset in 0..512 {
                if !(420..424).contains(&offset) && !(432..436).contains(&offset) {
                    assert_eq!(
                        plan.after.blocks[layer][offset],
                        before.blocks[layer][offset]
                    );
                }
            }
        }
        let mut current = plan.after;
        current.blocks.get_mut("base").unwrap()[16] = 2;
        let restored = owner.restore_plan(&current).unwrap().after;
        assert_eq!(restored.blocks["base"][16], 2);
        assert_eq!(
            &restored.blocks["base"][420..424],
            &before.blocks["base"][420..424]
        );
        current.blocks.get_mut("base").unwrap()[420] = 3;
        assert!(owner.restore_plan(&current).is_err());
        assert!(owner.restore_plan(&before).unwrap().blocks.is_empty());
    }
}
