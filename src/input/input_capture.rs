use std::io::{Error, ErrorKind};
use std::sync::Arc;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;
use winput::message_loop::EventReceiver;
use winput::Action;
use winput::{message_loop, Button};

use super::input_const::{KEYBOARD_CHANNEL_LABEL, MOUSE_CHANNEL_LABEL};
use crate::output::output_const::*;
use crate::utils::shutdown;

pub struct InputCapture {
    shutdown: shutdown::Shutdown,
    button_channel: Arc<RTCDataChannel>,
    mouse_channel: Arc<RTCDataChannel>,
}

impl InputCapture {
    pub async fn new(
        pc: Arc<RTCPeerConnection>,
        shutdown: shutdown::Shutdown,
    ) -> Result<InputCapture, Error> {
        let button_channel: Arc<RTCDataChannel> =
            match pc.create_data_channel(KEYBOARD_CHANNEL_LABEL, None).await {
                Ok(ch) => ch,
                Err(_) => {
                    return Err(Error::new(
                        ErrorKind::Other,
                        "Error creating latency data channel",
                    ))
                }
            };
        let mouse_channel: Arc<RTCDataChannel> =
            match pc.create_data_channel(MOUSE_CHANNEL_LABEL, None).await {
                Ok(ch) => ch,
                Err(_) => {
                    return Err(Error::new(
                        ErrorKind::Other,
                        "Error creating latency data channel",
                    ))
                }
            };

        Ok(InputCapture {
            shutdown,
            button_channel,
            mouse_channel,
        })
    }

    pub async fn start(&self) -> Result<(), Error> {
        self.shutdown.add_task().await;

        println!("Starting");
        let receiver: EventReceiver = match message_loop::start() {
            Ok(receiver) => receiver,
            Err(_e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Error setting local description",
                ))
            }
        };

        tokio::select! {
            _ = self.shutdown.wait_for_error() => {
                message_loop::stop();
            }
            _ = start_handler(receiver, self.button_channel.clone(), self.mouse_channel.clone(),self.shutdown.clone()) => {
                message_loop::stop();
                println!("Stopped");

            }
        }
        return Ok(());
    }
}

async fn start_handler(
    receiver: EventReceiver,
    button_channel: Arc<RTCDataChannel>,
    mouse_channel: Arc<RTCDataChannel>,
    shutdown: shutdown::Shutdown,
) {
    let shutdown_cpy = shutdown.clone();
    loop {
        let shutdown_cpy_loop = shutdown_cpy.clone();

        tokio::task::spawn(async move {});

        match receiver.next_event() {
            message_loop::Event::Keyboard {
                vk,
                action: Action::Press,
                scan_code,
            } => {
                let button_channel_cpy = button_channel.clone();
                //     tokio::task::spawn(async move {
                let mut key = "".to_string();
                if scan_code == 42 {
                    key = "160".to_string();
                } else if scan_code == 54 {
                    key = "161".to_string();
                } else {
                    key = vk.into_u8().to_string();
                }
                handle_button_action(
                    button_channel_cpy,
                    PRESS_KEYBOARD_ACTION,
                    key,
                    shutdown_cpy_loop.clone(),
                )
                .await
                .unwrap();
                //       });
            }
            message_loop::Event::Keyboard {
                vk,
                action: Action::Release,
                scan_code,
            } => {
                let button_channel_cpy = button_channel.clone();
                let mut key = "".to_string();
                if scan_code == 42 {
                    key = "160".to_string();
                } else if scan_code == 54 {
                    key = "161".to_string();
                } else {
                    key = vk.into_u8().to_string();
                }
                //   tokio::task::spawn(async move {
                handle_button_action(
                    button_channel_cpy,
                    RELEASE_KEYBOARD_ACTION,
                    key,
                    shutdown_cpy_loop.clone(),
                )
                .await
                .unwrap();
                //    });
            }
            message_loop::Event::MouseButton {
                action: Action::Press,
                button,
            } => {
                let button_channel_cpy = button_channel.clone();
                //  tokio::task::spawn(async move {
                handle_button_action(
                    button_channel_cpy,
                    PRESS_MOUSE_ACTION,
                    button_to_i32(button).to_string(),
                    shutdown_cpy_loop.clone(),
                )
                .await
                .unwrap();
                //  });
            }
            message_loop::Event::MouseButton {
                action: Action::Release,
                button,
            } => {
                let button_channel_cpy = button_channel.clone();
                //  tokio::task::spawn(async move {
                handle_button_action(
                    button_channel_cpy,
                    RELEASE_MOUSE_ACTION,
                    button_to_i32(button).to_string(),
                    shutdown_cpy_loop.clone(),
                )
                .await
                .unwrap();
                //});
            }
            message_loop::Event::MouseMoveRelative { x, y } => {
                if x == 0 && y == 0 {
                    continue;
                }
                if mouse_channel.ready_state()
                    == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
                {
                    let mouse_channel_cpy = mouse_channel.clone();
                    tokio::task::spawn(async move {
                        mouse_channel_cpy
                            .send_text(std::format!("{} {}", x, y).as_str())
                            .await
                            .unwrap();
                    });
                }
            }
            _ => (),
        }

        if shutdown.check_for_error().await {
            break;
        };
    }
}

async fn handle_button_action(
    button_channel: Arc<RTCDataChannel>,
    action: &str,
    text: String,
    shutdown: shutdown::Shutdown,
) -> Result<(), Error> {
    if button_channel.ready_state()
        == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
    {
        if let Err(_e) = button_channel
            .send_text(std::format!("{}{}", action, text).as_str())
            .await
        {
            shutdown.notify_error(false).await;
            return Err(Error::new(
                ErrorKind::Other,
                "Error sending message through data channel",
            ));
        };
    }
    return Ok(());
}

fn button_to_i32(button: Button) -> i32 {
    match button {
        Button::Left => 0,
        Button::Right => 1,
        Button::Middle => 2,
        Button::X1 => 3,
        Button::X2 => 4,
    }
}
