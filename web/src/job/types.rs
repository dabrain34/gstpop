use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Job type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobType {
    Transcode,
    Demucs,
}

/// Job status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Job is queued, waiting to start
    Pending,
    /// Job is currently processing
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed with an error
    Failed,
    /// Job was cancelled by user
    Cancelled,
}

/// Output format for transcoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Mp4,
    Webm,
    Mkv,
    Mp3,
    Ogg,
    Flac,
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Mp4 => "mp4",
            OutputFormat::Webm => "webm",
            OutputFormat::Mkv => "mkv",
            OutputFormat::Mp3 => "mp3",
            OutputFormat::Ogg => "ogg",
            OutputFormat::Flac => "flac",
        }
    }

    pub fn is_audio_only(&self) -> bool {
        matches!(
            self,
            OutputFormat::Mp3 | OutputFormat::Ogg | OutputFormat::Flac
        )
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mp4" => Ok(OutputFormat::Mp4),
            "webm" => Ok(OutputFormat::Webm),
            "mkv" => Ok(OutputFormat::Mkv),
            "mp3" => Ok(OutputFormat::Mp3),
            "ogg" => Ok(OutputFormat::Ogg),
            "flac" => Ok(OutputFormat::Flac),
            _ => Err(format!("Unknown format: {}", s)),
        }
    }
}

/// Demucs stem types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DemucsModel {
    /// htdemucs - Default hybrid transformer model
    HtDemucs,
    /// htdemucs_ft - Fine-tuned version with better quality
    HtDemucsFineTuned,
    /// htdemucs_6s - 6-stem model (vocals, drums, bass, guitar, piano, other)
    HtDemucs6s,
    /// mdx_extra - MDX-Net model
    MdxExtra,
}

impl Default for DemucsModel {
    fn default() -> Self {
        DemucsModel::HtDemucs
    }
}

impl std::str::FromStr for DemucsModel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "htdemucs" => Ok(DemucsModel::HtDemucs),
            "htdemucs_ft" => Ok(DemucsModel::HtDemucsFineTuned),
            "htdemucs_6s" => Ok(DemucsModel::HtDemucs6s),
            "mdx_extra" => Ok(DemucsModel::MdxExtra),
            _ => Err(format!("Unknown demucs model: {}", s)),
        }
    }
}

impl DemucsModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DemucsModel::HtDemucs => "htdemucs",
            DemucsModel::HtDemucsFineTuned => "htdemucs_ft",
            DemucsModel::HtDemucs6s => "htdemucs_6s",
            DemucsModel::MdxExtra => "mdx_extra",
        }
    }

    /// Returns the stems this model produces
    pub fn stems(&self) -> &'static [&'static str] {
        match self {
            DemucsModel::HtDemucs | DemucsModel::HtDemucsFineTuned | DemucsModel::MdxExtra => {
                &["vocals", "drums", "bass", "other"]
            }
            DemucsModel::HtDemucs6s => &["vocals", "drums", "bass", "guitar", "piano", "other"],
        }
    }
}

/// Transcoding options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodeOptions {
    pub output_format: OutputFormat,
    #[serde(default)]
    pub video_bitrate_kbps: Option<u32>,
    #[serde(default)]
    pub audio_bitrate_kbps: Option<u32>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::Mp4,
            video_bitrate_kbps: None,
            audio_bitrate_kbps: None,
            width: None,
            height: None,
        }
    }
}

/// Demucs separation options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemucsOptions {
    /// Model to use for separation
    #[serde(default)]
    pub model: DemucsModel,
    /// Stems to extract (if empty, extract all)
    #[serde(default)]
    pub stems: Vec<String>,
    /// Output format for stems (wav or flac)
    #[serde(default = "default_stem_format")]
    pub output_format: String,
}

fn default_stem_format() -> String {
    "wav".to_string()
}

impl Default for DemucsOptions {
    fn default() -> Self {
        Self {
            model: DemucsModel::default(),
            stems: vec![], // Empty means all stems
            output_format: "wav".to_string(),
        }
    }
}

/// Job-specific options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JobOptions {
    Transcode(TranscodeOptions),
    Demucs(DemucsOptions),
}

impl JobOptions {
    pub fn job_type(&self) -> JobType {
        match self {
            JobOptions::Transcode(_) => JobType::Transcode,
            JobOptions::Demucs(_) => JobType::Demucs,
        }
    }
}

