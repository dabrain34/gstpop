use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

use crate::auth::require_auth;
use crate::config::{is_allowed_extension, is_audio_extension, Config};
use crate::error::{AppError, Result};
use crate::job::{DemucsModel, DemucsOptions, JobManager, JobType, OutputFormat, TranscodeOptions};

/// Response for job creation
#[derive(Serialize)]
pub struct JobCreatedResponse {
    job_id: String,
    job_type: String,
    status: String,
    message: String,
}

/// Response for job list
#[derive(Serialize)]
pub struct JobListResponse {
    jobs: Vec<crate::job::JobSummary>,
    total: usize,
}

/// Query parameters for transcode job creation
#[derive(Debug, Deserialize)]
pub struct CreateTranscodeQuery {
    #[serde(default = "default_format")]
    output_format: String,
    video_bitrate_kbps: Option<u32>,
    audio_bitrate_kbps: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
}

fn default_format() -> String {
    "mp4".to_string()
}

/// Query parameters for demucs job creation
#[derive(Debug, Deserialize)]
pub struct CreateDemucsQuery {
    #[serde(default = "default_model")]
    model: String,
    /// Comma-separated list of stems to extract (empty = all)
    #[serde(default)]
    stems: String,
    #[serde(default = "default_stem_format")]
    output_format: String,
}

fn default_model() -> String {
    "htdemucs".to_string()
}

fn default_stem_format() -> String {
    "wav".to_string()
}

/// Extract file from multipart form
async fn extract_file_from_multipart(
    mut payload: Multipart,
    max_size: usize,
) -> Result<(Vec<u8>, String)> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;

    while let Some(field) = payload.next().await {
        let mut field = field.map_err(|e| AppError::Internal(e.to_string()))?;

        let content_disposition = field.content_disposition();
        let field_name = content_disposition
            .and_then(|cd| cd.get_name())
            .unwrap_or("");

        if field_name == "file" {
            filename = content_disposition
                .and_then(|cd| cd.get_filename())
                .map(|s| sanitize_filename::sanitize(s));

            let mut bytes = Vec::new();
            while let Some(chunk) = field.next().await {
                let chunk = chunk.map_err(|e| AppError::Internal(e.to_string()))?;
                if bytes.len() + chunk.len() > max_size {
                    return Err(AppError::FileTooLarge(bytes.len() + chunk.len(), max_size));
                }
                bytes.extend_from_slice(&chunk);
            }
            file_data = Some(bytes);
        }
    }

    let file_data = file_data.ok_or_else(|| AppError::Internal("No file uploaded".to_string()))?;
    let filename =
        filename.ok_or_else(|| AppError::Internal("No filename provided".to_string()))?;

    Ok((file_data, filename))
}

/// POST /api/jobs/transcode - Create a new transcoding job
pub async fn create_transcode_job(
    req: HttpRequest,
    payload: Multipart,
    query: web::Query<CreateTranscodeQuery>,
    manager: web::Data<Arc<JobManager>>,
    config: web::Data<Config>,
) -> Result<HttpResponse> {
    if let Err(resp) = require_auth(&req) {
        return Ok(resp);
    }

    let (file_data, filename) =
        extract_file_from_multipart(payload, config.max_upload_bytes()).await?;

    // Validate file extension
    let extension = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| AppError::InvalidFileType("unknown".to_string()))?;

    if !is_allowed_extension(&extension) {
        return Err(AppError::InvalidFileType(extension));
    }

    // Parse output format
    let output_format: OutputFormat = query
        .output_format
        .parse()
        .map_err(|e: String| AppError::Internal(e))?;

    // Build transcode options
    let options = TranscodeOptions {
        output_format,
        video_bitrate_kbps: query.video_bitrate_kbps,
        audio_bitrate_kbps: query.audio_bitrate_kbps,
        width: query.width,
        height: query.height,
    };

    info!(
        "Creating transcode job: {} ({} bytes) -> {}",
        filename,
        file_data.len(),
        output_format.extension()
    );

    let job_id = manager
        .create_transcode_job(&filename, &file_data, options)
        .await?;

    Ok(HttpResponse::Created().json(JobCreatedResponse {
        job_id,
        job_type: "transcode".to_string(),
        status: "pending".to_string(),
        message: "Transcode job created successfully".to_string(),
    }))
}

/// POST /api/jobs/demucs - Create a new demucs source separation job
pub async fn create_demucs_job(
    req: HttpRequest,
    payload: Multipart,
    query: web::Query<CreateDemucsQuery>,
    manager: web::Data<Arc<JobManager>>,
    config: web::Data<Config>,
) -> Result<HttpResponse> {
    if let Err(resp) = require_auth(&req) {
        return Ok(resp);
    }

    let (file_data, filename) =
        extract_file_from_multipart(payload, config.max_upload_bytes()).await?;

    // Validate file extension (must be audio)
    let extension = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| AppError::InvalidFileType("unknown".to_string()))?;

    if !is_audio_extension(&extension) {
        return Err(AppError::InvalidFileType(format!(
            "{} (demucs requires audio files)",
            extension
        )));
    }

    // Parse model
    let model: DemucsModel = query
        .model
        .parse()
        .map_err(|e: String| AppError::Internal(e))?;

    // Parse stems (comma-separated list)
    let stems: Vec<String> = if query.stems.is_empty() {
        vec![]
    } else {
        query
            .stems
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .collect()
    };

    // Validate stems for the model
    let valid_stems = model.stems();
    for stem in &stems {
        if !valid_stems.contains(&stem.as_str()) {
            return Err(AppError::Internal(format!(
                "Invalid stem '{}' for model {}. Valid stems: {:?}",
                stem,
                model.as_str(),
                valid_stems
            )));
        }
    }

    // Validate output format
    if query.output_format != "wav" && query.output_format != "flac" {
        return Err(AppError::Internal(format!(
            "Invalid output format '{}'. Must be 'wav' or 'flac'",
            query.output_format
        )));
    }

    let options = DemucsOptions {
        model,
        stems,
        output_format: query.output_format.clone(),
    };

    info!(
        "Creating demucs job: {} ({} bytes) with model {}",
        filename,
        file_data.len(),
        model.as_str()
    );

    let job_id = manager
        .create_demucs_job(&filename, &file_data, options)
        .await?;

    Ok(HttpResponse::Created().json(JobCreatedResponse {
        job_id,
        job_type: "demucs".to_string(),
        status: "pending".to_string(),
        message: "Demucs job created successfully".to_string(),
    }))
}

