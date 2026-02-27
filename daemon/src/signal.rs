// signal.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use tracing::info;

/// Wait for a shutdown signal (SIGINT/SIGTERM on Unix, Ctrl+C on other platforms).
pub async fn wait_for_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

        tokio::select! {
            _ = sigint.recv() => info!("Received SIGINT"),
            _ = sigterm.recv() => info!("Received SIGTERM"),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        info!("Received Ctrl+C");
    }

    Ok(())
}
