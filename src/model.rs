use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::Serialize;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Serialize)]
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
}
