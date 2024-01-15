use anyhow::Result;
use audio::audio_decoder::AudioDecoder;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use webrtc::ice_transport::ice_gatherer_state::RTCIceGathererState;
use webrtc::ice_transport::ice_gathering_state::RTCIceGatheringState;
use webrtc::peer_connection::RTCPeerConnection;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::Duration;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::track::track_remote::TrackRemote;
use dotenv::dotenv;

pub fn must_read_stdin() -> Result<String> {
    let mut line = String::new();

    std::io::stdin().read_line(&mut line)?;
    line = line.trim().to_owned();
    println!();

    Ok(line)
}

pub fn decode(s: &str) -> Result<String> {
    let b = BASE64_STANDARD.decode(s)?;

    //if COMPRESS {
    //    b = unzip(b)
    //}

    let s = String::from_utf8(b)?;
    Ok(s)
}

pub fn encode(b: &str) -> String {
    //if COMPRESS {
    //    b = zip(b)
    //}

    BASE64_STANDARD.encode(b)
}

async fn read_track(track: Arc<TrackRemote>, notify: Arc<Notify>, mut decoder: AudioDecoder) -> Result<()> {
    loop {
        tokio::select! {
            result = track.read_rtp() => {
                if let Ok((rtp_packet, _)) = result {
                    println!("LLego LUCAS PAQUETA");

                    decoder.decode(rtp_packet.payload.to_vec());
                
                    // let mut w = writer.lock().await;
                    // w.write_rtp(&rtp_packet)?;
                }else{
                    println!("Error leyendo paquete");
                    //println!("file closing begin after read_rtp error");
                    //let mut w = writer.lock().await;
                    // if let Err(err) = w.close() {
                    //     println!("file close err: {err}");
                    // }
                    // println!("file closing end after read_rtp error");
                    return Ok(());
                }
            }
            _ = notify.notified() => {
                println!("file closing begin after notified");
                // let mut w = writer.lock().await;
                // if let Err(err) = w.close() {
                //     println!("file close err: {err}");
                // }
                // println!("file closing end after notified");
                return Ok(());
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();


    // Create a MediaEngine object to configure the supported codec
    let mut m = MediaEngine::default();

    // Setup the codecs you want to use.
    // We'll use a VP8/VP9 and Opus but you can also define your own
    // m.register_codec(
    //     RTCRtpCodecParameters {
    //         capability: RTCRtpCodecCapability {
    //             mime_type: if is_vp9 {
    //                 MIME_TYPE_VP9.to_owned()
    //             } else {
    //                 MIME_TYPE_VP8.to_owned()
    //             },
    //             clock_rate: 90000,
    //             channels: 0,
    //             sdp_fmtp_line: "".to_owned(),
    //             rtcp_feedback: vec![],
    //         },
    //         payload_type: if is_vp9 { 98 } else { 96 },
    //         ..Default::default()
    //     },
    //     RTPCodecType::Video,
    // )?;

    m.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                clock_rate: 48000,
                channels: 2,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: vec![],
            },
            payload_type: 111,
            ..Default::default()
        },
        RTPCodecType::Audio,
    )?;

    // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
    // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
    // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
    // for each PeerConnection.
    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut m)?;

    // Create the API object with the MediaEngine
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    // Prepare the configuration
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    // Create a new RTCPeerConnection
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    let notify_tx = Arc::new(Notify::new());
    let notify_rx = notify_tx.clone();
        // Set a handler for when a new remote track starts, this handler saves buffers to disk as
    // an ivf file, since we could have multiple video tracks we provide a counter.
    // In your application this is where you would handle/process video
    let pc = Arc::downgrade(&peer_connection);
    peer_connection.on_track(Box::new(move |track, _, _| {
        // Send a PLI on an interval so that the publisher is pushing a keyframe every rtcpPLIInterval
        println!("Trackmania");
        let media_ssrc = track.ssrc();
        let pc2 = pc.clone();
        tokio::spawn(async move {
            let mut result = Result::<usize>::Ok(0);
            while result.is_ok() {
                let timeout = tokio::time::sleep(Duration::from_secs(3));
                tokio::pin!(timeout);

                tokio::select! {
                    _ = timeout.as_mut() =>{
                        if let Some(pc) = pc2.upgrade(){
                            result = pc.write_rtcp(&[Box::new(PictureLossIndication{
                                sender_ssrc: 0,
                                media_ssrc,
                            })]).await.map_err(Into::into);
                        }else{
                            break;
                        }
                    }
                };
            }
        });

        let notify_rx2 = Arc::clone(&notify_rx);
        // let ivf_writer2 = Arc::clone(&ivf_writer);
        // let ogg_writer2 = Arc::clone(&ogg_writer);
        const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/recorded.wav");
        let decoder = AudioDecoder::new(PATH).unwrap();
        Box::pin(async move {
            let codec = track.codec();
            let mime_type = codec.capability.mime_type.to_lowercase();
            if mime_type == MIME_TYPE_OPUS.to_lowercase() {
                println!("Got Opus track, saving to disk as output.opus (48 kHz, 2 channels)");

                tokio::spawn(async move {
                    let _ = read_track(track, notify_rx2, decoder).await;
                });
            }
            // else if mime_type == MIME_TYPE_VP8.to_lowercase()
            //     || mime_type == MIME_TYPE_VP9.to_lowercase()
            // {
            //     println!(
            //         "Got {} track, saving to disk as output.ivf",
            //         if is_vp9 { "VP9" } else { "VP8" }
            //     );
            //     tokio::spawn(async move {
            //         let _ = save_to_disk(ivf_writer2, track, notify_rx2).await;
            //     });
            // }
        })
    }));


    // Allow us to receive 1 audio track, and 1 video track
    peer_connection
        .add_transceiver_from_kind(RTPCodecType::Audio, None)
        .await?;



    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    
    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection.on_ice_connection_state_change(Box::new(
        move |connection_state: RTCIceConnectionState| {
            println!("Connection State has changed {connection_state}");

            if connection_state == RTCIceConnectionState::Connected {
                println!("Ctrl+C the remote client to stop the demo");
            } else if connection_state == RTCIceConnectionState::Failed {
                notify_tx.notify_waiters();

                println!("Done writing media files");

                let _ = done_tx.try_send(());
            }
            Box::pin(async {})
        },
    ));
    
    // Set the remote SessionDescription: ACA METER USER INPUT Y PEGAR EL SDP
    // Wait for the offer to be pasted
    println!("Paste the SDP offer from the remote peer");
    let line = must_read_stdin()?;
    let desc_data = decode(line.as_str())?;
    let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // Set the remote SessionDescription
    peer_connection.set_remote_description(offer).await?;

    // Create an answer
    let answer = peer_connection.create_answer(None).await?;

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    peer_connection.set_local_description(answer).await?;

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    let _ = gather_complete.recv().await;

    // Output the answer in base64 so we can paste it in browser
    if let Some(local_desc) = peer_connection.local_description().await {
        // IMPRIMIR SDP EN BASE64
        let json_str = serde_json::to_string(&local_desc)?;
        let b64 = BASE64_STANDARD.encode(&json_str);
        println!("{b64}");
    } else {
        println!("generate local_description failed!");
    }


    // // Create an answer to send to the other process
    // let answer = peer_connection.create_offer(None).await?;

    
    // // Create channel that is blocked until ICE Gathering is complete
    // let mut gather_complete = peer_connection.gathering_complete_promise().await;


    // // Sets the LocalDescription, and starts our UDP listeners
    // peer_connection.set_local_description(answer).await?;

    // let _ = gather_complete.recv().await;


    // // Sets the LocalDescription, and starts our UDP listeners
    // //peer_connection.set_local_description(answer).await?;

    // if let Some(local_desc) = peer_connection.local_description().await {
    //     let json_str = serde_json::to_string(&local_desc)?;
    //     let b64 = BASE64_STANDARD.encode(&json_str);
    //     println!("{b64}");
    // } else {
    //     println!("generate local_description failed!");
    // }

    // println!("Paste the SDP offer from the remote peer:");
    //  // Wait for the offer to be pasted
    // let line = must_read_stdin()?;
    // let desc_data = decode(line.as_str())?;
    // let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // // Set the remote SessionDescription
    // peer_connection.set_remote_description(offer).await?;



    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            println!("received done signal!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };

    peer_connection.close().await?;

    Ok(())
}
