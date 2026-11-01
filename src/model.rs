use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::http::FetchError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("网络请求失败: {0}")]
    Http(#[from] FetchError),
    #[error("RSS 解析失败: {0}")]
    RssParse(String),
    #[error("HTML 解析失败: {0}")]
    HtmlParse(String),
    #[error("IO 错误: {0}")]
    Io(String),
    #[error("没有找到更新信息")]
    NoUpdateFound,
}

#[derive(Debug)]
pub struct RssUpdateInfo {
    pub title: String,
    pub build_number: u32,
    pub release_id: u32,
    pub link: String,
    pub description: String,
    pub pub_date: DateTime<FixedOffset>,
    pub platform: Platform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Platform {
    PC,
    Xbox,
    PlayStation,
    Switch,
    Unknown,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::PC => write!(f, "PC"),
            Platform::Xbox => write!(f, "Xbox"),
            Platform::PlayStation => write!(f, "PlayStation"),
            Platform::Switch => write!(f, "Switch"),
            Platform::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HtmlGameUpdate {
    pub build_number: u32,
    pub link: String,
    pub release_date: NaiveDate,
    pub is_hotfix: bool,
    pub status: UpdateStatus,
    pub is_current: bool,
    pub row_id: u32,
    pub release_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateStatus {
    Test,
    Release,
    Unknown,
}

impl fmt::Display for UpdateStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateStatus::Test => write!(f, "Test"),
            UpdateStatus::Release => write!(f, "Release"),
            UpdateStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtmlUpdateDetail {
    pub build_number: u32,
    pub link: String,
    pub release_date: NaiveDate,
    pub is_hotfix: bool,
    pub status: UpdateStatus,
    pub is_current: bool,
    pub row_id: u32,
    pub release_id: u32,
    pub title: String,
    pub content_text: String,
}

#[derive(Serialize, Deserialize)]
pub struct UpdateExport {
    pub total: usize,
    pub fetched_at: String,
    pub updates: Vec<HtmlUpdateDetail>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_display() {
        assert_eq!(format!("{}", Platform::PC), "PC");
        assert_eq!(format!("{}", Platform::Xbox), "Xbox");
        assert_eq!(format!("{}", Platform::PlayStation), "PlayStation");
        assert_eq!(format!("{}", Platform::Switch), "Switch");
        assert_eq!(format!("{}", Platform::Unknown), "Unknown");
    }

    #[test]
    fn test_update_status_display() {
        assert_eq!(format!("{}", UpdateStatus::Test), "Test");
        assert_eq!(format!("{}", UpdateStatus::Release), "Release");
        assert_eq!(format!("{}", UpdateStatus::Unknown), "Unknown");
    }

    #[test]
    fn test_update_status_serde_roundtrip() {
        for status in [
            UpdateStatus::Test,
            UpdateStatus::Release,
            UpdateStatus::Unknown,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: UpdateStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_update_status_serde_values() {
        assert_eq!(
            serde_json::to_string(&UpdateStatus::Test).unwrap(),
            "\"Test\""
        );
        assert_eq!(
            serde_json::to_string(&UpdateStatus::Release).unwrap(),
            "\"Release\""
        );
        assert_eq!(
            serde_json::to_string(&UpdateStatus::Unknown).unwrap(),
            "\"Unknown\""
        );
        assert_eq!(
            serde_json::from_str::<UpdateStatus>("\"Test\"").unwrap(),
            UpdateStatus::Test
        );
        assert_eq!(
            serde_json::from_str::<UpdateStatus>("\"Release\"").unwrap(),
            UpdateStatus::Release
        );
        assert_eq!(
            serde_json::from_str::<UpdateStatus>("\"Unknown\"").unwrap(),
            UpdateStatus::Unknown
        );
    }

    #[test]
    fn test_html_update_detail_serde_roundtrip() {
        let detail = HtmlUpdateDetail {
            build_number: 529998,
            link: "https://example.com/529998-r1899/".to_string(),
            release_date: NaiveDate::from_ymd_opt(2022, 11, 5).unwrap(),
            is_hotfix: true,
            status: UpdateStatus::Test,
            is_current: false,
            row_id: 1899,
            release_id: 1899,
            title: "529998 - Test Update".to_string(),
            content_text: "Some update content".to_string(),
        };
        let json = serde_json::to_string(&detail).unwrap();
        let deserialized: HtmlUpdateDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(detail.build_number, deserialized.build_number);
        assert_eq!(detail.link, deserialized.link);
        assert_eq!(detail.release_date, deserialized.release_date);
        assert_eq!(detail.is_hotfix, deserialized.is_hotfix);
        assert_eq!(detail.status, deserialized.status);
        assert_eq!(detail.is_current, deserialized.is_current);
        assert_eq!(detail.row_id, deserialized.row_id);
        assert_eq!(detail.release_id, deserialized.release_id);
        assert_eq!(detail.title, deserialized.title);
        assert_eq!(detail.content_text, deserialized.content_text);
    }

    #[test]
    fn test_update_export_serde_roundtrip() {
        let detail = HtmlUpdateDetail {
            build_number: 530002,
            link: "https://example.com/530002-r1902/".to_string(),
            release_date: NaiveDate::from_ymd_opt(2022, 12, 1).unwrap(),
            is_hotfix: false,
            status: UpdateStatus::Release,
            is_current: true,
            row_id: 1902,
            release_id: 1902,
            title: "530002 - Release".to_string(),
            content_text: "Release notes".to_string(),
        };
        let export = UpdateExport {
            total: 1,
            fetched_at: "2022-12-01T00:00:00Z".to_string(),
            updates: vec![detail],
        };
        let json = serde_json::to_string(&export).unwrap();
        let deserialized: UpdateExport = serde_json::from_str(&json).unwrap();
        assert_eq!(export.total, deserialized.total);
        assert_eq!(export.fetched_at, deserialized.fetched_at);
        assert_eq!(export.updates.len(), deserialized.updates.len());
        assert_eq!(
            export.updates[0].build_number,
            deserialized.updates[0].build_number
        );
        assert_eq!(
            export.updates[0].content_text,
            deserialized.updates[0].content_text
        );
    }
}
