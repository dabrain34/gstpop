use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::storage::StorageManager;
use crate::ws::{GpopConnection, GpopEvent, ProgressBroadcaster, ProgressMessage};

use super::pipeline::{build_demucs_pipeline, build_transcode_pipeline, get_demucs_output_files};
use super::types::*;

/// Job manager handles the lifecycle of transcoding and demucs jobs
pub struct JobManager {
    jobs: RwLock<HashMap<String, Job>>,
    /// Map pipeline_id -> job_id for event routing
    pipeline_to_job: RwLock<HashMap<String, String>>,
    gpop: Arc<GpopConnection>,
    storage: Arc<StorageManager>,
    broadcaster: Arc<ProgressBroadcaster>,
    config: Config,
}

impl JobManager {
    pub fn new(
        gpop: Arc<GpopConnection>,
        storage: Arc<StorageManager>,
        broadcaster: Arc<ProgressBroadcaster>,
        config: Config,
    ) -> Self {
        Self {
            jobs: RwLock::new(HashMap::new()),
            pipeline_to_job: RwLock::new(HashMap::new()),
            gpop,
            storage,
            broadcaster,
            config,
        }
    }

    /// Check if there's room for another concurrent job
    async fn check_concurrent_limit(&self) -> Result<()> {
        let jobs = self.jobs.read().await;
        let active_count = jobs
            .values()
            .filter(|j| matches!(j.status, JobStatus::Processing | JobStatus::Pending))
            .count();

        if active_count >= self.config.max_concurrent_jobs {
            return Err(AppError::Internal(format!(
                "Maximum concurrent jobs ({}) reached",
                self.config.max_concurrent_jobs
            )));
        }
        Ok(())
    }

    /// Create a new transcoding job
    pub async fn create_transcode_job(
        &self,
        filename: &str,
        data: &[u8],
        options: TranscodeOptions,
    ) -> Result<String> {
        self.check_concurrent_limit().await?;

        // Generate job ID
        let job_id = uuid::Uuid::new_v4().to_string();

        // Store the uploaded file
        let input_path = self.storage.store_upload(&job_id, filename, data).await?;

        // Determine output path
        let output_path = self
            .storage
            .job_output_path(&job_id, options.output_format.extension());

        // Create job
        let job = Job::new_transcode(
            job_id.clone(),
            filename.to_string(),
            input_path.clone(),
            output_path.clone(),
            options.clone(),
        );

        // Store job
        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(job_id.clone(), job);
        }

        info!(
            "Created transcode job {}: {} -> {}",
            job_id,
            filename,
            options.output_format.extension()
        );

        // Build and create pipeline
        let pipeline_desc = build_transcode_pipeline(&input_path, &output_path, &options);
        debug!("Pipeline for job {}: {}", job_id, pipeline_desc);

        self.start_pipeline(&job_id, &pipeline_desc).await?;

