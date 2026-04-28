use chrono::{DateTime, FixedOffset, ParseError};
use log::{debug, warn};
use rss::Channel;

use crate::http::fetch;
use crate::model::{AppError, Platform, RssUpdateInfo};

pub const RSS_URL: &str =
    "https://forums.kleientertainment.com/rss/6-dont-starve-together-updates.xml/";

fn extract_platform(link: &str) -> Platform {
    if link.contains("/dst_xboxone/") {
        Platform::Xbox
    } else if link.contains("/dst_ps4/") {
        Platform::PlayStation
    } else if link.contains("/dst_switch/") {
        Platform::Switch
    } else if link.contains("/dst/") {
        Platform::PC
    } else {
        Platform::Unknown
    }
}

fn extract_version(link: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = link.split('/').collect();
    for part in parts {
        if let Some(pos) = part.find("-r") {
            let build_part = &part[..pos];
            let release_part = &part[pos + 2..];
            if !build_part.is_empty() && build_part.chars().all(|c| c.is_ascii_digit()) {
                let build_number: u32 = build_part.parse().ok()?;
                let release_id: u32 = release_part.parse().ok()?;
                return Some((build_number, release_id));
            }
        }
        if part.starts_with('r') && part.len() > 1 {
            let release_id: u32 = part[1..].parse().ok()?;
            return Some((0, release_id));
        }
    }
    None
}

fn parse_pub_date(date_str: &str) -> Result<DateTime<FixedOffset>, ParseError> {
    DateTime::parse_from_rfc2822(date_str)
}

pub fn fetch_rss(url: &str) -> Result<Channel, AppError> {
    debug!("获取 RSS: {}", url);
    let content = fetch(url)?;
    let channel = Channel::read_from(content.as_bytes())
        .map_err(|e| AppError::RssParse(e.to_string()))?;
    debug!("RSS 解析到 {} 条条目", channel.items().len());
    Ok(channel)
}

pub fn parse_updates(channel: &Channel) -> Vec<RssUpdateInfo> {
    channel
        .items()
        .iter()
        .filter_map(|item| {
            let title = item.title().unwrap_or("").to_string();
            let link = item.link().unwrap_or("").to_string();
            let description = item.description().unwrap_or("").to_string();
            let pub_date_str = item.pub_date().unwrap_or("");

            let pub_date = match parse_pub_date(pub_date_str) {
                Ok(d) => d,
                Err(e) => {
                    warn!("跳过 RSS 条目，日期解析失败: title={:?}, pub_date={:?}, error={}", title, pub_date_str, e);
                    return None;
                }
            };
            let platform = extract_platform(&link);
            let (build_number, release_id) = match extract_version(&link) {
                Some(v) => v,
                None => {
                    warn!("跳过 RSS 条目，无法提取版本号: title={:?}, link={:?}", title, link);
                    return None;
                }
            };

            Some(RssUpdateInfo {
                title,
                build_number,
                release_id,
                link,
                description,
                pub_date,
                platform,
            })
        })
        .collect()
}

