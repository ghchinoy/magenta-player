//! CPAL audio output: device setup, the real-time lock-free resampler, and
//! the `--record` WAV capture path.

use crate::ffi::ffi;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Stream;
use std::sync::Arc;

/// Opens the default output device, preferring a native 48 kHz stereo stream
/// (MRT2's native rate), and builds an output stream that pulls stereo samples
/// from the runner. If the device can't do 48 kHz (e.g. Sonos/Bluetooth locked
/// to 44.1 kHz), a real-time, lock-free, boundary-safe linear resampler is used
/// so playback stays at the correct pitch/speed.
///
/// Returns the started-elsewhere `Stream` (caller must `.play()`) and a human
/// readable "N channels, R Hz" string for display.
pub fn build_output_stream(
    runner: &Arc<cxx::UniquePtr<ffi::RealtimeRunnerBridge>>,
) -> (Stream, String) {
    println!("Opening default audio output device...");
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("❌ Error: No audio output device available");

    // Query supported configurations to request exactly 48000 Hz stereo
    let supported_configs_range = device
        .supported_output_configs()
        .expect("❌ Error: Failed to query supported audio configurations");

    let config_format = supported_configs_range
        .filter(|c| c.channels() == 2)
        .find(|c| c.min_sample_rate().0 <= 48000 && c.max_sample_rate().0 >= 48000)
        .map(|c| c.with_sample_rate(cpal::SampleRate(48000)))
        .unwrap_or_else(|| {
            let default_config = device
                .default_output_config()
                .expect("❌ Error: Failed to get default audio output configuration");
            println!("\n[WARNING] 48kHz stereo output not directly supported by this audio device (e.g. Sonos/Bluetooth/AirPlay).");
            println!("          Falling back to default format ({} channels, {} Hz).", default_config.channels(), default_config.sample_rate().0);
            println!("          The MRT2 engine runs internally at exactly 48000 Hz; a real-time");
            println!("          resampler will convert to the device rate to preserve pitch/speed.");
            println!("          -> TIP: For pristine sound, use built-in MBP speakers and set them to 48,000 Hz in Audio MIDI Setup!\n");
            default_config
        });

    let audio_format_line = format!(
        "{} channels, {} Hz",
        config_format.channels(),
        config_format.sample_rate().0
    );
    println!("Audio Format:  {}", audio_format_line);

    // Audio Resampler Setup:
    // MRT2 generates frames strictly at 48000 Hz. If the hardware device runs at a
    // different rate (e.g. 44100 Hz on Sonos Roam or Bluetooth), we perform real-time
    // lock-free linear resampling to prevent flat pitch shifts and playback speed drops.
    let device_sample_rate = config_format.sample_rate().0 as f64;
    let ratio = 48000.0 / device_sample_rate;
    let is_resampling = (device_sample_rate - 48000.0).abs() > 5.0; // allow small tolerance

    // Resampler boundary states captured mutably by the audio thread closure
    let mut src_accum = 0.0f64;
    let mut last_sample_l = 0.0f32;
    let mut last_sample_r = 0.0f32;

    let runner_clone = Arc::clone(runner);
    let stream = device
        .build_output_stream(
            &config_format.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let num_frames = data.len() / 2;

                if !is_resampling {
                    // Fast native path (no resampling overhead)
                    let mut left = vec![0.0f32; num_frames];
                    let mut right = vec![0.0f32; num_frames];
                    runner_clone.read_audio_stereo(&mut left, &mut right);
                    for (i, frame) in data.chunks_exact_mut(2).enumerate() {
                        frame[0] = left[i];
                        frame[1] = right[i];
                    }
                } else {
                    // High-fidelity, lock-free real-time linear resampler.
                    // Boundary-safe: keeps memory of the very last sample of the prior
                    // block to interpolate cleanly across buffer boundaries, preventing
                    // clicks and pops.
                    let next_src_pos = src_accum + num_frames as f64 * ratio;
                    let consumed_inputs = next_src_pos.floor() as usize;

                    let mut left = vec![0.0f32; consumed_inputs];
                    let mut right = vec![0.0f32; consumed_inputs];

                    if consumed_inputs > 0 {
                        runner_clone.read_audio_stereo(&mut left, &mut right);
                    }

                    // Input buffers of size consumed_inputs + 1 to hold the saved last sample at index 0
                    let mut input_l = vec![0.0f32; consumed_inputs + 1];
                    let mut input_r = vec![0.0f32; consumed_inputs + 1];

                    input_l[0] = last_sample_l;
                    input_r[0] = last_sample_r;

                    if consumed_inputs > 0 {
                        input_l[1..=consumed_inputs].copy_from_slice(&left);
                        input_r[1..=consumed_inputs].copy_from_slice(&right);

                        last_sample_l = left[consumed_inputs - 1];
                        last_sample_r = right[consumed_inputs - 1];
                    }

                    // Resample and interleave directly into the hardware output buffer
                    for i in 0..num_frames {
                        let src_pos = src_accum + i as f64 * ratio;
                        let idx = src_pos.floor() as usize;
                        let frac = (src_pos - idx as f64) as f32;

                        let out_l = input_l[idx] * (1.0 - frac) + input_l[idx + 1] * frac;
                        let out_r = input_r[idx] * (1.0 - frac) + input_r[idx + 1] * frac;

                        data[i * 2] = out_l;
                        data[i * 2 + 1] = out_r;
                    }

                    // Store fractional accumulator offset for the next callback block
                    src_accum = next_src_pos - consumed_inputs as f64;
                }
            },
            |err| eprintln!("❌ Audio stream error: {}", err),
            None,
        )
        .expect("❌ Error: Failed to build CPAL audio output stream");

    (stream, audio_format_line)
}

