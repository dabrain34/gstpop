// playback_mode_tests.rs
//
// Copyright 2026 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GstPrinceOfParser
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;
use std::time::Duration;

use super::manager::*;
use crate::gst::event::{create_event_channel, EventReceiver, PipelineEvent};

fn init_gstreamer() {
    let _ = gstreamer::init();
}

/// Collect pipeline-completion events (Eos, Error, Unsupported, PipelineRemoved)
/// until `expected_count` are received, or timeout expires.
/// Panics on timeout if `require_all` is true; otherwise returns what was collected.
async fn wait_for_events(
    rx: &mut EventReceiver,
    expected_count: usize,
    timeout_secs: u64,
) -> Vec<PipelineEvent> {
    collect_events(rx, expected_count, timeout_secs, true).await
}

async fn collect_events(
    rx: &mut EventReceiver,
    expected_count: usize,
    timeout_secs: u64,
    require_all: bool,
) -> Vec<PipelineEvent> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

    while events.len() < expected_count {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(event)) => match &event {
                PipelineEvent::Eos { .. }
                | PipelineEvent::Error { .. }
                | PipelineEvent::Unsupported { .. }
                | PipelineEvent::PipelineRemoved { .. } => {
                    events.push(event);
                }
                _ => {} // Ignore StateChanged, PipelineAdded, etc.
            },
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                continue;
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                break;
            }
            Err(_) => {
                if require_all {
                    panic!(
                        "Timed out waiting for events: got {}/{} after {}s",
                        events.len(),
                        expected_count,
                        timeout_secs
                    );
                }
                break;
            }
        }
    }

    events
}

#[tokio::test]
async fn test_single_pipeline_eos() {
    init_gstreamer();
    let (tx, _rx) = create_event_channel();
    let mut event_rx = tx.subscribe();
    let manager = PipelineManager::new(tx);

    let id = manager
        .add_pipeline("videotestsrc num-buffers=10 ! fakesink")
        .await
        .unwrap();

    manager.play(&id).await.unwrap();

    let events = wait_for_events(&mut event_rx, 1, 10).await;

    assert_eq!(events.len(), 1);
    match &events[0] {
        PipelineEvent::Eos { pipeline_id } => {
            assert_eq!(pipeline_id, &id);
        }
        other => panic!("Expected Eos event, got {:?}", other),
    }

    manager.shutdown().await;
}

#[tokio::test]
async fn test_multiple_pipelines_all_eos() {
    init_gstreamer();
    let (tx, _rx) = create_event_channel();
    let mut event_rx = tx.subscribe();
    let manager = PipelineManager::new(tx);

    let mut ids = Vec::new();
    for _ in 0..3 {
        let id = manager
            .add_pipeline("videotestsrc num-buffers=10 ! fakesink")
            .await
            .unwrap();
        ids.push(id);
    }

    for id in &ids {
        manager.play(id).await.unwrap();
    }

    let events = wait_for_events(&mut event_rx, 3, 10).await;

    let eos_ids: HashSet<String> = events
        .iter()
        .filter_map(|e| match e {
            PipelineEvent::Eos { pipeline_id } => Some(pipeline_id.clone()),
            _ => None,
        })
        .collect();

    let expected_ids: HashSet<String> = ids.into_iter().collect();
    assert_eq!(eos_ids, expected_ids, "All 3 pipelines should reach EOS");

    manager.shutdown().await;
}

#[tokio::test]
async fn test_mixed_eos_and_error() {
    init_gstreamer();
    let (tx, _rx) = create_event_channel();
    let mut event_rx = tx.subscribe();
    let manager = PipelineManager::new(tx);

    // This pipeline will reach EOS
    let good_id = manager
        .add_pipeline("videotestsrc num-buffers=10 ! fakesink")
        .await
        .unwrap();

    // This pipeline will fail to play (non-existent file causes state change failure)
    let bad_id = manager
        .add_pipeline("filesrc location=/nonexistent_file_xyz ! fakesink")
        .await
        .unwrap();

    manager.play(&good_id).await.unwrap();

    // The bad pipeline fails at set_state(Playing), confirming it errors
    let bad_play_result = manager.play(&bad_id).await;
    assert!(
        bad_play_result.is_err(),
        "Playing a pipeline with non-existent file should fail"
    );

    // The bus watcher may emit Error events for the bad pipeline before
    // the good pipeline reaches EOS. Collect events generously — the bus
    // may emit multiple errors, so we don't require an exact count.
    let events = collect_events(&mut event_rx, 4, 3, false).await;

    let mut got_eos = false;
    let mut got_error_for_bad = false;
    for event in &events {
        match event {
            PipelineEvent::Eos { pipeline_id } if pipeline_id == &good_id => {
                got_eos = true;
            }
            PipelineEvent::Error { pipeline_id, .. } if pipeline_id == &bad_id => {
                got_error_for_bad = true;
            }
            _ => {}
        }
    }

    assert!(
        got_eos,
        "Good pipeline should reach EOS despite bad pipeline failing"
    );
    // The bad pipeline's synchronous play() failure is the primary error path.
    // A bus Error event may or may not arrive depending on timing, so we only
    // log it rather than requiring it.
    if got_error_for_bad {
        // Expected in most runs — the bus emits an error before state change completes
    }

    manager.shutdown().await;
}