/// GET /api/jobs - List all jobs
pub async fn list_jobs(manager: web::Data<Arc<JobManager>>) -> Result<HttpResponse> {
    let jobs = manager.list_jobs().await;
    let total = jobs.len();

    Ok(HttpResponse::Ok().json(JobListResponse { jobs, total }))
}

/// GET /api/jobs/{id} - Get job details
pub async fn get_job(
    path: web::Path<String>,
    manager: web::Data<Arc<JobManager>>,
) -> Result<HttpResponse> {
    let job_id = path.into_inner();
    let details = manager.get_job_details(&job_id).await?;

    Ok(HttpResponse::Ok().json(details))
}

/// DELETE /api/jobs/{id} - Delete a job
pub async fn delete_job(
    req: HttpRequest,
    path: web::Path<String>,
    manager: web::Data<Arc<JobManager>>,
) -> Result<HttpResponse> {
    if let Err(resp) = require_auth(&req) {
        return Ok(resp);
    }

    let job_id = path.into_inner();
    manager.delete_job(&job_id).await?;

    Ok(HttpResponse::NoContent().finish())
}

/// GET /api/jobs/{id}/download - Download the output file (transcode jobs)
pub async fn download_job(
    path: web::Path<String>,
    manager: web::Data<Arc<JobManager>>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    let job_id = path.into_inner();
    let job = manager.get_job(&job_id).await?;

    // Check if job is completed
    if job.status != crate::job::JobStatus::Completed {
        return Err(AppError::Internal("Job not completed".to_string()));
    }

    // Check job type
    if job.job_type != JobType::Transcode {
        return Err(AppError::Internal(
            "Use /download/{stem} endpoint for demucs jobs".to_string(),
        ));
    }

    // Check if output file exists
    let output_path = &job.output_path;
    if !output_path.exists() {
        return Err(AppError::FileNotFound(output_path.display().to_string()));
    }

    // Determine content type
    let content_type = mime_guess::from_path(output_path)
        .first_or_octet_stream()
        .to_string();

    // Create filename for download
    let download_filename = if let Some(opts) = job.transcode_options() {
        format!(
            "{}.{}",
            Path::new(&job.input_filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output"),
            opts.output_format.extension()
        )
    } else {
        "output".to_string()
    };

    debug!(
        "Serving download: {} as {}",
        output_path.display(),
        download_filename
    );

    let file = actix_files::NamedFile::open(output_path)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .set_content_disposition(actix_web::http::header::ContentDisposition {
            disposition: actix_web::http::header::DispositionType::Attachment,
            parameters: vec![actix_web::http::header::DispositionParam::Filename(
                download_filename,
            )],
        })
        .set_content_type(content_type.parse().unwrap());

    Ok(file.into_response(&req))
}

/// GET /api/jobs/{id}/download/{stem} - Download a demucs stem
pub async fn download_stem(
    path: web::Path<(String, String)>,
    manager: web::Data<Arc<JobManager>>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    let (job_id, stem) = path.into_inner();

    // Validate stem name to prevent path traversal
    if stem.contains('/') || stem.contains('\\') || stem.contains("..") {
        return Err(AppError::Internal("Invalid stem name".to_string()));
    }

    // Get the stem file path
    let stem_path = manager.get_demucs_stem_path(&job_id, &stem).await?;

    // Determine content type
    let content_type = mime_guess::from_path(&stem_path)
        .first_or_octet_stream()
        .to_string();

    // Get job for filename
    let job = manager.get_job(&job_id).await?;

    // Create filename for download
    let ext = stem_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("wav");
    let download_filename = format!(
        "{}_{}.{}",
        Path::new(&job.input_filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output"),
        stem,
        ext
    );

    debug!(
        "Serving stem download: {} as {}",
        stem_path.display(),
        download_filename
    );

    let file = actix_files::NamedFile::open(&stem_path)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .set_content_disposition(actix_web::http::header::ContentDisposition {
            disposition: actix_web::http::header::DispositionType::Attachment,
            parameters: vec![actix_web::http::header::DispositionParam::Filename(
                download_filename,
            )],
        })
        .set_content_type(content_type.parse().unwrap());

    Ok(file.into_response(&req))
}

/// GET /health - Health check endpoint
pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok"
    }))
}

/// Configure job API routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route("/jobs/transcode", web::post().to(create_transcode_job))
            .route("/jobs/demucs", web::post().to(create_demucs_job))
            .route("/jobs", web::get().to(list_jobs))
            .route("/jobs/{id}", web::get().to(get_job))
            .route("/jobs/{id}", web::delete().to(delete_job))
            .route("/jobs/{id}/download", web::get().to(download_job))
            .route("/jobs/{id}/download/{stem}", web::get().to(download_stem)),
    )
    .route("/health", web::get().to(health_check));
}
