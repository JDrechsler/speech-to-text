use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use rubato::{FftFixedIn, Resampler};
use serde::Serialize;

pub const SAMPLE_RATE: u32 = 16_000;
const LEVEL_WINDOW: usize = 800;
const RESAMPLER_CHUNK: usize = 1024;
pub const SYSTEM_DEFAULT: &str = "default";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDevice {
    pub name: String,
    pub is_built_in: bool,
}

pub struct Recorder {
    stop: Sender<()>,
    worker: JoinHandle<Result<f64, String>>,
    pub path: PathBuf,
}

impl Recorder {
    pub fn start(
        path: PathBuf,
        microphone: Option<String>,
        on_level: impl Fn(f32) + Send + 'static,
    ) -> Result<Recorder, String> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop, stop_rx) = mpsc::channel();
        let target = path.clone();
        let worker = thread::spawn(move || record(&target, microphone, on_level, ready_tx, stop_rx));
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Recorder { stop, worker, path }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => Err("The recorder stopped unexpectedly".into()),
        }
    }

    pub fn stop(self) -> Result<f64, String> {
        let _ = self.stop.send(());
        self.worker
            .join()
            .map_err(|_| "The recorder crashed".to_string())?
    }
}

pub fn input_devices() -> Vec<InputDevice> {
    input_device_names()
        .into_iter()
        .map(|name| InputDevice { is_built_in: is_built_in(&name), name })
        .collect()
}

#[cfg(target_os = "macos")]
fn input_device_names() -> Vec<String> {
    crate::audio_devices::input_device_names()
}

#[cfg(not(target_os = "macos"))]
fn input_device_names() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|devices| devices.filter_map(|device| device.name().ok()).collect())
        .unwrap_or_default()
}

pub fn default_device_name() -> Option<String> {
    cpal::default_host().default_input_device()?.name().ok()
}

fn is_built_in(name: &str) -> bool {
    name.contains("Built-in") || (name.starts_with("MacBook") && name.ends_with("Microphone"))
}

pub fn resolve_device(preference: Option<&str>) -> Option<Device> {
    let wanted = preference.unwrap_or(if cfg!(target_os = "macos") { "builtin" } else { SYSTEM_DEFAULT });
    find_preferred(wanted).or_else(|| cpal::default_host().default_input_device())
}

pub fn resolved_device_name(preference: Option<&str>) -> Option<String> {
    resolve_device(preference).and_then(|device| device.name().ok())
}

pub fn unavailable_preference_notice(preference: Option<&str>) -> Option<String> {
    let wanted = preference.filter(|wanted| *wanted != SYSTEM_DEFAULT)?;
    if find_preferred(wanted).is_some() {
        return None;
    }
    let fallback = default_device_name().unwrap_or_else(|| "system default".into());
    Some(format!("Preferred microphone unavailable, using {fallback}"))
}

fn find_preferred(wanted: &str) -> Option<Device> {
    if wanted == SYSTEM_DEFAULT {
        return None;
    }
    let name = input_device_names()
        .into_iter()
        .find(|name| if wanted == "builtin" { is_built_in(name) } else { name == wanted })?;
    cpal::default_host()
        .devices()
        .ok()?
        .find(|device| device.name().is_ok_and(|candidate| candidate == name))
}

