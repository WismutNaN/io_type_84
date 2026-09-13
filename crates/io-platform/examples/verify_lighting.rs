//! Явный аппаратный эксперимент: яркость на один шаг, readback, восстановление.
use io_core::keyboard::{ChangeRequest, Edit};
use io_platform::{
    changes::{self, PreparedChange},
    device::NativeDevice,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let restore_only = std::env::args().nth(1).as_deref() == Some("--restore");
    if std::env::args().nth(1).as_deref() != Some("--run") && !restore_only {
        eprintln!(
            "Запуск: cargo run -p io-platform --example verify_lighting -- --run. Временно меняет яркость и восстанавливает снимок."
        );
        return Ok(());
    }
    let directory = std::path::Path::new("archive_data/recovery");
    let mut device = NativeDevice::open()?;
    let before = if restore_only {
        let mut files = std::fs::read_dir(directory)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect::<Vec<_>>();
        files.sort();
        let record: changes::RecoveryRecord =
            serde_json::from_slice(&std::fs::read(files.last().ok_or("Нет снимков")?)?)?;
        if record.before.identity != device.identity {
            return Err("Другая identity".into());
        }
        record.before
    } else {
        device.snapshot()?
    };
    let mut light = before.decode()?.lighting;
    light.brightness = if light.brightness == 5 {
        4
    } else {
        light.brightness + 1
    };
    let plan = changes::prepare(
        &before,
        ChangeRequest {
            base_revision: before.revision(),
            edits: vec![Edit::Lighting { value: light }],
        },
    )?;
    println!("{}", serde_json::to_string(&plan.preview())?);
    if !restore_only {
        changes::apply(&mut device, &plan, directory)?;
        println!("Яркость изменена и прочитана обратно.");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    drop(device);
    let mut device = NativeDevice::open()?;
    let current = device.snapshot()?;
    let mut target = current.clone();
    target
        .blocks
        .insert("lighting".into(), before.blocks["lighting"].clone());
    let restore = PreparedChange {
        token: target.revision(),
        before: current,
        after: target,
        blocks: vec!["lighting".into()],
    };
    changes::apply(&mut device, &restore, directory)?;
    let after = device.snapshot()?;
    if before.revision() != after.revision() {
        return Err("Конфигурация после восстановления отличается".into());
    }
    println!(
        "{}",
        serde_json::json!({"restored":true,"beforeRevision":before.revision(),"afterRevision":after.revision(),"changedBlock":"lighting","backupDirectory":"archive_data/recovery"})
    );
    Ok(())
}
