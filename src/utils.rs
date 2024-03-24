pub fn f32_to_u8_pct(value: f32) -> u8 {
    f32::round(value * 100.0) as u8
}