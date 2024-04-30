use std::io::{Error, ErrorKind};
use std::sync::{mpsc, Arc};

use crate::input::input_capture::InputCapture;
use crate::video::video_player::start_video_player;

use crate::utils::error_tracker::ErrorTracker;
use crate::utils::shutdown;
use crate::utils::webrtc_const::{READ_TRACK_LIMIT, READ_TRACK_THRESHOLD};
use webrtc::api::media_engine::MIME_TYPE_H264;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::{
    api::media_engine::MIME_TYPE_OPUS, rtp_transceiver::rtp_codec::RTPCodecType,
    track::track_remote::TrackRemote,
};

use crate::utils::latency_const::LATENCY_CHANNEL_LABEL;
use crate::utils::shutdown::Shutdown;
use crate::utils::webrtc_const::STUN_ADRESS;
use crate::webrtcommunication::communication::{encode, Communication};
use crate::webrtcommunication::latency::Latency;
use crate::websocketprotocol::websocketprotocol::WsProtocol;

pub struct ReceiverSide {}

impl ReceiverSide {
    pub async fn new(client_name: &str, offerer_name: &str, game_name: &str) -> Result<(), Error> {
        // Initialize Log:
        let mut ws: WsProtocol = WsProtocol::ws_protocol().await?;
        ws.init_client(client_name, offerer_name, game_name).await?;

        env_logger::builder().format_target(false).init();
        let shutdown = Shutdown::new();

        let comunication = Communication::new(STUN_ADRESS.to_owned()).await?;

        let peer_connection = comunication.get_peer();

        // Start mosue and keyboard capture
        let shutdown_cpy = shutdown.clone();
        let pc_cpy = peer_connection.clone();
        //TODO: Retornar errores ?
        let shutdown_cpy1 = shutdown.clone();

        tokio::spawn(async move {
            match InputCapture::new(pc_cpy, shutdown_cpy).await {
                Ok(input_capture) => match input_capture.start().await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Failed to start InputCapture: {}", e);
                        shutdown_cpy1.notify_error(false).await;
                    }
                },
                Err(e) => {
                    log::error!("Failed to create InputCapture: {}", e);
                    shutdown_cpy1.notify_error(false).await;
                }
            }
        });

        // Create video frame channels
        let (tx_video, rx_video): (mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>) =
            mpsc::channel();
        let shutdown_player = shutdown.clone();
        tokio::spawn(async move {
            start_video_player(rx_video, shutdown_player).await;
        });

        let (tx_audio, rx_audio): (mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>) =
            mpsc::channel();
        let shutdown_audio = shutdown.clone();
        tokio::spawn(async move {
            crate::sound::audio_player::start_audio_player(rx_audio, shutdown_audio).await;
        });

        // Set a handler for when a new remote track starts, this handler saves buffers to disk as
        // an ivf file, since we could have multiple video tracks we provide a counter.
        // In your application this is where you would handle/process video
        set_on_track_handler(&peer_connection, tx_audio, tx_video, shutdown.clone());

        channel_handler(&peer_connection, shutdown.clone());

        // Allow us to receive 1 audio track
        if peer_connection
            .add_transceiver_from_kind(RTPCodecType::Audio, None)
            .await
            .is_err()
        {
            return Err(Error::new(
                ErrorKind::Other,
                "Error adding audio transceiver",
            ));
        }

        //set_on_ice_connection_state_change_handler(&peer_connection, shutdown.clone());

        // Set the remote SessionDescription: ACA METER USER INPUT Y PEGAR EL SDP
        // Wait for the offer to be pasted

        let sdp = ws.wait_for_offerer_sdp().await?;
        comunication.set_sdp(sdp).await?;
        let peer_connection = comunication.get_peer();

        // Create an answer
        let answer = match peer_connection.create_answer(None).await {
            Ok(answer) => answer,
            Err(_) => return Err(Error::new(ErrorKind::Other, "Error creating answer")),
        };

        // Create channel that is blocked until ICE Gathering is complete
        let mut gather_complete = peer_connection.gathering_complete_promise().await;

        // Sets the LocalDescription, and starts our UDP listeners
        if peer_connection.set_local_description(answer).await.is_err() {
            return Err(Error::new(
                ErrorKind::Other,
                "Error setting local description",
            ));
        }

        // Block until ICE Gathering is complete, disabling trickle ICE
        // we do this because we only can exchange one signaling message
        // in a production application you should exchange ICE Candidates via OnICECandidate
        let _ = gather_complete.recv().await;

        // Output the answer in base64 so we can paste it in browser
        if let Some(local_desc) = peer_connection.local_description().await {
            // IMPRIMIR SDP EN BASE64
            let json_str = serde_json::to_string(&local_desc)?;
            let b64 = encode(&json_str);
            ws.send_sdp_to_offerer(offerer_name, &b64).await?;
            println!("{b64}");
        } else {
            log::error!("RECEIVER | Generate local_description failed!");
        }

        println!("Press ctrl-c to stop");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("RECEIVER | ctrl-c signal");
                println!();
            }
            _ = shutdown.wait_for_shutdown() => {
                log::info!("RECEIVER | Error notifier signal");
            }
        };

        if peer_connection.close().await.is_err() {
            return Err(Error::new(
                ErrorKind::Other,
                "Error closing peer connection",
            ));
        }

        shutdown.shutdown();

        Ok(())
    }
}

