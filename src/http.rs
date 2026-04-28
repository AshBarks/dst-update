use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub max_retries: u32,
    pub timeout: Duration,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            max_retries: 3,
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
pub enum FetchError {
    HttpError(String),
    NetworkError(String),
    MaxRetriesExceeded,
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            FetchError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            FetchError::MaxRetriesExceeded => write!(f, "Max retries exceeded"),
        }
    }
}

impl std::error::Error for FetchError {}

pub fn fetch(url: &str) -> Result<String, FetchError> {
    fetch_with_config(url, &HttpConfig::default())
}

pub fn fetch_with_config(url: &str, config: &HttpConfig) -> Result<String, FetchError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(config.timeout)
        .build()
        .map_err(|e| FetchError::NetworkError(e.to_string()))?;

    let mut delay = config.initial_delay;

    for attempt in 0..=config.max_retries {
        match client.get(url).send() {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    return response
                        .text()
                        .map_err(|e| FetchError::HttpError(e.to_string()));
                } else if status.is_server_error() {
                    if attempt < config.max_retries {
                        thread::sleep(delay);
                        delay = (delay * 2).min(config.max_delay);
                        continue;
                    }
                    return Err(FetchError::HttpError(format!("Server error: {}", status)));
                } else {
                    return Err(FetchError::HttpError(format!("Client error: {}", status)));
                }
            }
            Err(e) => {
                if attempt < config.max_retries {
                    thread::sleep(delay);
                    delay = (delay * 2).min(config.max_delay);
                    continue;
                }
                return Err(FetchError::NetworkError(e.to_string()));
            }
        }
    }

    Err(FetchError::MaxRetriesExceeded)
}
