//! Actix header access only. Protocol and product policies live in the shared application.
use actix_web::http::header::HeaderMap;
use nomail_app::webhook::HeaderLookup;

pub(super) struct Headers<'a>(pub &'a HeaderMap);
impl HeaderLookup for Headers<'_> {
    fn values(&self, name: &str) -> Vec<&str> {
        self.0
            .get_all(name)
            .filter_map(|value| value.to_str().ok())
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header::{HeaderName, HeaderValue};
    #[test]
    fn header_adapter_preserves_repeated_values_and_ignores_case() {
        let mut headers = HeaderMap::new();
        let name = HeaderName::from_static("x-repeated");
        headers.append(name.clone(), HeaderValue::from_static("one"));
        headers.append(name, HeaderValue::from_static("two"));
        assert_eq!(Headers(&headers).values("X-Repeated"), ["one", "two"]);
    }
}
