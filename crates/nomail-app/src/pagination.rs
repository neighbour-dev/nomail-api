/// Application pagination result, independent of HTTP envelopes and routes.
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}
impl<T> Page<T> {
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            page: self.page,
            page_size: self.page_size,
        }
    }
}

/// Check numeric query bounds and compute a database-safe offset.
/// String parsing, defaults and clamping remain at the HTTP boundary.
pub(crate) fn checked_offset(page: u64, page_size: u64) -> Result<u64, &'static str> {
    if page == 0 || !(1..=100).contains(&page_size) {
        return Err("invalid pagination");
    }
    (page - 1)
        .checked_mul(page_size)
        .filter(|offset| *offset <= i64::MAX as u64)
        .ok_or("page is too large")
}

pub fn summarize(text: &str) -> String {
    let summary = nomail_core::webhook::summarize(text);
    if summary.is_empty() {
        "无法解析消息内容".into()
    } else {
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::checked_offset;

    #[test]
    fn offsets_preserve_bounds_exact_integers_and_error_precedence() {
        let max = i64::MAX as u64;
        for (page, size, offset) in [
            (1, 20, 0),
            (3, 20, 40),
            (2, 100, 100),
            (9_007_199_254_740_994, 1, 9_007_199_254_740_993),
            (max + 1, 1, max),
        ] {
            assert_eq!(checked_offset(page, size), Ok(offset));
        }
        for (page, size) in [(0, 20), (1, 0), (1, 101), (u64::MAX, 101)] {
            assert_eq!(checked_offset(page, size), Err("invalid pagination"));
        }
        for (page, size) in [(max + 2, 1), (max, 100), (u64::MAX, 100)] {
            assert_eq!(checked_offset(page, size), Err("page is too large"));
        }
    }
}
