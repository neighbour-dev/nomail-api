pub fn now_ms() -> i64 {
    worker::Date::now().as_millis() as i64
}
pub fn format(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .expect("runtime timestamp in range")
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
