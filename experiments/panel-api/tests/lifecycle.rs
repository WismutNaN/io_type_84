use io_panel_api_prototype::*;

const BLUE: Frame = [[0, 90, 255]; 10];

fn send(panel: &mut Panel, packet: Packet, now: u32) -> Result<(), Status> {
    validate_reply(&packet, &panel.handle(&packet, now).unwrap())
}

#[test]
fn stock_echo_cannot_enable_the_extension() {
    let query = discover(7);
    let mut stock_echo = query;
    stock_echo[0] = 0x55;
    assert!(!supports_panel(&query, &stock_echo));
    let mut panel = Panel::default();
    let reply = panel.handle(&query, 0).unwrap();
    assert!(supports_panel(&query, &reply));
    assert!(panel.current_frame(0).is_none());
    let mut stale = reply;
    stale[3] ^= 1;
    assert!(!supports_panel(&query, &stale));
    let mut foreign = reply;
    foreign[20] ^= 1;
    assert!(!supports_panel(&query, &foreign));
}

#[test]
fn incomplete_or_malformed_frames_do_not_replace_pixels() {
    let mut panel = Panel::default();
    let valid = frame_packet(1, 10, 1, 500, &BLUE).unwrap();
    assert_eq!(send(&mut panel, valid, 0), Ok(()));
    for at in [2, 6, 7, 52, 63] {
        let mut malformed = frame_packet(2, 10, 2, 500, &[[255; 3]; 10]).unwrap();
        malformed[at] ^= 0x40;
        assert_eq!(send(&mut panel, malformed, 1), Err(Status::InvalidPacket));
        assert_eq!(panel.current_frame(1), Some(BLUE));
    }
    for ttl in [0u16, 99, 2001, u16::MAX] {
        let mut malformed = valid;
        malformed[20..22].copy_from_slice(&ttl.to_le_bytes());
        assert_eq!(send(&mut panel, malformed, 1), Err(Status::InvalidValue));
    }
}

#[test]
fn stale_packet_does_not_keep_lights_alive() {
    let mut panel = Panel::default();
    let packet = frame_packet(1, 1, 1, 100, &BLUE).unwrap();
    send(&mut panel, packet, 20).unwrap();
    assert_eq!(send(&mut panel, packet, 119), Err(Status::Stale));
    assert_eq!(panel.current_frame(119), Some(BLUE));
    assert_eq!(panel.current_frame(120), None);
}

#[test]
fn release_is_retryable_and_closes_the_session() {
    let mut panel = Panel::default();
    send(&mut panel, frame_packet(1, 1, 1, 500, &BLUE).unwrap(), 0).unwrap();
    let release = release_packet(2, 1, 2).unwrap();
    send(&mut panel, release, 20).unwrap();
    send(&mut panel, release, 21).unwrap();
    assert_eq!(panel.current_frame(21), None);
    assert_eq!(
        send(&mut panel, frame_packet(3, 1, 3, 500, &BLUE).unwrap(), 22),
        Err(Status::Closed)
    );
    send(&mut panel, frame_packet(4, 2, 1, 500, &BLUE).unwrap(), 23).unwrap();
}

#[test]
fn another_owner_cannot_interrupt_an_active_lease() {
    let mut panel = Panel::default();
    send(&mut panel, frame_packet(1, 1, 1, 100, &BLUE).unwrap(), 0).unwrap();
    assert_eq!(
        send(&mut panel, frame_packet(2, 2, 1, 100, &BLUE).unwrap(), 99),
        Err(Status::Busy)
    );
    assert_eq!(
        send(&mut panel, release_packet(3, 2, 2).unwrap(), 99),
        Err(Status::NotOwner)
    );
    send(&mut panel, frame_packet(4, 2, 1, 100, &BLUE).unwrap(), 100).unwrap();
}

#[test]
fn rollover_and_disconnected_or_diagnostic_state_fall_back_to_stock() {
    let mut panel = Panel::default();
    send(
        &mut panel,
        frame_packet(1, 1, 1, 100, &BLUE).unwrap(),
        u32::MAX - 50,
    )
    .unwrap();
    assert_eq!(panel.current_frame(48), Some(BLUE));
    assert_eq!(panel.current_frame(49), None);
    send(&mut panel, frame_packet(2, 2, 1, 100, &BLUE).unwrap(), 50).unwrap();
    assert!(panel.output_frame(51, 255, false).is_none());
    assert!(panel.output_frame(52, 255, true).is_none());
}

#[test]
fn brightness_scales_without_mutating_logical_colors_or_stock_state() {
    let mut panel = Panel::default();
    let white = [[255; 3]; 10];
    send(&mut panel, frame_packet(1, 1, 1, 100, &white).unwrap(), 0).unwrap();
    assert_eq!(panel.output_frame(1, 0, true), Some([[0; 3]; 10]));
    assert_eq!(panel.output_frame(2, 64, true), Some([[63; 3]; 10]));
    assert_eq!(panel.output_frame(3, 255, true), Some([[253; 3]; 10]));
    assert_eq!(panel.current_frame(4), Some(white));
}

#[test]
fn legacy_and_isp_packets_are_outside_the_api() {
    let mut panel = Panel::default();
    for command in [0x10, 0x23, 0x32, 0x65, 0x80] {
        let mut packet = discover(1);
        packet[1] = command;
        assert!(panel.handle(&packet, 0).is_none());
    }
    let mut legacy_colors = discover(1);
    legacy_colors[8..12].fill(0);
    assert!(panel.handle(&legacy_colors, 0).is_none());
    let mut magic = [0; 64];
    magic[..8].copy_from_slice(&[0xAA, 0x55, 0xA5, 0x5A, 0xFF, 0, 0x33, 0xCC]);
    assert!(panel.handle(&magic, 0).is_none());
}
