pub fn pagination(page: Option<&str>, size: Option<&str>) -> (u64, u64) {
    let page = page
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1) as u64;
    let size = size
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(20)
        .min(100) as u64;
    (page, size)
}

pub fn summarize(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(160)
        .collect()
}
