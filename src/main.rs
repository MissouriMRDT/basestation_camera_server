use std::io::Write;
use std::sync::Arc;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use log::debug;
use log::error;
use log::info;
use log::warn;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_VP8};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};

#[tokio::main]
async fn main() -> Result<()> {
    let debug = false;
    env_logger::Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{}:{} [{}] {} - {}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.level(),
                chrono::Local::now().format("%H:%M:%S.%6f"),
                record.args()
            )
        })
        .filter(
            None,
            if debug {
                log::LevelFilter::Trace
            } else {
                log::LevelFilter::Debug
            },
        )
        .init();

    // Create Track that we send video back to browser on.
    let video_track = Arc::new(TrackLocalStaticRTP::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_VP8.to_owned(),
            ..Default::default()
        },
        "video".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    // Open a UDP Listener for RTP Packets.
    const RTP_ADDRESS: &str = "127.0.0.1:5004";
    info!("Starting RTP server on {RTP_ADDRESS}.");
    let listener = UdpSocket::bind(RTP_ADDRESS).await?;

    // Read RTP packets forever and send them to the WebRTC Client.
    let video_track2 = video_track.clone();
    tokio::spawn(async move {
        let mut inbound_rtp_packet = vec![0u8; 1600]; // UDP MTU
        while let Ok((n, _)) = listener.recv_from(&mut inbound_rtp_packet).await {
            if let Err(err) = video_track2.write(&inbound_rtp_packet[..n]).await {
                error!("video_track write err: {err}");
                return;
            }
        }
    });
    debug!("Started RTP listener.");

    const WS_ADDRESS: &str = "0.0.0.0:8085";
    info!("Starting WebSocket SDP server on {WS_ADDRESS}.");
    let listener = TcpListener::bind(WS_ADDRESS)
        .await
        .expect("Failed to bind.");
    tokio::select! {
        _ = async {loop {
            match listener.accept().await {
                Err(err) => warn!("Websocket SDP server TCP connection error: {err}"),
                Ok((stream, addr)) => {
                    info!("Websocket SDP server accepted TCP connection from {addr}.");
                    let video_track3 = video_track.clone();
                    tokio::spawn(async move {
                        match ws_handler(stream, video_track3).await {
                            Err(err) => warn!("Handling {addr} failed with {err:?}"),
                            Ok(_) => info!("Handling {addr} finished."),
                        };
                    });
                }
            }
        }} => {}
        _ = tokio::signal::ctrl_c() => {
            info!("Shutting down.")
        },
    };

    Ok(())
}

async fn ws_handler(stream: TcpStream, video_track: Arc<TrackLocalStaticRTP>) -> Result<()> {
    let mut ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .context("Websocket handshake failed")?;

    // Wait for a SDP to be received.
    debug!("Waiting for remote session description.");
    let offer = read_offer(&mut ws_stream)
        .await
        .context("Failed to read and decode remote session description")?;
    debug!("Received remote session description.");

    // Create a MediaEngine object to configure the supported codec.
    let mut m = MediaEngine::default();

    m.register_default_codecs()
        .context("Failed to register default codecs")?;

    // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
    // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
    // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
    // for each PeerConnection.
    let registry = Registry::new();

    // Use the default set of Interceptors.
    let registry = register_default_interceptors(registry, &mut m)
        .context("Failed to register default interceptors")?;

    // Create the API object with the MediaEngine.
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    // Prepare the configuration.
    let config = RTCConfiguration {
        ice_servers: vec![],
        ..Default::default()
    };

    // Create a new RTCPeerConnection.
    let peer_connection = Arc::new(
        api.new_peer_connection(config)
            .await
            .context("Failed to create a new RTCPeerConnection")?,
    );

    // Add this newly created track to the PeerConnection.
    debug!("Adding track to RTCPeerConnection.");
    let rtp_sender = peer_connection
        .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
        .context("Failed to add track to RTCPeerConnection")?;

    // Read incoming RTCP packets.
    debug!("Starting RTCP listener.");
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        Result::<()>::Ok(())
    });

    // Set the handler for ICE connection state.
    peer_connection.on_ice_connection_state_change(Box::new(
        move |connection_state: RTCIceConnectionState| {
            debug!("ICE connection state has changed {connection_state}.");
            Box::pin(async {})
        },
    ));

    // Set the handler for Peer connection state.
    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        debug!("Peer connection state has changed: {s}.");
        Box::pin(async {})
    }));

    // Set the remote SessionDescription
    debug!("Setting remote session description.");
    peer_connection
        .set_remote_description(offer)
        .await
        .context("Failed to set remote session description")?;

    // Create an answer
    debug!("Creating session description answer.");
    let answer = peer_connection
        .create_answer(None)
        .await
        .context("Failed to create session description answer")?;

    // Create channel that is blocked until ICE Gathering is complete
    debug!("Creating gathering waiter.");
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    debug!("Setting local session description.");
    peer_connection
        .set_local_description(answer)
        .await
        .context("Failed to set local SDP")?;

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    debug!("Waiting for gathering to complete.");
    let _ = gather_complete.recv().await;

    // Send local session description.
    debug!("Sending local session description.");
    write_offer(&mut ws_stream, &peer_connection)
        .await
        .context("Failed to send local session description")?;

    Ok(())
}

async fn read_offer(stream: &mut WebSocketStream<TcpStream>) -> Result<RTCSessionDescription> {
    // Read base 64.
    let Some(next) = stream.next().await else {
        return Err(anyhow!("No message"));
    };
    let b64 = next?.into_data();

    // Decode into json.
    let json = BASE64_STANDARD
        .decode(b64)
        .context("Failed to decode base 64")?;
    debug!("{}", std::str::from_utf8(&json).unwrap_or_default());

    // Deserialize into RTCSessionDescription.
    Ok(serde_json::from_slice::<RTCSessionDescription>(&json)?)
}

async fn write_offer(
    stream: &mut WebSocketStream<TcpStream>,
    peer_connection: &Arc<RTCPeerConnection>,
) -> Result<()> {
    // Serialize into json.
    let local_description = peer_connection
        .local_description()
        .await
        .ok_or(anyhow::anyhow!("Failed to get local session description"))?;
    let json = serde_json::to_vec(&local_description).context("Failed to serialize to json")?;
    debug!("{}", std::str::from_utf8(&json).unwrap_or_default());

    // Encode into base 64.
    let b64 = BASE64_STANDARD.encode(&json);

    // Write base 64.
    stream
        .send(Message::Text(b64.into()))
        .await
        .context("Failed to write local session description")?;
    Ok(())
}
