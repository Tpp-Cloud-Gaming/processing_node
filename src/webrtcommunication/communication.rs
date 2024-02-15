use crate::utils::common_utils::must_read_stdin;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::{APIBuilder, API};
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};

use crate::utils::webrtc_const::{CHANNELS, PAYLOAD_TYPE, SAMPLE_RATE};

/// Represents the WebRtc connection with other peer
///
/// Allows as to configure the different stages to establish the connection
pub struct Communication {
    ///
    peer_connection: Arc<RTCPeerConnection>,
}
impl Communication {
    /// Create new Comunication, needs a correct stun server adress to work
    pub async fn new(stun_adress: String) -> Result<Self, Error> {
        let api = create_api()?;

        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec![stun_adress.to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // Create a new RTCPeerConnection
        let peer_connection = Arc::new(if let Ok(val) = api.new_peer_connection(config).await {
            val
        } else {
            return Err(Error::new(
                ErrorKind::Other,
                "Error creating peer connection",
            ));
        });

        Ok(Self { peer_connection })
    }
    /// Waits to recibe an sdp string offer to setting the pc remote description
    pub async fn set_sdp(&self) -> Result<(), Error> {
        println!("Paste the SDP offer from the remote peer");
        let line = must_read_stdin()?;
        let desc_data = decode(line.as_str())?;
        let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;
        // Set the remote SessionDescription
        if self
            .peer_connection
            .set_remote_description(offer)
            .await
            .is_err()
        {
            return Err(Error::new(
                ErrorKind::Other,
                "Error setting remote description",
            ));
        };
        Ok(())
    }

    pub fn get_peer(&self) -> Arc<RTCPeerConnection> {
        self.peer_connection.clone()
    }
}

fn create_api() -> Result<API, Error> {
    let mut m = MediaEngine::default();
    if let Err(_val) = m.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                clock_rate: SAMPLE_RATE,
                channels: CHANNELS,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: vec![],
            },
            payload_type: PAYLOAD_TYPE,
            ..Default::default()
        },
        RTPCodecType::Audio,
    ) {
        return Err(Error::new(ErrorKind::Other, "Error registering codec"));
    }

    let mut registry = Registry::new();

    // Use the default set of Interceptors
    if let Ok(val) = register_default_interceptors(registry, &mut m) {
        registry = val;
    } else {
        return Err(Error::new(
            ErrorKind::Other,
            "Error registering default interceptors",
        ));
    }

    // Create the API object with the MediaEngine
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();
    Ok(api)
}


/// Decode a base64 string
/// # Arguments
/// * `s` - &str that represents the base64 string
/// # Returns
/// * Result<String, Error> - The decoded string
fn decode(s: &str) -> Result<String, Error> {
    let b = match BASE64_STANDARD.decode(s) {
        Ok(b) => b,
        Err(_) => return Err(Error::new(ErrorKind::Other, "Error decoding base64")),
    };

    match String::from_utf8(b) {
        Ok(s) => Ok(s),
        Err(_) => Err(Error::new(ErrorKind::Other, "Error decoding utf8")),
    }
}

fn encode(b: &str) -> String {
    BASE64_STANDARD.encode(b)
}
