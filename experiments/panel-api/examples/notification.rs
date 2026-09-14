use io_panel_api_prototype::*;

fn main() {
    let mut device = Panel::default();
    let query = discover(1);
    assert!(supports_panel(&query, &device.handle(&query, 0).unwrap()));
    println!("Offline API prototype: 10 panel pixels, no hardware connection.");
    let frame = [[0, 110, 255]; 10];
    let packet = frame_packet(2, 42, 1, 500, &frame).unwrap();
    validate_reply(&packet, &device.handle(&packet, 100).unwrap()).unwrap();
    println!(
        "Notification at 100 ms: {:?}",
        device.current_frame(100).unwrap()[0]
    );
    assert!(device.current_frame(599).is_some());
    assert!(device.current_frame(600).is_none());
    println!("No new frames: at 600 ms use the current stock effect.");
    println!(
        "State size: {} bytes; no heap allocations.",
        core::mem::size_of::<Panel>()
    );
}
