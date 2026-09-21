pub const MAX_TRACKER_LENGTH: usize = 6144;

pub fn parse_lines(body: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for raw in body.split('\n') {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if seen.insert(line.to_string()) {
            out.push(line.to_string());
        }
    }
    out
}

pub fn parse_trackers(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for raw in text.split(['\n', ',']) {
        let item = raw.trim();
        if item.is_empty() || item.starts_with('#') {
            continue;
        }
        if seen.insert(item.to_string()) {
            out.push(item.to_string());
        }
    }
    out
}

pub fn to_comma(text: &str) -> String {
    parse_trackers(text).join(",")
}

pub fn to_lines(text: &str) -> String {
    parse_trackers(text).join("\n")
}

pub fn count(text: &str) -> usize {
    parse_trackers(text).len()
}

pub fn reduce(value: String) -> String {
    if value.len() <= MAX_TRACKER_LENGTH {
        return value;
    }
    let mut end = MAX_TRACKER_LENGTH;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    let sub = &value[..end];
    match sub.rfind(',') {
        Some(index) => sub[..index].to_string(),
        None => sub.to_string(),
    }
}

pub fn sync_due(
    auto_sync: bool,
    interval_hours: u32,
    last_sync: Option<i64>,
    startup: bool,
    now_ms: i64,
) -> bool {
    if !auto_sync {
        return false;
    }
    if interval_hours == 0 {
        return startup;
    }
    let last = last_sync.unwrap_or(0);
    if last <= 0 {
        return true;
    }
    now_ms - last >= interval_hours as i64 * 3600 * 1000
}

pub async fn fetch_sources(
    urls: &[String],
    proxy: Option<String>,
) -> (Vec<String>, Vec<(String, String)>) {
    if urls.is_empty() {
        return (vec![], vec![]);
    }
    let builder = match crate::config::apply_proxy(
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)),
        proxy.as_deref(),
    ) {
        Ok(builder) => builder,
        Err(e) => {
            let failures: Vec<(String, String)> = urls
                .iter()
                .map(|u| (u.clone(), format!("proxy: {e}")))
                .collect();
            return (vec![], failures);
        }
    };
    let client = match builder.build() {
        Ok(c) => c,
        Err(e) => {
            let failures: Vec<(String, String)> =
                urls.iter().map(|u| (u.clone(), e.to_string())).collect();
            return (vec![], failures);
        }
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let futs = urls.iter().map(|url| {
        let client = client.clone();
        let url = url.clone();
        async move {
            let request_url = format!("{}?t={}", url, now);
            match client.get(&request_url).send().await {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        return (
                            None,
                            Some((url, format!("HTTP {}", resp.status().as_u16()))),
                        );
                    }
                    match resp.text().await {
                        Ok(body) => (Some(body), None),
                        Err(e) => (None, Some((url, e.to_string()))),
                    }
                }
                Err(e) => (None, Some((url, e.to_string()))),
            }
        }
    });

    let mut data = Vec::new();
    let mut failures = Vec::new();
    for (body, failure) in futures::future::join_all(futs).await {
        if let Some(b) = body {
            data.push(b);
        }
        if let Some(f) = failure {
            failures.push(f);
        }
    }
    (data, failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lines_splits_trim_dedup() {
        let body = "udp://a:1/announce\r\nudp://b:2/announce\r\n\r\n  udp://a:1/announce  \n";
        let lines = parse_lines(body);
        assert_eq!(lines, vec!["udp://a:1/announce", "udp://b:2/announce"]);
    }

    #[test]
    fn parse_lines_skips_comments_and_empty() {
        let body = "# comment\n\nudp://a:1/announce\n";
        assert_eq!(parse_lines(body), vec!["udp://a:1/announce"]);
    }

    #[test]
    fn parse_trackers_comma_and_newline() {
        assert_eq!(
            parse_trackers("udp://a:1/announce,udp://b:2/announce\nudp://c:3/announce"),
            vec![
                "udp://a:1/announce",
                "udp://b:2/announce",
                "udp://c:3/announce"
            ]
        );
        assert_eq!(
            parse_trackers("udp://a:1/announce, udp://a:1/announce"),
            vec!["udp://a:1/announce"]
        );
    }

    #[test]
    fn round_trip_comma_lines() {
        let text = "udp://a:1/announce\nudp://b:2/announce";
        assert_eq!(to_comma(text), "udp://a:1/announce,udp://b:2/announce");
        assert_eq!(to_lines(&to_comma(text)), text);
    }

    #[test]
    fn count_dedups() {
        assert_eq!(count("a,b\nb,c"), 3);
    }

    #[test]
    fn reduce_truncates_to_comma() {
        let long = format!("{},\n{}", "a".repeat(MAX_TRACKER_LENGTH), "b");
        assert_eq!(reduce(long), "a".repeat(MAX_TRACKER_LENGTH));
    }

    #[test]
    fn reduce_short_unchanged() {
        assert_eq!(
            reduce("udp://a:1/announce".to_string()),
            "udp://a:1/announce"
        );
    }

    #[test]
    fn reduce_cut_at_comma_boundary() {
        let mut value = String::new();
        for i in 0..1000 {
            value.push_str(&format!("udp://tracker{i}.org:1337/announce,"));
        }
        let reduced = reduce(value);
        assert!(reduced.len() <= MAX_TRACKER_LENGTH);
        assert!(!reduced.ends_with(','));
    }

    #[test]
    fn sync_due_rules() {
        assert!(!sync_due(false, 24, Some(0), false, 1_000));
        assert!(sync_due(true, 0, Some(0), true, 1_000));
        assert!(!sync_due(true, 0, Some(0), false, 1_000));
        assert!(sync_due(true, 24, None, false, 1_000));
        assert!(sync_due(true, 24, Some(0), false, 1_000));
        assert!(!sync_due(true, 24, Some(1_000), false, 1_000));
        assert!(sync_due(true, 1, Some(1_000), false, 1_000 + 3_600_000));
    }
}
