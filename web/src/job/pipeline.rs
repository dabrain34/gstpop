use std::path::Path;

use super::types::{DemucsOptions, OutputFormat, TranscodeOptions};

/// Build a GStreamer pipeline description for transcoding
pub fn build_transcode_pipeline(
    input_path: &Path,
    output_path: &Path,
    options: &TranscodeOptions,
) -> String {
    let input = escape_path(input_path);
    let output = escape_path(output_path);

    match options.output_format {
        OutputFormat::Mp4 => build_mp4_pipeline(&input, &output, options),
        OutputFormat::Webm => build_webm_pipeline(&input, &output, options),
        OutputFormat::Mkv => build_mkv_pipeline(&input, &output, options),
        OutputFormat::Mp3 => build_mp3_pipeline(&input, &output, options),
        OutputFormat::Ogg => build_ogg_pipeline(&input, &output, options),
        OutputFormat::Flac => build_flac_pipeline(&input, &output, options),
    }
}

/// Build a GStreamer pipeline description for demucs source separation
///
/// The demucs element outputs multiple audio streams (one per stem).
/// Each stem is written to a separate file in the output directory.
pub fn build_demucs_pipeline(
    input_path: &Path,
    output_dir: &Path,
    options: &DemucsOptions,
) -> String {
    let input = escape_path(input_path);
    let output_dir_str = escape_path(output_dir);

    // Get the stems to extract
    let stems = get_stems_for_pipeline(options);

    // Determine output format extension
    let ext = if options.output_format == "flac" {
        "flac"
    } else {
        "wav"
    };

    // Build the encoder based on format
    let encoder = if options.output_format == "flac" {
        "flacenc"
    } else {
        "wavenc"
    };

    // Build pipeline with demucs element
    // The demucs element has multiple src pads: src_vocals, src_drums, src_bass, src_other, etc.
    let mut pipeline = format!(
        "filesrc location=\"{input}\" ! decodebin ! audioconvert ! audioresample ! \
         audio/x-raw,format=F32LE,rate=44100,channels=2 ! \
         demucs model={model} name=demucs",
        model = options.model.as_str()
    );

    // Add output branch for each stem
    for stem in &stems {
        let output_file = format!("{}/{}.{}", output_dir_str, stem, ext);
        pipeline.push_str(&format!(
            " demucs.src_{stem} ! queue ! audioconvert ! {encoder} ! filesink location=\"{output_file}\""
        ));
    }

    // If we're only extracting specific stems and model produces more,
    // we need to handle unused pads (connect to fakesink)
    let all_model_stems = options.model.stems();
    for stem in all_model_stems {
        if !stems.contains(&stem.to_string()) {
            pipeline.push_str(&format!(" demucs.src_{stem} ! fakesink"));
        }
    }

    pipeline
}

/// Get the stems to extract based on options
fn get_stems_for_pipeline(options: &DemucsOptions) -> Vec<String> {
    if options.stems.is_empty() {
        // Extract all stems for this model
        options
            .model
            .stems()
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        // Filter to only requested stems that the model supports
        let model_stems: Vec<&str> = options.model.stems().to_vec();
        options
            .stems
            .iter()
            .filter(|s| model_stems.contains(&s.as_str()))
            .cloned()
            .collect()
    }
}

/// Get the expected output stem files for a demucs job
pub fn get_demucs_output_files(
    output_dir: &Path,
    options: &DemucsOptions,
) -> Vec<(String, std::path::PathBuf)> {
    let stems = get_stems_for_pipeline(options);
    let ext = if options.output_format == "flac" {
        "flac"
    } else {
        "wav"
    };

    stems
        .into_iter()
        .map(|stem| {
            let path = output_dir.join(format!("{}.{}", stem, ext));
            (stem, path)
        })
        .collect()
}

