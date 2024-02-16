use crate::audio::audio_utils::search_device;

use cpal::{traits::DeviceTrait, Device, SampleFormat, Stream};
use std::{
    io::Error,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::Receiver;

/// Struct to play audio samples
pub struct AudioPlayer {
    /// Receiver of the audio samples
    rx: Arc<Mutex<Receiver<f32>>>,
    /// Audio device
    device: Device,
    /// Audio device config
    config: cpal::StreamConfig,
    /// Audio sample format
    sample_format: SampleFormat,
}

impl AudioPlayer {
    /// Returns new instance of AudioPlayer.
    /// # Arguments
    /// * `device` - An optional string that represents the device name
    /// * `rx` - Arc<Mutex<Receiver<f32>>> that represents the receiver of the audio samples
    pub fn new(device: Option<String>, rx: Arc<Mutex<Receiver<f32>>>) -> Result<Self, Error> {
        let device = search_device(device)?;
        let config = device
            .default_output_config()
            .map_err(|e| Error::new(std::io::ErrorKind::Other, e))?;
        let sample_format = config.sample_format();
        let config: cpal::StreamConfig = config.into();
        Ok(Self {
            rx,
            device,
            config,
            sample_format,
        })
    }
    /// Start the audio player
    /// # Returns
    /// * `Stream` - The audio stream  
    pub fn start(&self) -> Result<Stream, Error> {
        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
        let rx_clone = self.rx.clone();
        let stream = match self.sample_format {
            SampleFormat::F32 => self.device.build_output_stream(
                &self.config,
                move |data: &mut [f32], _: &_| write_data(data, rx_clone.clone()),
                err_fn,
                None,
            ),
            SampleFormat::I16 => self.device.build_output_stream(
                &self.config,
                move |data: &mut [f32], _: &_| write_data(data, rx_clone.clone()),
                err_fn,
                None,
            ),
            SampleFormat::U16 => self.device.build_output_stream(
                &self.config,
                move |data: &mut [f32], _: &_| write_data(data, rx_clone.clone()),
                err_fn,
                None,
            ),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        };

        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => return Err(Error::new(std::io::ErrorKind::Other, e)),
        };

        Ok(stream)
    }
}

/// Write the audio data to the output
/// # Arguments
/// * `output` - &mut [f32] that represents the output audio samples
/// * `rx` - Arc<Mutex<Receiver<f32>>> that represents the receiver of the audio samples
fn write_data(output: &mut [f32], rx: Arc<Mutex<Receiver<f32>>>) {
    for sample in output {
        let mut rx_lock = match rx.lock() {
            Ok(r) => r,
            Err(e) => {
                log::error!("Error locking the receiver: {:?}", e);
                return;
            }
        };

        let data = rx_lock.try_recv().unwrap_or(0.0);
        *sample = data;
    }
}
