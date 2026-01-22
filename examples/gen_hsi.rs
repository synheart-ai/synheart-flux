//! Generate HSI output for validation testing

fn main() {
    let json = r#"{
        "session_id": "validation-test",
        "device_id": "device-456",
        "timezone": "America/New_York",
        "start_time": "2024-01-15T14:00:00Z",
        "end_time": "2024-01-15T14:30:00Z",
        "events": [
            { "timestamp": "2024-01-15T14:01:00Z", "event_type": "scroll", "scroll": { "velocity": 150.5, "direction": "down", "direction_reversal": false } },
            { "timestamp": "2024-01-15T14:02:00Z", "event_type": "tap", "tap": { "tap_duration_ms": 120, "long_press": false } },
            { "timestamp": "2024-01-15T14:05:00Z", "event_type": "app_switch", "app_switch": { "from_app_id": "com.a", "to_app_id": "com.b" } },
            { "timestamp": "2024-01-15T14:10:00Z", "event_type": "notification", "interruption": { "action": "ignored" } },
            { "timestamp": "2024-01-15T14:15:00Z", "event_type": "typing", "typing": { "typing_speed_cpm": 180.0, "cadence_stability": 0.85 } }
        ]
    }"#;

    match synheart_flux::behavior::behavior_to_hsi(json.to_string()) {
        Ok(hsi) => print!("{hsi}"),
        Err(e) => eprintln!("Error: {e:?}"),
    }
}
