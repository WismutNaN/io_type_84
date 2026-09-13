//! IO White 1.17: строгий codec подтверждённых пакетов AA/55.

use io_core::keyboard::{AppError, Result};
pub const FRAME_SIZE: usize = 64;
pub const PAYLOAD_SIZE: usize = 56;

pub fn frame(
    command: u8,
    address: u16,
    size: usize,
    payload: &[u8],
    last: bool,
) -> Result<[u8; 64]> {
    if size > PAYLOAD_SIZE || payload.len() > size {
        return Err(AppError::new(
            "invalidPacket",
            "Данные не помещаются в HID-пакет",
        ));
    }
    let mut out = [0; FRAME_SIZE];
    out[0] = 0xaa;
    out[1] = command;
    out[2] = size as u8;
    out[3..5].copy_from_slice(&address.to_le_bytes());
    out[6] = u8::from(last);
    out[8..8 + payload.len()].copy_from_slice(payload);
    Ok(out)
}

pub fn response<'a>(sent: &[u8; 64], received: &'a [u8]) -> Result<&'a [u8]> {
    if received.len() != FRAME_SIZE || received[0] != 0x55 || received[1..5] != sent[1..5] {
        return Err(AppError::new(
            "protocolMismatch",
            "Получен ответ другой длины, команды или адреса. Переподключите устройство.",
        ));
    }
    Ok(&received[8..8 + usize::from(sent[2])])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|s| u8::from_str_radix(s, 16).unwrap())
            .collect()
    }
    #[test]
    fn all_eighty_hardware_packets_round_trip() {
        let capture: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/evidence/read-snapshot.json"))
                .unwrap();
        for pair in capture["packets"].as_array().unwrap() {
            let request = bytes(pair["request_hex"].as_str().unwrap());
            let reply = bytes(pair["response_hex"].as_str().unwrap());
            let encoded = frame(
                request[1],
                u16::from_le_bytes([request[3], request[4]]),
                request[2].into(),
                &request[8..8 + usize::from(request[2])],
                request[6] == 1,
            )
            .unwrap();
            assert_eq!(encoded.as_slice(), request);
            assert_eq!(
                response(&encoded, &reply).unwrap().len(),
                usize::from(request[2])
            );
        }
    }
    #[test]
    fn refuses_truncation_and_mismatched_replies() {
        assert!(frame(0x12, 0, 57, &[], true).is_err());
        assert!(frame(0x12, 0, 1, &[0; 2], true).is_err());
        let sent = frame(0x12, 56, 56, &[], false).unwrap();
        for len in [0, 1, 8, 63, 65] {
            assert!(response(&sent, &vec![0; len]).is_err());
        }
        for byte in 0..5 {
            let mut reply = sent;
            reply[0] = 0x55;
            reply[byte] ^= 1;
            assert!(response(&sent, &reply).is_err());
        }
    }
}