fn escape_path(path: &Path) -> String {
    // Escape special characters for GStreamer pipeline strings
    // Strip null bytes to prevent injection via filename
    path.to_string_lossy()
        .replace('\0', "")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn build_mp4_pipeline(input: &str, output: &str, options: &TranscodeOptions) -> String {
    let video_enc = build_x264_encoder(options);
    let audio_enc = build_aac_encoder(options);

    format!(
        "filesrc location=\"{input}\" ! decodebin name=dec \
         dec. ! queue ! videoconvert ! {video_enc} ! h264parse ! queue ! mux. \
         dec. ! queue ! audioconvert ! audioresample ! {audio_enc} ! queue ! mux. \
         mp4mux name=mux ! filesink location=\"{output}\""
    )
}

fn build_webm_pipeline(input: &str, output: &str, options: &TranscodeOptions) -> String {
    let video_bitrate = options.video_bitrate_kbps.unwrap_or(2000);
    let audio_bitrate = options.audio_bitrate_kbps.unwrap_or(128);

    format!(
        "filesrc location=\"{input}\" ! decodebin name=dec \
         dec. ! queue ! videoconvert ! vp8enc target-bitrate={video_bps} deadline=1 ! queue ! mux. \
         dec. ! queue ! audioconvert ! audioresample ! vorbisenc bitrate={audio_bps} ! queue ! mux. \
         webmmux name=mux ! filesink location=\"{output}\"",
        video_bps = video_bitrate * 1000,
        audio_bps = audio_bitrate * 1000
    )
}

fn build_mkv_pipeline(input: &str, output: &str, options: &TranscodeOptions) -> String {
    let video_enc = build_x264_encoder(options);
    let audio_enc = build_aac_encoder(options);

    format!(
        "filesrc location=\"{input}\" ! decodebin name=dec \
         dec. ! queue ! videoconvert ! {video_enc} ! h264parse ! queue ! mux. \
         dec. ! queue ! audioconvert ! audioresample ! {audio_enc} ! queue ! mux. \
         matroskamux name=mux ! filesink location=\"{output}\""
    )
}

fn build_mp3_pipeline(input: &str, output: &str, options: &TranscodeOptions) -> String {
    let bitrate = options.audio_bitrate_kbps.unwrap_or(192);

    format!(
        "filesrc location=\"{input}\" ! decodebin ! audioconvert ! audioresample ! \
         lamemp3enc target=bitrate bitrate={bitrate} ! \
         id3v2mux ! filesink location=\"{output}\""
    )
}

fn build_ogg_pipeline(input: &str, output: &str, options: &TranscodeOptions) -> String {
    let bitrate = options.audio_bitrate_kbps.unwrap_or(128);

    format!(
        "filesrc location=\"{input}\" ! decodebin ! audioconvert ! audioresample ! \
         vorbisenc bitrate={bitrate_bps} ! \
         oggmux ! filesink location=\"{output}\"",
        bitrate_bps = bitrate * 1000
    )
}

fn build_flac_pipeline(input: &str, output: &str, _options: &TranscodeOptions) -> String {
    format!(
        "filesrc location=\"{input}\" ! decodebin ! audioconvert ! audioresample ! \
         flacenc ! filesink location=\"{output}\""
    )
}

fn build_x264_encoder(options: &TranscodeOptions) -> String {
    let mut parts = vec!["x264enc tune=zerolatency".to_string()];

    if let Some(bitrate) = options.video_bitrate_kbps {
        parts.push(format!("bitrate={}", bitrate));
    }

    // Add video scaling if dimensions specified
    let scale = build_video_scale(options);

    format!("{}{}", scale, parts.join(" "))
}

fn build_aac_encoder(options: &TranscodeOptions) -> String {
    let bitrate = options.audio_bitrate_kbps.unwrap_or(128);
    // Use fdkaacenc if available, fallback to avenc_aac
    format!("fdkaacenc bitrate={}", bitrate * 1000)
}

fn build_video_scale(options: &TranscodeOptions) -> String {
    match (options.width, options.height) {
        (Some(w), Some(h)) => format!("videoscale ! video/x-raw,width={},height={} ! ", w, h),
        (Some(w), None) => format!("videoscale ! video/x-raw,width={} ! ", w),
        (None, Some(h)) => format!("videoscale ! video/x-raw,height={} ! ", h),
        (None, None) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::DemucsModel;
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_mp4_pipeline() {
        let input = PathBuf::from("/tmp/input.mkv");
        let output = PathBuf::from("/tmp/output.mp4");
        let options = TranscodeOptions::default();

        let pipeline = build_transcode_pipeline(&input, &output, &options);

        assert!(pipeline.contains("filesrc"));
        assert!(pipeline.contains("decodebin"));
        assert!(pipeline.contains("x264enc"));
        assert!(pipeline.contains("mp4mux"));
        assert!(pipeline.contains("filesink"));
    }

    #[test]
    fn test_mp3_pipeline() {
        let input = PathBuf::from("/tmp/input.wav");
        let output = PathBuf::from("/tmp/output.mp3");
        let options = TranscodeOptions {
            output_format: OutputFormat::Mp3,
            audio_bitrate_kbps: Some(320),
            ..Default::default()
        };

        let pipeline = build_transcode_pipeline(&input, &output, &options);

        assert!(pipeline.contains("lamemp3enc"));
        assert!(pipeline.contains("bitrate=320"));
    }

    #[test]
    fn test_demucs_pipeline_all_stems() {
        let input = PathBuf::from("/tmp/input.mp3");
        let output_dir = PathBuf::from("/tmp/output");
        let options = DemucsOptions::default();

        let pipeline = build_demucs_pipeline(&input, &output_dir, &options);

        assert!(pipeline.contains("demucs"));
        assert!(pipeline.contains("model=htdemucs"));
        assert!(pipeline.contains("src_vocals"));
        assert!(pipeline.contains("src_drums"));
        assert!(pipeline.contains("src_bass"));
        assert!(pipeline.contains("src_other"));
        assert!(pipeline.contains("wavenc"));
    }

    #[test]
    fn test_demucs_pipeline_specific_stems() {
        let input = PathBuf::from("/tmp/input.mp3");
        let output_dir = PathBuf::from("/tmp/output");
        let options = DemucsOptions {
            model: DemucsModel::HtDemucs,
            stems: vec!["vocals".to_string(), "drums".to_string()],
            output_format: "flac".to_string(),
        };

        let pipeline = build_demucs_pipeline(&input, &output_dir, &options);

        assert!(pipeline.contains("demucs"));
        assert!(pipeline.contains("src_vocals"));
        assert!(pipeline.contains("src_drums"));
        assert!(pipeline.contains("flacenc"));
        // Unused stems should go to fakesink
        assert!(pipeline.contains("src_bass ! fakesink"));
        assert!(pipeline.contains("src_other ! fakesink"));
    }

    #[test]
    fn test_get_demucs_output_files() {
        let output_dir = PathBuf::from("/tmp/output");
        let options = DemucsOptions {
            model: DemucsModel::HtDemucs,
            stems: vec!["vocals".to_string()],
            output_format: "wav".to_string(),
        };

        let files = get_demucs_output_files(&output_dir, &options);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "vocals");
        assert_eq!(files[0].1, PathBuf::from("/tmp/output/vocals.wav"));
    }
}
