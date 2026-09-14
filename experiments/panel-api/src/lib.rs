//! Offline protocol/lease prototype. No HID, memory addresses, allocator or firmware writer.
#![no_std]

pub type Packet = [u8; 64];
pub type Frame = [[u8; 3]; 10];
pub const MIN_TTL_MS: u16 = 100;
pub const MAX_TTL_MS: u16 = 2000;
pub const CAPS: u8 = 1;
pub const FRAME: u8 = 2;
pub const RELEASE: u8 = 3;
pub const ABI_ID: u32 = 0x0001_1084;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Ok = 0,
    InvalidPacket = 1,
    InvalidValue = 2,
    Busy = 3,
    Stale = 4,
    Closed = 5,
    NotOwner = 6,
    Unsupported = 7,
}

fn read32(packet: &Packet, at: usize) -> u32 {
    u32::from_le_bytes([packet[at], packet[at + 1], packet[at + 2], packet[at + 3]])
}

fn header(tx: u16, op: u8, length: u8, response: bool) -> Packet {
    let mut packet = [0; 64];
    packet[0] = if response { 0x55 } else { 0xAA };
    packet[1] = 0x33;
    packet[2] = length;
    packet[3..5].copy_from_slice(&tx.to_le_bytes());
    packet[5] = if response { op | 0x80 } else { op };
    packet[6] = 1;
    packet[7] = 1;
    packet[8..12].copy_from_slice(if response { b"IOLR" } else { b"IOLQ" });
    packet
}

fn valid_shape(packet: &Packet, length: u8) -> bool {
    packet[2] == length
        && packet[6] == 1
        && packet[7] == 1
        && packet[8 + usize::from(length)..].iter().all(|&b| b == 0)
}

pub fn discover(tx: u16) -> Packet {
    header(tx, CAPS, 4, false)
}

pub fn frame_packet(
    tx: u16,
    session: u32,
    sequence: u32,
    ttl: u16,
    frame: &Frame,
) -> Result<Packet, Status> {
    if session == 0 || sequence == 0 || !(MIN_TTL_MS..=MAX_TTL_MS).contains(&ttl) {
        return Err(Status::InvalidValue);
    }
    let mut packet = header(tx, FRAME, 44, false);
    packet[12..16].copy_from_slice(&session.to_le_bytes());
    packet[16..20].copy_from_slice(&sequence.to_le_bytes());
    packet[20..22].copy_from_slice(&ttl.to_le_bytes());
    for (i, color) in frame.iter().enumerate() {
        packet[22 + i * 3..25 + i * 3].copy_from_slice(color);
    }
    Ok(packet)
}

pub fn release_packet(tx: u16, session: u32, sequence: u32) -> Result<Packet, Status> {
    if session == 0 || sequence == 0 {
        return Err(Status::InvalidValue);
    }
    let mut packet = header(tx, RELEASE, 12, false);
    packet[12..16].copy_from_slice(&session.to_le_bytes());
    packet[16..20].copy_from_slice(&sequence.to_le_bytes());
    Ok(packet)
}

/// Reject stock ACK/echo, stale transactions, malformed replies and unknown statuses.
pub fn validate_reply(request: &Packet, reply: &Packet) -> Result<(), Status> {
    if reply[0] != 0x55
        || reply[1] != 0x33
        || reply[3..5] != request[3..5]
        || reply[5] != request[5] | 0x80
        || &reply[8..12] != b"IOLR"
    {
        return Err(Status::InvalidPacket);
    }
    let length = if request[5] == CAPS && reply[12] == 0 {
        16
    } else {
        5
    };
    if !valid_shape(reply, length) {
        return Err(Status::InvalidPacket);
    }
    match reply[12] {
        0 => Ok(()),
        1 => Err(Status::InvalidPacket),
        2 => Err(Status::InvalidValue),
        3 => Err(Status::Busy),
        4 => Err(Status::Stale),
        5 => Err(Status::Closed),
        6 => Err(Status::NotOwner),
        7 => Err(Status::Unsupported),
        _ => Err(Status::InvalidPacket),
    }
}

pub fn supports_panel(request: &Packet, reply: &Packet) -> bool {
    request[5] == CAPS
        && validate_reply(request, reply).is_ok()
        && reply[13] == 10
        && u16::from_le_bytes([reply[14], reply[15]]) == MAX_TTL_MS
        && u16::from_le_bytes([reply[16], reply[17]]) == MIN_TTL_MS
        && reply[18] == 3
        && reply[19] == 3
        && read32(reply, 20) == ABI_ID
}