/// Sets on track event for the provided connection
///
/// # Arguments
///
/// * `peer_connection` - A RTCPeerConnection.
/// * `tx_audio` - A channel to configure in case it is an audio track.
/// * `tx_audio` - A channel to configure in case it is a video track.
/// * `shutdown` -  Used for graceful shutdown.
fn set_on_track_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    tx_audio: mpsc::Sender<Vec<u8>>,
    tx_video: mpsc::Sender<Vec<u8>>,
    shutdown: shutdown::Shutdown,
) {
    peer_connection.on_track(Box::new(move |track, _, _| {
        let codec = track.codec();
        let mime_type = codec.capability.mime_type.to_lowercase();

        // Check if is a audio track
        if mime_type == MIME_TYPE_OPUS.to_lowercase() {
            let tx_audio_cpy = tx_audio.clone();
            let shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                println!("RECEIVER | Got OPUS Track");
                tokio::spawn(async move {
                    let _ = read_audio_track(track, &tx_audio_cpy, shutdown_cpy).await;
                });
            });
        };

        // Check if is a audio track
        if mime_type == MIME_TYPE_H264.to_lowercase() {
            let tx_video_cpy = tx_video.clone();
            let shutdown_cpy = shutdown.clone();
            return Box::pin(async move {
                println!("RECEIVER | Got H264 Track");
                tokio::spawn(async move {
                    let _ = read_video_track(track, &tx_video_cpy, shutdown_cpy).await;
                });
            });
        };

        Box::pin(async {})
    }));
}

/// Reads RTP Packets on the provided audio track and sends them to the channel provided
///
/// # Arguments
///
/// * `track` - Audio track from which to read rtp packets
/// * `tx` - A channel to send the packets read
/// * `shutdown` -  Used for graceful shutdown.
///
/// # Return
/// Result containing `Ok(())` on success. Error on error.
async fn read_audio_track(
    track: Arc<TrackRemote>,
    tx: &mpsc::Sender<Vec<u8>>,
    shutdown: shutdown::Shutdown,
) -> Result<(), Error> {
    let mut error_tracker = ErrorTracker::new(READ_TRACK_THRESHOLD, READ_TRACK_LIMIT);
    shutdown.add_task().await;

    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    let value = rtp_packet.payload.to_vec();
                    match tx.send(value){
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("RECEIVER | Error sending audio packet to channel: {e}");
                            shutdown.notify_error(false).await;
                            return Err(Error::new(ErrorKind::Other, "Error sending audio packet to channel"));
                        }
                    }

                }else if error_tracker.increment_with_error(){
                        log::error!("RECEIVER | Max Attemps | Error reading RTP packet");
                        shutdown.notify_error(false).await;
                        return Err(Error::new(ErrorKind::Other, "Error reading RTP packet"));
                }else{
                        log::warn!("RECEIVER | Error reading RTP packet");
                };

            }
            _ = tokio::signal::ctrl_c() => {
                return Ok(());
            }
            _= shutdown.wait_for_error() => {
                println!("Se cerro el read track");
                return Ok(());
            }
        }
    }
}