/// Records `seconds` of audio from the engine's internal recording buffer and
/// writes a 16-bit PCM stereo WAV into `output_dir`. Pulls at the engine's
/// native 48 kHz regardless of the live output device rate.
pub fn record_to_wav(
    runner: &Arc<cxx::UniquePtr<ffi::RealtimeRunnerBridge>>,
    output_dir: &str,
    seconds: u64,
) {
    std::fs::create_dir_all(output_dir).unwrap_or_else(|e| {
        eprintln!("❌ Error: Failed to create output directory {}: {}", output_dir, e);
        std::process::exit(1);
    });

    println!("\n[INFO] Recording {} seconds...", seconds);
    runner.start_recording();
    std::thread::sleep(std::time::Duration::from_secs(seconds));
    runner.stop_recording();

    let sample_count = runner.get_recorded_sample_count();
    if sample_count == 0 {
        eprintln!("❌ Error: No audio was captured (0 samples recorded).");
        std::process::exit(1);
    }

    let mut left = vec![0.0f32; sample_count];
    let mut right = vec![0.0f32; sample_count];
    runner.get_recorded_audio(0, &mut left, &mut right);

    let filename = format!(
        "recording-{}.wav",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let out_path = std::path::Path::new(output_dir).join(&filename);

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&out_path, spec).unwrap_or_else(|e| {
        eprintln!("❌ Error: Failed to create WAV file {}: {}", out_path.display(), e);
        std::process::exit(1);
    });
    for i in 0..sample_count {
        let l_i16 = (left[i].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        let r_i16 = (right[i].clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(l_i16).ok();
        writer.write_sample(r_i16).ok();
    }
    writer.finalize().unwrap_or_else(|e| {
        eprintln!("❌ Error: Failed to finalize WAV file: {}", e);
        std::process::exit(1);
    });

    println!(
        "✓ Recorded {:.1}s ({} samples) to: {}",
        sample_count as f64 / 48000.0,
        sample_count,
        out_path.display()
    );
}