/// State has no allocation or IO. Integration must call disconnect() on USB lifecycle changes.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Panel {
    owner: u32,
    sequence: u32,
    received_at: u32,
    ttl: u16,
    closed: bool,
    frame: Frame,
}

impl Panel {
    fn expire(&mut self, now: u32) {
        if self.ttl != 0 && now.wrapping_sub(self.received_at) >= u32::from(self.ttl) {
            self.ttl = 0;
        }
    }

    pub fn disconnect(&mut self) {
        self.ttl = 0;
        self.closed = true;
    }

    /// None means no overlay: render the live stock frame, not a saved configuration snapshot.
    pub fn current_frame(&mut self, now: u32) -> Option<Frame> {
        self.expire(now);
        (self.ttl != 0 && !self.closed).then_some(self.frame)
    }

    /// Apply the stock panel's brightness scaling at a full envelope of 255.
    /// `permitted` must exclude calibration/diagnostics and USB suspend/disconnect.
    pub fn output_frame(&mut self, now: u32, brightness: u8, permitted: bool) -> Option<Frame> {
        if !permitted {
            self.disconnect();
            return None;
        }
        let mut frame = self.current_frame(now)?;
        for color in &mut frame {
            for channel in color {
                *channel = ((u32::from(*channel) * 255 * u32::from(brightness)) >> 16) as u8;
            }
        }
        Some(frame)
    }

    /// None routes a legacy packet to the original dispatcher without interpreting it here.
    pub fn handle(&mut self, packet: &Packet, now: u32) -> Option<Packet> {
        if packet[0] != 0xAA || packet[1] != 0x33 || &packet[8..12] != b"IOLQ" {
            return None;
        }
        let op = packet[5];
        let status = self.apply(packet, now);
        let caps_ok = op == CAPS && status == Status::Ok;
        let mut reply = header(
            u16::from_le_bytes([packet[3], packet[4]]),
            op,
            if caps_ok { 16 } else { 5 },
            true,
        );
        reply[12] = status as u8;
        if caps_ok {
            reply[13] = 10;
            reply[14..16].copy_from_slice(&MAX_TTL_MS.to_le_bytes());
            reply[16..18].copy_from_slice(&MIN_TTL_MS.to_le_bytes());
            reply[18] = 3;
            reply[19] = 3; // Volatile frames + explicit release; no persistence capability.
            reply[20..24].copy_from_slice(&ABI_ID.to_le_bytes());
        }
        Some(reply)
    }

    fn apply(&mut self, packet: &Packet, now: u32) -> Status {
        let length = match packet[5] {
            CAPS => 4,
            FRAME => 44,
            RELEASE => 12,
            _ => return Status::Unsupported,
        };
        if !valid_shape(packet, length) {
            return Status::InvalidPacket;
        }
        if packet[5] == CAPS {
            return Status::Ok;
        }
        let owner = read32(packet, 12);
        let sequence = read32(packet, 16);
        let ttl = u16::from_le_bytes([packet[20], packet[21]]);
        if owner == 0
            || sequence == 0
            || (packet[5] == FRAME && !(MIN_TTL_MS..=MAX_TTL_MS).contains(&ttl))
        {
            return Status::InvalidValue;
        }
        self.expire(now);
        if packet[5] == RELEASE {
            if owner != self.owner {
                return Status::NotOwner;
            }
            if sequence < self.sequence || (sequence == self.sequence && !self.closed) {
                return Status::Stale;
            }
            self.sequence = sequence;
            self.disconnect();
            return Status::Ok;
        }
        if owner == self.owner {
            if self.closed {
                return Status::Closed;
            }
            if sequence <= self.sequence {
                return Status::Stale;
            }
        } else if self.ttl != 0 {
            return Status::Busy;
        }
        // All validation precedes the single state update. No partial pixel update is exposed.
        let mut frame = [[0; 3]; 10];
        for (i, color) in frame.iter_mut().enumerate() {
            color.copy_from_slice(&packet[22 + i * 3..25 + i * 3]);
        }
        self.frame = frame;
        self.owner = owner;
        self.sequence = sequence;
        self.received_at = now;
        self.ttl = ttl;
        self.closed = false;
        Status::Ok
    }
}
