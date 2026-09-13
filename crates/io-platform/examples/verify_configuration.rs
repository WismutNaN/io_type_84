//! Явный ограниченный эксперимент: запись, GET сверка, восстановление.
//! Макрос/DKS записываются без назначения, никакие действия не исполняются.
use io_core::keyboard::*;
use io_platform::{
    changes::{self, PreparedChange, RecoveryRecord},
    device::NativeDevice,
};

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() != Some("--run") {
        eprintln!("Для обратимого аппаратного теста: --run. Закройте другие редакторы.");
        return Ok(());
    }
    let directory = std::path::Path::new("archive_data/configuration_recovery");
    let mut device = NativeDevice::open()?;
    let before = device.snapshot()?;
    let dto = before.decode()?;
    if !dto.macros.is_empty() {
        return Err("Этот эксперимент рассчитан на пустой каталог макросов.".into());
    }
    let dks_index = dto
        .dks
        .iter()
        .find(|d| {
            d.thresholds == [0; 4]
                && d.states == [0; 4]
                && !dto.keys.iter().any(|k| {
                    [&k.base, &k.function]
                        .iter()
                        .any(|b| b.page == 8 && b.parameters[0] == d.index)
                })
        })
        .ok_or("Нет свободной DKS записи")?
        .index;
    let mut rt = dto.keys[12].actuation.clone();
    rt.trigger_um = 1200;
    rt.press_um = 100;
    rt.release_um = 200;
    rt.rapid_trigger = true;
    let plan = changes::prepare(
        &before,
        ChangeRequest {
            base_revision: before.revision(),
            edits: vec![
                Edit::Binding {
                    slots: vec![12],
                    function_layer: false,
                    binding: BindingRecord {
                        page: 2,
                        parameters: [0, 68, 0],
                    },
                },
                Edit::Binding {
                    slots: vec![12],
                    function_layer: true,
                    binding: BindingRecord {
                        page: 2,
                        parameters: [0, 67, 0],
                    },
                },
                Edit::Actuation {
                    slots: vec![12],
                    value: rt,
                },
                Edit::Color {
                    slots: vec![12],
                    color: Rgb {
                        r: 46,
                        g: 80,
                        b: 130,
                    },
                },
                Edit::Macros {
                    values: vec![HardwareMacro {
                        id: 0,
                        steps: vec![
                            MacroStep {
                                key_code: 4,
                                kind: 1,
                                pressed: true,
                                delay_ms: 10,
                            },
                            MacroStep {
                                key_code: 4,
                                kind: 1,
                                pressed: false,
                                delay_ms: 10,
                            },
                        ],
                    }],
                },
                Edit::Dks {
                    value: DksConfiguration {
                        index: dks_index,
                        thresholds: [16, 30, 30, 16],
                        actions: [4, 0, 0, 0],
                        states: [1, 0, 0, 0],
                    },
                },
            ],
        },
    )?;
    println!("{}", serde_json::to_string(&plan.preview())?);
    let result = changes::apply(&mut device, &plan, directory);
    println!(
        "{}",
        serde_json::json!({"applySucceeded":result.is_ok(),"error":result.as_ref().err()})
    );
    drop(device);
    let mut device = NativeDevice::open()?;
    let current = device.snapshot()?;
    let mut files = std::fs::read_dir(directory)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect::<Vec<_>>();
    files.sort();
    let record: RecoveryRecord = serde_json::from_slice(&std::fs::read(
        files.last().ok_or("Нет резервного снимка")?,
    )?)?;
    if record.target.revision() != plan.after.revision() {
        return Err("Резервный снимок относится к другой операции".into());
    }
    let mut target = current.clone();
    for name in &plan.blocks {
        target
            .blocks
            .insert(name.clone(), record.before.blocks[name].clone());
    }
    let restore = PreparedChange {
        token: target.revision(),
        before: current,
        after: target,
        blocks: plan.blocks.clone(),
    };
    changes::apply(&mut device, &restore, directory)?;
    let after = device.snapshot()?;
    if before.revision() != after.revision() {
        return Err("После восстановления снимок отличается".into());
    }
    println!(
        "{}",
        serde_json::json!({"restored":true,"beforeRevision":before.revision(),"afterRevision":after.revision(),"blocks":plan.blocks})
    );
    result?;
    Ok(())
}