/// Reads data on the provided audio track and sends it to the channel provided
///
/// # Arguments
///
/// * `track` - Video track from which to read data
/// * `tx` - A channel to send the data read
/// * `shutdown` -  Used for graceful shutdown.
///
/// # Return
/// Result containing `Ok(())` on success. Error on error.
async fn read_video_track(
    track: Arc<TrackRemote>,
    tx: &mpsc::Sender<Vec<u8>>,
    shutdown: shutdown::Shutdown,
) -> Result<(), Error> {
    let mut error_tracker = ErrorTracker::new(READ_TRACK_THRESHOLD, READ_TRACK_LIMIT);
    shutdown.add_task().await;

    loop {
        let mut buff: [u8; 1400] = [0; 1400];
        tokio::select! {

            result = track.read(&mut buff) => {
                if let Ok((_rtp_packet, _)) = result {

                    match tx.send(buff.to_vec()){
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("RECEIVER | Error sending video packet to channel: {e}");
                            shutdown.notify_error(false).await;
                            return Err(Error::new(ErrorKind::Other, "Error sending video packet to channel"));
                        }

                    };

                }else if error_tracker.increment_with_error(){
                        log::error!("RECEIVER | Max Attemps | Error reading RTP packet");
                        shutdown.notify_error(false).await;
                        return Err(Error::new(ErrorKind::Other, "Error reading RTP packet"));
                }else{
                        log::warn!("RECEIVER | Error reading RTP packet");
                };

            }
            _ = tokio::signal::ctrl_c() => {
                return Ok(());
            }
            _= shutdown.wait_for_error() => {
                println!("Se cerro el read track");
                return Ok(());
            }
        }
    }
}

/// Sets on data channel event for the given connection
///
/// # Arguments
///
/// * `peer_conection` - A RTCPeerConnection
/// * `shutdown` -  Used for graceful shutdown.
fn channel_handler(peer_connection: &Arc<RTCPeerConnection>, shutdown: shutdown::Shutdown) {
    // Register data channel creation handling
    peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
        let d_label = d.label().to_owned();

        if d_label == LATENCY_CHANNEL_LABEL {
            let shutdown_cpy = shutdown.clone();
            Box::pin(async move {
                // Start the latency measurement
                if let Err(e) = Latency::start_latency_receiver(d).await {
                    log::error!("RECEIVER | Error starting latency receiver: {e}");
                    shutdown_cpy.notify_error(false).await;
                }
            })
        } else {
            Box::pin(async move {
                log::info!("RECEIVER |New DataChannel has been opened | {d_label}");
            })
        }
    }));
}

//Esta funcion solo sirve para que detecte si algun on ice pasa a connection state failed y ahi
// mande un signal para que todo termine

// Set the handler for ICE connection state
// This will notify you when the peer has connected/disconnected
// fn set_on_ice_connection_state_change_handler(
//     peer_connection: &Arc<RTCPeerConnection>,
//     _shutdown: shutdown::Shutdown,
// ) {
//     peer_connection.on_ice_connection_state_change(Box::new(
//         move |connection_state: RTCIceConnectionState| {
//             log::info!("RECEIVER | ICE Connection State has changed | {connection_state}");

//             // if connection_state == RTCIceConnectionState::Connected {
//             //     //let shutdown_cpy = shutdown.clone();
//             // } else if connection_state == RTCIceConnectionState::Failed {
//             //     TODO: ver que hacer en este escenario
//             //     let shutdown_cpy = shutdown.clone();
//             //     _ = Box::pin(async move {
//             //         shutdown_cpy.notify_error(true).await;
//             //     });
//             // }
//             Box::pin(async {})
//         },
//     ));
// }