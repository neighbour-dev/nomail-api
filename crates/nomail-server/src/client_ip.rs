//! Best-effort device metadata reported by proxies, not a verified identity.
use actix_web::HttpRequest;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub(crate) fn resolve(request: &HttpRequest) -> IpAddr {
    for name in [
        "CF-Connecting-IP",
        "X-Forwarded-For",
        "X-Real-IP",
        "Forwarded",
    ] {
        for header in request.headers().get_all(name) {
            let Ok(value) = header.to_str() else {
                continue;
            };
            for entry in value.split(',') {
                let ip = if name == "Forwarded" {
                    entry.split(';').find_map(|parameter| {
                        let (key, value) = parameter.trim().split_once('=')?;
                        if key.trim().eq_ignore_ascii_case("for") {
                            parse_address(value)
                        } else {
                            None
                        }
                    })
                } else {
                    parse_address(entry)
                };
                if let Some(ip) = ip {
                    return ip;
                }
            }
        }
    }
    request
        .peer_addr()
        .map(|address| address.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
}

fn parse_address(value: &str) -> Option<IpAddr> {
    let value = value.trim();
    let value = if value.starts_with('"') {
        value.strip_prefix('"')?.strip_suffix('"')?
    } else {
        value
    };
    value
        .parse::<IpAddr>()
        .ok()
        .or_else(|| value.parse::<SocketAddr>().ok().map(|address| address.ip()))
        .or_else(|| value.strip_prefix('[')?.strip_suffix(']')?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    #[test]
    fn cloudflare_header_has_priority_including_public_connections() {
        let request = TestRequest::get()
            .peer_addr("8.8.8.8:443".parse().unwrap())
            .insert_header(("cf-connecting-ip", "100.101.102.103"))
            .insert_header(("X-Forwarded-For", "192.168.1.20, 9.9.9.9"))
            .insert_header(("X-Real-IP", "9.9.9.9"))
            .insert_header(("Forwarded", "for=1.1.1.1"))
            .to_http_request();
        assert_eq!(
            resolve(&request),
            "100.101.102.103".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn header_precedence_and_left_to_right_scan_skip_invalid_entries() {
        for (headers, expected) in [
            (
                vec![
                    ("CF-Connecting-IP", "unknown"),
                    ("X-Forwarded-For", "192.168.1.20, 9.9.9.9"),
                    ("X-Real-IP", "9.9.9.9"),
                ],
                "192.168.1.20",
            ),
            (
                vec![("X-Forwarded-For", "unknown, , 100.101.102.103, 10.0.0.2")],
                "100.101.102.103",
            ),
            (
                vec![
                    ("X-Forwarded-For", "unknown"),
                    ("X-Forwarded-For", "192.168.1.20"),
                    ("X-Forwarded-For", "10.0.0.2"),
                ],
                "192.168.1.20",
            ),
            (
                vec![
                    ("X-Forwarded-For", ""),
                    ("X-Real-IP", "fd7a:115c:a1e0::1"),
                    ("Forwarded", "for=1.1.1.1"),
                ],
                "fd7a:115c:a1e0::1",
            ),
            (
                vec![
                    ("X-Real-IP", "unknown"),
                    (
                        "Forwarded",
                        "for=unknown;proto=https, For=\"[fd7a:115c:a1e0::1]:443\"",
                    ),
                ],
                "fd7a:115c:a1e0::1",
            ),
            (
                vec![("Forwarded", "for=192.168.1.20;proto=https, for=10.0.0.2")],
                "192.168.1.20",
            ),
        ] {
            let mut request = TestRequest::get().peer_addr("172.18.0.1:3000".parse().unwrap());
            for (name, value) in headers {
                request = request.append_header((name, value));
            }
            assert_eq!(
                resolve(&request.to_http_request()),
                expected.parse::<IpAddr>().unwrap()
            );
        }
    }

    #[test]
    fn falls_back_to_peer_then_unspecified_when_headers_are_unusable() {
        for headers in [
            vec![],
            vec![
                ("CF-Connecting-IP", "unknown"),
                ("X-Forwarded-For", " , invalid"),
                ("X-Real-IP", ""),
                ("Forwarded", "for=_hidden"),
            ],
        ] {
            let mut request = TestRequest::get().peer_addr("172.18.0.1:3000".parse().unwrap());
            for (name, value) in headers {
                request = request.insert_header((name, value));
            }
            assert_eq!(
                resolve(&request.to_http_request()),
                "172.18.0.1".parse::<IpAddr>().unwrap()
            );
        }
        assert_eq!(
            resolve(&TestRequest::get().to_http_request()),
            "0.0.0.0".parse::<IpAddr>().unwrap()
        );
        let request = TestRequest::get()
            .insert_header(("CF-Connecting-IP", "192.168.1.20"))
            .to_http_request();
        assert_eq!(resolve(&request), "192.168.1.20".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn parses_ip_addresses_without_filtering_private_networks() {
        for (value, expected) in [
            ("192.168.1.20", "192.168.1.20"),
            (" 100.101.102.103 ", "100.101.102.103"),
            ("127.0.0.1:8080", "127.0.0.1"),
            ("fd7a:115c:a1e0::1", "fd7a:115c:a1e0::1"),
            ("\"[fd7a:115c:a1e0::1]:443\"", "fd7a:115c:a1e0::1"),
            ("[::1]", "::1"),
        ] {
            assert_eq!(parse_address(value), Some(expected.parse().unwrap()));
        }
        for value in [
            "",
            "unknown",
            "_hidden",
            "example.com",
            "1.2.3.999",
            "\"::1",
            "[::1]:bad",
        ] {
            assert_eq!(parse_address(value), None);
        }
    }
}
