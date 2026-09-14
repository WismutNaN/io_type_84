//! Read-only Core Audio observer, used only by the interactive hardware diagnostic.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use windows::Win32::{
    Media::Audio::{
        Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole,
        eMultimedia, eRender,
    },
    System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
        CoUninitialize,
    },
};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: constructed only after successful initialization on this same thread.
        unsafe {
            CoUninitialize();
        }
    }
}
pub struct AudioProbe {
    enumerator: IMMDeviceEnumerator,
    // Last field: COM objects are released before the apartment is uninitialized.
    _apartment: Apartment,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub endpoint: String,
    pub percent: f32,
    pub muted: bool,
}
impl AudioProbe {
    pub fn new() -> windows::core::Result<Self> {
        // SAFETY: this diagnostic owns its main thread's COM initialization.
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        let apartment = Apartment;
        // SAFETY: documented MMDeviceEnumerator COM class; no outer aggregation.
        let enumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        Ok(Self {
            enumerator,
            _apartment: apartment,
        })
    }
    pub fn read(&self) -> windows::core::Result<BTreeMap<String, Reading>> {
        let mut result = BTreeMap::new();
        for (label, role) in [("console", eConsole), ("multimedia", eMultimedia)] {
            // SAFETY: valid COM interfaces in the initialized apartment. Only getters are called.
            let reading = unsafe {
                let device = self.enumerator.GetDefaultAudioEndpoint(eRender, role)?;
                let raw_id = device.GetId()?;
                let id = raw_id.to_string();
                CoTaskMemFree(Some(raw_id.0.cast()));
                let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
                Reading {
                    endpoint: format!("{:x}", Sha256::digest(id?.as_bytes())),
                    percent: endpoint.GetMasterVolumeLevelScalar()? * 100.0,
                    muted: endpoint.GetMute()?.as_bool(),
                }
            };
            result.insert(label.to_string(), reading);
        }
        Ok(result)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeRange {
    start: Reading,
    end: Reading,
    min_percent: f32,
    max_percent: f32,
    samples: u32,
}
#[derive(Default, Serialize)]
pub struct VolumeTrace(BTreeMap<String, VolumeRange>);
impl VolumeTrace {
    pub fn observe(&mut self, readings: BTreeMap<String, Reading>) -> Result<(), &'static str> {
        for (role, reading) in readings {
            if let Some(range) = self.0.get_mut(&role) {
                if range.start.endpoint != reading.endpoint {
                    return Err(
                        "Звуковое устройство сменилось; повторите проверку с одним выходом.",
                    );
                }
                range.min_percent = range.min_percent.min(reading.percent);
                range.max_percent = range.max_percent.max(reading.percent);
                range.end = reading;
                range.samples += 1;
            } else {
                self.0.insert(
                    role,
                    VolumeRange {
                        start: reading.clone(),
                        end: reading.clone(),
                        min_percent: reading.percent,
                        max_percent: reading.percent,
                        samples: 1,
                    },
                );
            }
        }
        Ok(())
    }
}