        Ok(job_id)
    }

    /// Create a new demucs job
    pub async fn create_demucs_job(
        &self,
        filename: &str,
        data: &[u8],
        options: DemucsOptions,
    ) -> Result<String> {
        self.check_concurrent_limit().await?;

        // Generate job ID
        let job_id = uuid::Uuid::new_v4().to_string();

        // Store the uploaded file
        let input_path = self.storage.store_upload(&job_id, filename, data).await?;

        // Create output directory for stems
        let output_dir = self.storage.job_demucs_output_dir(&job_id).await?;

        // Create job
        let job = Job::new_demucs(
            job_id.clone(),
            filename.to_string(),
            input_path.clone(),
            output_dir.clone(),
            options.clone(),
        );

        // Store job
        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(job_id.clone(), job);
        }

        info!(
            "Created demucs job {}: {} (model: {})",
            job_id,
            filename,
            options.model.as_str()
        );

        // Build and create pipeline
        let pipeline_desc = build_demucs_pipeline(&input_path, &output_dir, &options);
        debug!("Pipeline for job {}: {}", job_id, pipeline_desc);

        self.start_pipeline(&job_id, &pipeline_desc).await?;

        Ok(job_id)
    }

    /// Start a pipeline for a job
    async fn start_pipeline(&self, job_id: &str, pipeline_desc: &str) -> Result<()> {
        match self.gpop.create_pipeline(pipeline_desc).await {
            Ok(pipeline_id) => {
                // Update job with pipeline ID
                {
                    let mut jobs = self.jobs.write().await;
                    if let Some(job) = jobs.get_mut(job_id) {
                        job.pipeline_id = Some(pipeline_id.clone());
                        job.status = JobStatus::Processing;
                        job.started_at = Some(Utc::now());
                    }
                }

                // Map pipeline to job
                {
                    let mut mapping = self.pipeline_to_job.write().await;
                    mapping.insert(pipeline_id.clone(), job_id.to_string());
                }

                // Start the pipeline
                if let Err(e) = self.gpop.play(&pipeline_id).await {
                    error!("Failed to start pipeline for job {}: {}", job_id, e);
                    self.mark_job_failed(job_id, &e.to_string()).await;
                    return Err(AppError::PipelineCreation(e.to_string()));
                }

                // Broadcast job started
                self.broadcaster.send(ProgressMessage::JobStarted {
                    job_id: job_id.to_string(),
                });

                Ok(())
            }
            Err(e) => {
                error!("Failed to create pipeline for job {}: {}", job_id, e);
                self.mark_job_failed(job_id, &e.to_string()).await;
                Err(AppError::PipelineCreation(e.to_string()))
            }
        }
    }

    /// Get a job by ID
    pub async fn get_job(&self, job_id: &str) -> Result<Job> {
        let jobs = self.jobs.read().await;
        jobs.get(job_id)
            .cloned()
            .ok_or_else(|| AppError::JobNotFound(job_id.to_string()))
    }

    /// Get job details with download URLs if applicable
    pub async fn get_job_details(&self, job_id: &str) -> Result<JobDetails> {
        let job = self.get_job(job_id).await?;

        let (download_url, download_urls) = if job.status == JobStatus::Completed {
            match job.job_type {
                JobType::Transcode => (Some(format!("/api/jobs/{}/download", job_id)), None),
                JobType::Demucs => {
                    // Generate download URLs for each stem
                    if let Some(opts) = job.demucs_options() {
                        let stem_files = get_demucs_output_files(&job.output_path, opts);
                        let urls: Vec<StemDownload> = stem_files
                            .into_iter()
                            .map(|(stem, _)| StemDownload {
                                stem: stem.clone(),
                                url: format!("/api/jobs/{}/download/{}", job_id, stem),
                            })
                            .collect();
                        (None, Some(urls))
                    } else {
                        (None, None)
                    }
                }
            }
        } else {
            (None, None)
        };

        Ok(JobDetails::from_job(&job, download_url, download_urls))
    }

    /// List all jobs
    pub async fn list_jobs(&self) -> Vec<JobSummary> {
        let jobs = self.jobs.read().await;
        jobs.values().map(JobSummary::from).collect()
    }

    /// Cancel and delete a job (two-phase: mark cancelled, cleanup, then remove)
    pub async fn delete_job(&self, job_id: &str) -> Result<()> {
        // Phase 1: Mark as cancelled
        let job = {
            let mut jobs = self.jobs.write().await;
            match jobs.get_mut(job_id) {
                Some(job) => {
                    job.status = JobStatus::Cancelled;
                    job.completed_at = Some(Utc::now());
                    job.clone()
                }
                None => return Err(AppError::JobNotFound(job_id.to_string())),
            }
        };

        {
            // Stop the pipeline if running
            if let Some(pipeline_id) = &job.pipeline_id {
                let _ = self.gpop.stop(pipeline_id).await;
                let _ = self.gpop.remove_pipeline(pipeline_id).await;

                // Remove mapping
                let mut mapping = self.pipeline_to_job.write().await;
                mapping.remove(pipeline_id);
            }

            // Clean up files based on job type
            let _ = self.storage.cleanup_job(job_id).await;

            match job.job_type {
                JobType::Transcode => {
                    if let Some(opts) = job.transcode_options() {
                        let _ = self
                            .storage
                            .cleanup_output(job_id, opts.output_format.extension())
                            .await;
                    }
                }
                JobType::Demucs => {
                    // Clean up entire output directory
                    let _ = self.storage.cleanup_demucs_output(job_id).await;
                }
            }

            info!("Deleted job {}", job_id);
        }

        // Phase 2: Remove from jobs map
        {
            let mut jobs = self.jobs.write().await;
            jobs.remove(job_id);
        }

        Ok(())
    }

    /// Get a stem file path for demucs job download
    pub async fn get_demucs_stem_path(&self, job_id: &str, stem: &str) -> Result<PathBuf> {
        let job = self.get_job(job_id).await?;

        if job.job_type != JobType::Demucs {
            return Err(AppError::Internal("Not a demucs job".to_string()));
        }

        if job.status != JobStatus::Completed {
            return Err(AppError::Internal("Job not completed".to_string()));
        }

        let opts = job
            .demucs_options()
            .ok_or_else(|| AppError::Internal("Missing demucs options".to_string()))?;

        let stem_files = get_demucs_output_files(&job.output_path, opts);

        for (s, path) in stem_files {
            if s == stem {
                if path.exists() {
                    return Ok(path);
                } else {
                    return Err(AppError::FileNotFound(path.display().to_string()));
                }
            }
        }

        Err(AppError::FileNotFound(format!("Stem '{}' not found", stem)))
    }

    /// Mark a job as failed
    async fn mark_job_failed(&self, job_id: &str, error: &str) {
        {
            let mut jobs = self.jobs.write().await;
            if let Some(job) = jobs.get_mut(job_id) {
                job.status = JobStatus::Failed;
                job.error = Some(error.to_string());
                job.completed_at = Some(Utc::now());
            }
        }

        self.broadcaster.send(ProgressMessage::JobFailed {
            job_id: job_id.to_string(),
            error: error.to_string(),
        });
    }

    /// Mark a job as completed
    async fn mark_job_completed(&self, job_id: &str) {
        let job_type = {
            let mut jobs = self.jobs.write().await;
            if let Some(job) = jobs.get_mut(job_id) {
                job.status = JobStatus::Completed;
                job.progress = 1.0;
                job.completed_at = Some(Utc::now());

                // For demucs jobs, populate output_stems
                if job.job_type == JobType::Demucs {
                    if let Some(opts) = job.demucs_options() {
                        let stem_files = get_demucs_output_files(&job.output_path, opts);
                        job.output_stems = stem_files.into_iter().map(|(_, p)| p).collect();
                    }
                }

                Some(job.job_type)
            } else {
                None
            }
        };

        // Send appropriate completion message
        match job_type {
            Some(JobType::Transcode) => {
                let download_url = format!("/api/jobs/{}/download", job_id);
                self.broadcaster.send(ProgressMessage::JobCompleted {
                    job_id: job_id.to_string(),
                    download_url,
                });
            }
            Some(JobType::Demucs) => {
                // For demucs, we use a special message with multiple URLs
                self.broadcaster.send(ProgressMessage::DemucsCompleted {
                    job_id: job_id.to_string(),
                });
            }
            None => {}
        }

        info!("Job {} completed", job_id);
    }

    /// Handle events from gpop-daemon
    pub async fn handle_gpop_event(&self, event: GpopEvent) {
        match event {
            GpopEvent::Eos { pipeline_id } => {
                if let Some(job_id) = self.get_job_id_for_pipeline(&pipeline_id).await {
                    self.mark_job_completed(&job_id).await;

                    // Clean up the pipeline
                    let _ = self.gpop.remove_pipeline(&pipeline_id).await;

                    // Remove mapping
                    let mut mapping = self.pipeline_to_job.write().await;
                    mapping.remove(&pipeline_id);
                }
            }
            GpopEvent::Error {
                pipeline_id,
                message,
            } => {
                if let Some(job_id) = self.get_job_id_for_pipeline(&pipeline_id).await {
                    self.mark_job_failed(&job_id, &message).await;

                    // Clean up the pipeline
                    let _ = self.gpop.remove_pipeline(&pipeline_id).await;

                    // Remove mapping
                    let mut mapping = self.pipeline_to_job.write().await;
                    mapping.remove(&pipeline_id);
                }
            }
            GpopEvent::StateChanged {
                pipeline_id,
                old_state,
                new_state,
            } => {
                if let Some(job_id) = self.get_job_id_for_pipeline(&pipeline_id).await {
                    self.broadcaster.send(ProgressMessage::StateChanged {
                        job_id,
                        old_state,
                        new_state,
                    });
                }
            }
            _ => {}
        }
    }

    /// Get job ID for a pipeline ID
    async fn get_job_id_for_pipeline(&self, pipeline_id: &str) -> Option<String> {
        let mapping = self.pipeline_to_job.read().await;
        mapping.get(pipeline_id).cloned()
    }

    /// Poll progress for all active jobs.
    /// Collects all position queries first, then applies updates in a single write lock.
    pub async fn poll_progress(&self) {
        let active_jobs: Vec<(String, String)> = {
            let jobs = self.jobs.read().await;
            jobs.values()
                .filter(|j| j.status == JobStatus::Processing && j.pipeline_id.is_some())
                .map(|j| (j.id.clone(), j.pipeline_id.clone().unwrap()))
                .collect()
        };

        if active_jobs.is_empty() {
            return;
        }

        // Phase 1: Collect all position results (no locks held)
        let mut updates: Vec<(String, Option<u64>, Option<u64>)> = Vec::new();
        for (job_id, pipeline_id) in active_jobs {
            match self.gpop.get_position(&pipeline_id).await {
                Ok(pos) => {
                    updates.push((job_id, pos.position_ns, pos.duration_ns));
                }
                Err(e) => {
                    debug!("Failed to get position for job {}: {}", job_id, e);
                }
            }
        }

        if updates.is_empty() {
            return;
        }

        // Phase 2: Apply all updates under a single write lock
        let broadcasts: Vec<_> = {
            let mut jobs = self.jobs.write().await;
            updates
                .iter()
                .filter_map(|(job_id, position_ns, duration_ns)| {
                    let progress = match (position_ns, duration_ns) {
                        (Some(pos), Some(dur)) if *dur > 0 => *pos as f64 / *dur as f64,
                        _ => 0.0,
                    };
                    if let Some(job) = jobs.get_mut(job_id) {
                        job.progress = progress;
                        job.position_ns = *position_ns;
                        job.duration_ns = *duration_ns;
                        Some((job_id.clone(), progress, *position_ns, *duration_ns))
                    } else {
                        None
                    }
                })
                .collect()
        };

        // Phase 3: Broadcast outside the lock
        for (job_id, progress, position_ns, duration_ns) in broadcasts {
            self.broadcaster.send(ProgressMessage::Progress {
                job_id,
                progress,
                position_ns,
                duration_ns,
            });
        }
    }

    /// Get the broadcaster for WebSocket clients
    pub fn broadcaster(&self) -> Arc<ProgressBroadcaster> {
        Arc::clone(&self.broadcaster)
    }

    /// Get the storage manager
    pub fn storage(&self) -> Arc<StorageManager> {
        Arc::clone(&self.storage)
    }
}

/// Start the event handling loop
pub async fn start_event_handler(manager: Arc<JobManager>, gpop: Arc<GpopConnection>) {
    let mut event_rx = gpop.subscribe_events();

    loop {
        match event_rx.recv().await {
            Ok(event) => {
                manager.handle_gpop_event(event).await;
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                warn!("Event handler lagged, missed {} events", n);
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                info!("Event channel closed");
                break;
            }
        }
    }
}

/// Start the progress polling loop
pub async fn start_progress_poller(manager: Arc<JobManager>, interval: Duration) {
    let mut interval_timer = tokio::time::interval(interval);

    loop {
        interval_timer.tick().await;
        manager.poll_progress().await;
    }
}