/// A processing job (transcode or demucs)
#[derive(Debug, Clone)]
pub struct Job {
    /// Unique job ID (UUID)
    pub id: String,
    /// Job type
    pub job_type: JobType,
    /// gpop pipeline ID (assigned when pipeline is created)
    pub pipeline_id: Option<String>,
    /// Current job status
    pub status: JobStatus,
    /// Original input filename
    pub input_filename: String,
    /// Full path to uploaded input file
    pub input_path: PathBuf,
    /// Full path to output file (for transcode) or output directory (for demucs)
    pub output_path: PathBuf,
    /// Output paths for demucs stems (populated after completion)
    pub output_stems: Vec<PathBuf>,
    /// Job options
    pub options: JobOptions,
    /// Progress (0.0 to 1.0)
    pub progress: f64,
    /// Current position in nanoseconds
    pub position_ns: Option<u64>,
    /// Total duration in nanoseconds
    pub duration_ns: Option<u64>,
    /// Error message if failed
    pub error: Option<String>,
    /// When the job was created
    pub created_at: DateTime<Utc>,
    /// When processing started
    pub started_at: Option<DateTime<Utc>>,
    /// When processing completed
    pub completed_at: Option<DateTime<Utc>>,
}

impl Job {
    pub fn new_transcode(
        id: String,
        input_filename: String,
        input_path: PathBuf,
        output_path: PathBuf,
        options: TranscodeOptions,
    ) -> Self {
        Self {
            id,
            job_type: JobType::Transcode,
            pipeline_id: None,
            status: JobStatus::Pending,
            input_filename,
            input_path,
            output_path,
            output_stems: vec![],
            options: JobOptions::Transcode(options),
            progress: 0.0,
            position_ns: None,
            duration_ns: None,
            error: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn new_demucs(
        id: String,
        input_filename: String,
        input_path: PathBuf,
        output_dir: PathBuf,
        options: DemucsOptions,
    ) -> Self {
        Self {
            id,
            job_type: JobType::Demucs,
            pipeline_id: None,
            status: JobStatus::Pending,
            input_filename,
            input_path,
            output_path: output_dir,
            output_stems: vec![],
            options: JobOptions::Demucs(options),
            progress: 0.0,
            position_ns: None,
            duration_ns: None,
            error: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    /// Get transcode options if this is a transcode job
    pub fn transcode_options(&self) -> Option<&TranscodeOptions> {
        match &self.options {
            JobOptions::Transcode(opts) => Some(opts),
            _ => None,
        }
    }

    /// Get demucs options if this is a demucs job
    pub fn demucs_options(&self) -> Option<&DemucsOptions> {
        match &self.options {
            JobOptions::Demucs(opts) => Some(opts),
            _ => None,
        }
    }
}

/// Job summary for API responses
#[derive(Debug, Clone, Serialize)]
pub struct JobSummary {
    pub id: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub input_filename: String,
    /// Output format (for transcode) or model (for demucs)
    pub output_info: String,
    pub progress: f64,
    pub created_at: DateTime<Utc>,
}

impl From<&Job> for JobSummary {
    fn from(job: &Job) -> Self {
        let output_info = match &job.options {
            JobOptions::Transcode(opts) => opts.output_format.extension().to_string(),
            JobOptions::Demucs(opts) => opts.model.as_str().to_string(),
        };
        Self {
            id: job.id.clone(),
            job_type: job.job_type,
            status: job.status,
            input_filename: job.input_filename.clone(),
            output_info,
            progress: job.progress,
            created_at: job.created_at,
        }
    }
}

/// Full job details for API responses
#[derive(Debug, Clone, Serialize)]
pub struct JobDetails {
    pub id: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub input_filename: String,
    pub options: JobOptions,
    pub progress: f64,
    pub position_ns: Option<u64>,
    pub duration_ns: Option<u64>,
    pub error: Option<String>,
    /// Download URL for single-file output (transcode)
    pub download_url: Option<String>,
    /// Download URLs for multiple outputs (demucs stems)
    pub download_urls: Option<Vec<StemDownload>>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Stem download info for demucs jobs
#[derive(Debug, Clone, Serialize)]
pub struct StemDownload {
    pub stem: String,
    pub url: String,
}

impl JobDetails {
    pub fn from_job(
        job: &Job,
        download_url: Option<String>,
        download_urls: Option<Vec<StemDownload>>,
    ) -> Self {
        Self {
            id: job.id.clone(),
            job_type: job.job_type,
            status: job.status,
            input_filename: job.input_filename.clone(),
            options: job.options.clone(),
            progress: job.progress,
            position_ns: job.position_ns,
            duration_ns: job.duration_ns,
            error: job.error.clone(),
            download_url,
            download_urls,
            created_at: job.created_at,
            started_at: job.started_at,
            completed_at: job.completed_at,
        }
    }
}