fn record(
    path: &Path,
    microphone: Option<String>,
    on_level: impl Fn(f32),
    ready: Sender<Result<(), String>>,
    stop: Receiver<()>,
) -> Result<f64, String> {
    let opened = open_stream(microphone.as_deref());
    let (stream, samples, input_rate) = match opened {
        Ok(opened) => opened,
        Err(error) => {
            let _ = ready.send(Err(error.clone()));
            return Err(error);
        }
    };
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut sink = match hound::WavWriter::create(path, spec) {
        Ok(writer) => Sink { writer, frames: 0, meter: LevelMeter::default() },
        Err(error) => {
            let message = format!("Could not create the recording file: {error}");
            let _ = ready.send(Err(message.clone()));
            return Err(message);
        }
    };
    let _ = ready.send(Ok(()));

    let mut converter = Converter::new(input_rate)?;
    loop {
        match stop.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }
        match samples.recv_timeout(Duration::from_millis(20)) {
            Ok(chunk) => sink.write(&converter.push(&chunk), &on_level)?,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(stream);
    for chunk in samples.try_iter() {
        sink.write(&converter.push(&chunk), &on_level)?;
    }
    sink.write(&converter.finish(), &on_level)?;
    sink.finish()
}

struct Sink {
    writer: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
    frames: u64,
    meter: LevelMeter,
}

impl Sink {
    fn write(&mut self, chunk: &[f32], on_level: &impl Fn(f32)) -> Result<(), String> {
        for &sample in chunk {
            let clamped = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            self.writer.write_sample(clamped).map_err(|error| error.to_string())?;
        }
        self.frames += chunk.len() as u64;
        self.meter.feed(chunk, on_level);
        Ok(())
    }

    fn finish(self) -> Result<f64, String> {
        self.writer.finalize().map_err(|error| error.to_string())?;
        Ok(self.frames as f64 / SAMPLE_RATE as f64)
    }
}

fn open_stream(microphone: Option<&str>) -> Result<(Stream, Receiver<Vec<f32>>, u32), String> {
    let device = resolve_device(microphone).ok_or("No microphone found")?;
    let supported = device
        .default_input_config()
        .map_err(|error| format!("The microphone is not available: {error}"))?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let channels = config.channels as usize;
    let rate = config.sample_rate.0;
    let (sender, receiver) = mpsc::channel::<Vec<f32>>();
    let on_error = |error| eprintln!("microphone stream error: {error}");

    let stream = match format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                let _ = sender.send(mono(data.iter().copied(), channels));
            },
            on_error,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _: &_| {
                let _ = sender.send(mono(data.iter().map(|&sample| sample as f32 / 32_768.0), channels));
            },
            on_error,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _: &_| {
                let _ = sender.send(mono(
                    data.iter().map(|&sample| (sample as f32 - 32_768.0) / 32_768.0),
                    channels,
                ));
            },
            on_error,
            None,
        ),
        other => return Err(format!("Unsupported microphone sample format {other}")),
    }
    .map_err(|error| format!("Could not open the microphone: {error}"))?;
    stream
        .play()
        .map_err(|error| format!("Could not start the microphone: {error}"))?;
    Ok((stream, receiver, rate))
}

fn mono(samples: impl Iterator<Item = f32>, channels: usize) -> Vec<f32> {
    let interleaved: Vec<f32> = samples.collect();
    if channels <= 1 {
        return interleaved;
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

struct Converter {
    resampler: Option<FftFixedIn<f32>>,
    pending: Vec<f32>,
}

impl Converter {
    fn new(input_rate: u32) -> Result<Converter, String> {
        let resampler = if input_rate == SAMPLE_RATE {
            None
        } else {
            Some(
                FftFixedIn::new(input_rate as usize, SAMPLE_RATE as usize, RESAMPLER_CHUNK, 2, 1)
                    .map_err(|error| format!("Could not convert the microphone audio: {error}"))?,
            )
        };
        Ok(Converter { resampler, pending: Vec::new() })
    }

    fn push(&mut self, input: &[f32]) -> Vec<f32> {
        let Some(resampler) = self.resampler.as_mut() else {
            return input.to_vec();
        };
        self.pending.extend_from_slice(input);
        let mut output = Vec::new();
        while self.pending.len() >= resampler.input_frames_next() {
            let needed = resampler.input_frames_next();
            let chunk: Vec<f32> = self.pending.drain(..needed).collect();
            if let Ok(mut resampled) = resampler.process(&[chunk], None) {
                output.append(&mut resampled[0]);
            }
        }
        output
    }

    fn finish(&mut self) -> Vec<f32> {
        let Some(resampler) = self.resampler.as_mut() else {
            return Vec::new();
        };
        let rest = std::mem::take(&mut self.pending);
        resampler
            .process_partial(Some(&[rest]), None)
            .map(|mut resampled| resampled.remove(0))
            .unwrap_or_default()
    }
}

#[derive(Default)]
struct LevelMeter {
    window: Vec<f32>,
}

impl LevelMeter {
    fn feed(&mut self, samples: &[f32], on_level: &impl Fn(f32)) {
        self.window.extend_from_slice(samples);
        while self.window.len() >= LEVEL_WINDOW {
            let energy: f32 = self.window[..LEVEL_WINDOW].iter().map(|sample| sample * sample).sum();
            let rms = (energy / LEVEL_WINDOW as f32).sqrt();
            on_level((rms * 8.0).min(1.0));
            self.window.drain(..LEVEL_WINDOW);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_is_averaged_to_mono() {
        assert_eq!(mono([0.5, -0.5, 1.0, 0.0].into_iter(), 2), vec![0.0, 0.5]);
    }

    #[test]
    fn resampling_48k_gives_a_third_of_the_samples() {
        let mut converter = Converter::new(48_000).unwrap();
        let mut total = converter.push(&vec![0.0; 48_000]).len();
        total += converter.finish().len();
        assert!((15_000..=17_000).contains(&total), "got {total}");
    }

    #[test]
    fn one_level_per_50_ms() {
        let levels = std::cell::RefCell::new(Vec::new());
        let mut meter = LevelMeter::default();
        meter.feed(&vec![0.1; 16_000], &|level| levels.borrow_mut().push(level));
        assert_eq!(levels.borrow().len(), 20);
    }
}
