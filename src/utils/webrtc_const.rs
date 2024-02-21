pub const SAMPLE_RATE: u32 = 48000;
pub const CHANNELS: u16 = 2;
pub const PAYLOAD_TYPE: u8 = 111;
pub const ENCODE_BUFFER_SIZE: usize = 960;
pub const TRACK_ID: &str = "audio";
pub const STREAM_TRACK_ID: &str = "webrtc-rs";
pub const STUN_ADRESS: &str = "stun:stun.l.google.com:19302";

// Error Tracker parameters
//SENDER
pub const READ_TRACK_THRESHOLD: u32 = 500;
pub const READ_TRACK_LIMIT: u32 = 1000;
//RECEIVER
pub const SEND_TRACK_THRESHOLD: u32 = 500;
pub const SEND_TRACK_LIMIT: u32 = 1000;