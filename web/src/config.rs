use clap::Parser;
use std::path::PathBuf;

/// Web interface for gpop transcoding
#[derive(Parser, Debug, Clone)]
#[command(name = "gpop-web")]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Server host address
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    pub host: String,

    /// Server port
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    /// gpop daemon WebSocket URL
    #[arg(short, long, default_value = "ws://127.0.0.1:9000")]
    pub gpop_url: String,

    /// Data directory for uploads and outputs
    #[arg(short, long, default_value = "./data")]
    pub data_dir: PathBuf,

    /// Maximum upload file size in megabytes
    #[arg(long, default_value_t = 2048)]
    pub max_upload_mb: usize,

    /// Maximum concurrent transcoding jobs
    #[arg(long, default_value_t = 4)]
    pub max_concurrent_jobs: usize,

    /// File retention period in hours (0 = keep forever)
    #[arg(long, default_value_t = 24)]
    pub retention_hours: u64,

    /// API key for authentication (optional, reads from GPOP_WEB_API_KEY env var)
    #[arg(long, env = "GPOP_WEB_API_KEY")]
    pub api_key: Option<String>,
}

impl Config {
    pub fn max_upload_bytes(&self) -> usize {
        self.max_upload_mb * 1024 * 1024
    }

    pub fn uploads_dir(&self) -> PathBuf {
        self.data_dir.join("uploads")
    }

    pub fn outputs_dir(&self) -> PathBuf {
        self.data_dir.join("outputs")
    }
}

/// Validate that the gpop URL uses a WebSocket scheme
pub fn validate_gpop_url(url: &str) -> std::result::Result<(), String> {
    if !url.starts_with("ws://") && !url.starts_with("wss://") {
        return Err(format!(
            "Invalid gpop URL scheme: '{}'. Must start with ws:// or wss://",
            url
        ));
    }
    Ok(())
}

/// Allowed file extensions for upload
pub const ALLOWED_EXTENSIONS: &[&str] = &[
    // Video formats
    "mp4", "mkv", "webm", "avi", "mov", "wmv", "flv", "m4v", "ts", "mts",
    // Audio formats
    "mp3", "wav", "ogg", "flac", "aac", "m4a", "wma", "opus",
];

/// Audio-only file extensions (for demucs)
pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "aac", "m4a", "wma", "opus"];

/// Check if a file extension is allowed
pub fn is_allowed_extension(ext: &str) -> bool {
    ALLOWED_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// Check if a file extension is an audio format (for demucs)
pub fn is_audio_extension(ext: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}