pub fn latest_update(
    updates: &[RssUpdateInfo],
    platform: Option<Platform>,
) -> Option<&RssUpdateInfo> {
    let target = platform.unwrap_or(Platform::PC);
    updates
        .iter()
        .filter(|u| u.platform == target)
        .max_by_key(|u| u.release_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_extract_platform_pc() {
        assert_eq!(
            extract_platform("https://forums.kleientertainment.com/game-updates/dst/123456-r789/"),
            Platform::PC
        );
    }

    #[test]
    fn test_extract_platform_xbox() {
        assert_eq!(
            extract_platform(
                "https://forums.kleientertainment.com/game-updates/dst_xboxone/123456-r789/"
            ),
            Platform::Xbox
        );
    }

    #[test]
    fn test_extract_platform_playstation() {
        assert_eq!(
            extract_platform(
                "https://forums.kleientertainment.com/game-updates/dst_ps4/123456-r789/"
            ),
            Platform::PlayStation
        );
    }

    #[test]
    fn test_extract_platform_switch() {
        assert_eq!(
            extract_platform(
                "https://forums.kleientertainment.com/game-updates/dst_switch/123456-r789/"
            ),
            Platform::Switch
        );
    }

    #[test]
    fn test_extract_platform_unknown() {
        assert_eq!(
            extract_platform("https://example.com/something/else/"),
            Platform::Unknown
        );
    }

    #[test]
    fn test_extract_version_build_and_release() {
        let (build, release) =
            extract_version("https://forums.kleientertainment.com/game-updates/dst/529998-r1899/")
                .unwrap();
        assert_eq!(build, 529998);
        assert_eq!(release, 1899);
    }

    #[test]
    fn test_extract_version_release_only() {
        let (build, release) =
            extract_version("https://forums.kleientertainment.com/game-updates/dst/r1899/")
                .unwrap();
        assert_eq!(build, 0);
        assert_eq!(release, 1899);
    }

    #[test]
    fn test_extract_version_no_match() {
        assert!(extract_version("https://example.com/no-version-here/").is_none());
    }

    #[test]
    fn test_parse_pub_date_valid() {
        let result = parse_pub_date("Mon, 07 Nov 2022 18:00:00 +0000");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_pub_date_invalid() {
        let result = parse_pub_date("not a date");
        assert!(result.is_err());
    }

    #[test]
    fn test_latest_update_filters_pc_only() {
        let updates = vec![
            RssUpdateInfo {
                title: "Xbox Update".into(),
                build_number: 600000,
                release_id: 100,
                link: String::new(),
                description: String::new(),
                pub_date: FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 0, 0, 0)
                    .unwrap(),
                platform: Platform::Xbox,
            },
            RssUpdateInfo {
                title: "PC Update".into(),
                build_number: 500000,
                release_id: 2,
                link: String::new(),
                description: String::new(),
                pub_date: FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 0, 0, 0)
                    .unwrap(),
                platform: Platform::PC,
            },
        ];
        let result = latest_update(&updates, None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.release_id, 2);
        assert_eq!(r.platform, Platform::PC);
    }

    #[test]
    fn test_latest_update_returns_highest_release_id() {
        let updates = vec![
            RssUpdateInfo {
                title: "Old PC".into(),
                build_number: 500000,
                release_id: 1,
                link: String::new(),
                description: String::new(),
                pub_date: FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 0, 0, 0)
                    .unwrap(),
                platform: Platform::PC,
            },
            RssUpdateInfo {
                title: "New PC".into(),
                build_number: 400000,
                release_id: 2,
                link: String::new(),
                description: String::new(),
                pub_date: FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 6, 1, 0, 0, 0)
                    .unwrap(),
                platform: Platform::PC,
            },
        ];
        let result = latest_update(&updates, None);
        assert_eq!(result.unwrap().release_id, 2);
    }

    #[test]
    fn test_latest_update_empty() {
        let updates: Vec<RssUpdateInfo> = vec![];
        assert!(latest_update(&updates, None).is_none());
    }

    #[test]
    fn test_latest_update_no_pc() {
        let updates = vec![RssUpdateInfo {
            title: "Xbox".into(),
            build_number: 600000,
            release_id: 1,
            link: String::new(),
            description: String::new(),
            pub_date: FixedOffset::east_opt(0)
                .unwrap()
                .with_ymd_and_hms(2022, 1, 1, 0, 0, 0)
                .unwrap(),
            platform: Platform::Xbox,
        }];
        assert!(latest_update(&updates, None).is_none());
    }

    #[test]
    fn test_latest_update_xbox_platform() {
        let updates = vec![
            RssUpdateInfo {
                title: "PC".into(),
                build_number: 500000,
                release_id: 10,
                link: String::new(),
                description: String::new(),
                pub_date: FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 1, 1, 0, 0, 0)
                    .unwrap(),
                platform: Platform::PC,
            },
            RssUpdateInfo {
                title: "Xbox".into(),
                build_number: 600000,
                release_id: 5,
                link: String::new(),
                description: String::new(),
                pub_date: FixedOffset::east_opt(0)
                    .unwrap()
                    .with_ymd_and_hms(2022, 6, 1, 0, 0, 0)
                    .unwrap(),
                platform: Platform::Xbox,
            },
        ];
        let result = latest_update(&updates, Some(Platform::Xbox));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.platform, Platform::Xbox);
        assert_eq!(r.release_id, 5);
    }
}