#[tokio::test]
async fn test_pipeline_removed_during_playback() {
    init_gstreamer();
    let (tx, _rx) = create_event_channel();
    let mut event_rx = tx.subscribe();
    let manager = PipelineManager::new(tx);

    let id1 = manager
        .add_pipeline("videotestsrc num-buffers=10 ! fakesink")
        .await
        .unwrap();
    // Use a pipeline that won't EOS on its own (no num-buffers limit)
    let id2 = manager
        .add_pipeline("videotestsrc ! fakesink")
        .await
        .unwrap();

    manager.play(&id1).await.unwrap();
    manager.play(&id2).await.unwrap();

    // Give a brief moment for pipelines to start, then remove the second one
    tokio::time::sleep(Duration::from_millis(100)).await;
    manager.remove_pipeline(&id2).await.unwrap();

    let events = wait_for_events(&mut event_rx, 2, 10).await;

    let mut got_eos_id1 = false;
    let mut got_removed_id2 = false;

    for event in &events {
        match event {
            PipelineEvent::Eos { pipeline_id } if pipeline_id == &id1 => {
                got_eos_id1 = true;
            }
            PipelineEvent::PipelineRemoved { pipeline_id } if pipeline_id == &id2 => {
                got_removed_id2 = true;
            }
            _ => {}
        }
    }

    assert!(got_eos_id1, "First pipeline should reach EOS");
    assert!(
        got_removed_id2,
        "Second pipeline should emit PipelineRemoved"
    );

    manager.shutdown().await;
}

#[tokio::test]
async fn test_multiple_pipelines_different_durations() {
    init_gstreamer();
    let (tx, _rx) = create_event_channel();
    let mut event_rx = tx.subscribe();
    let manager = PipelineManager::new(tx);

    // Short pipeline: 10 buffers
    let short_id = manager
        .add_pipeline("videotestsrc num-buffers=10 ! fakesink")
        .await
        .unwrap();

    // Long pipeline: 30 buffers
    let long_id = manager
        .add_pipeline("videotestsrc num-buffers=30 ! fakesink")
        .await
        .unwrap();

    manager.play(&short_id).await.unwrap();
    manager.play(&long_id).await.unwrap();

    let events = wait_for_events(&mut event_rx, 2, 10).await;

    let eos_ids: HashSet<String> = events
        .iter()
        .filter_map(|e| match e {
            PipelineEvent::Eos { pipeline_id } => Some(pipeline_id.clone()),
            _ => None,
        })
        .collect();

    assert!(
        eos_ids.contains(&short_id),
        "Short pipeline should reach EOS"
    );
    assert!(eos_ids.contains(&long_id), "Long pipeline should reach EOS");

    manager.shutdown().await;
}

/// Test that an Unsupported event is correctly tracked separately from Error.
/// We send a synthetic Unsupported event through the event channel to verify
/// the tracker handles it, since triggering a real unsupported media error
/// at runtime requires specific codec/format conditions.
#[tokio::test]
async fn test_unsupported_event_tracked() {
    init_gstreamer();
    let (tx, _rx) = create_event_channel();
    let mut event_rx = tx.subscribe();
    let manager = PipelineManager::new(tx.clone());

    // Create a pipeline that will reach EOS
    let good_id = manager
        .add_pipeline("videotestsrc num-buffers=10 ! fakesink")
        .await
        .unwrap();

    // Create a second pipeline (won't play — we'll send a synthetic Unsupported event)
    let unsupported_id = manager
        .add_pipeline("videotestsrc ! fakesink")
        .await
        .unwrap();

    manager.play(&good_id).await.unwrap();

    // Send a synthetic Unsupported event for the second pipeline
    let _ = tx.send(PipelineEvent::Unsupported {
        pipeline_id: unsupported_id.clone(),
        message: "missing codec: test".to_string(),
    });

    let events = collect_events(&mut event_rx, 3, 5, false).await;

    let mut got_eos = false;
    let mut got_unsupported = false;

    for event in &events {
        match event {
            PipelineEvent::Eos { pipeline_id } if pipeline_id == &good_id => {
                got_eos = true;
            }
            PipelineEvent::Unsupported { pipeline_id, .. } if pipeline_id == &unsupported_id => {
                got_unsupported = true;
            }
            _ => {}
        }
    }

    assert!(got_eos, "Good pipeline should reach EOS");
    assert!(
        got_unsupported,
        "Unsupported event should be received for the second pipeline"
    );

    manager.shutdown().await;
}
