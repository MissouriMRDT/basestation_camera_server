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
use tokio_tungstenite::WebSocketStream;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_VP8};
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocalWriter;

/// Holds configuration values read from config.toml
#[derive(serde::Deserialize, Clone)]
struct Config {
    websocket_address: String,
    camera_addresses: Vec<String>,
    #[serde(deserialize_with = "level_from_str")]
    log_level: log::LevelFilter,
}

fn level_from_str<'de, D>(deserializer: D) -> Result<log::LevelFilter, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    match s.as_str() {
        "Off" => Ok(log::LevelFilter::Off),
        "Error" => Ok(log::LevelFilter::Error),
        "Warn" => Ok(log::LevelFilter::Warn),
        "Info" => Ok(log::LevelFilter::Info),
        "Debug" => Ok(log::LevelFilter::Debug),
        "Trace" => Ok(log::LevelFilter::Trace),
        _ => Err(serde::de::Error::custom("invalid log::LevelFilter value")),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_file_path = std::env::current_exe()
        .expect("unable to obtain executable directory")
        .parent()
        .expect("unable to obtain executable directory")
        .join("config.toml");
    println!(
        "Loading configuration file \"{}\".",
        config_file_path.display()
    );
    let config_data = match std::fs::read_to_string(&config_file_path) {
        Ok(data) => data,
        Err(error) => {
            println!("Unable to read configuration file: {error}.\nInstalling default configuration file.");
            std::fs::write(&config_file_path, include_str!("default_config.toml"))
                .expect("Failed to write default configuration file");
            std::fs::read_to_string(&config_file_path)
                .expect("Unable to read from newly created configuration file")
        }
    };
    let config: Config =
        toml::from_str(&config_data).expect("Unable to deserialize configuration file");

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
        .filter(None, config.log_level)
        .init();

    // Create tracks that we send video back to browser on
    let video_tracks: Arc<Vec<Arc<TrackLocalStaticRTP>>> = Arc::new(
        config
            .camera_addresses
            .iter()
            .enumerate()
            .map(|(i, _)| {
                Arc::new(TrackLocalStaticRTP::new(
                    webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
                        mime_type: MIME_TYPE_VP8.to_owned(),
                        ..Default::default()
                    },
                    "video".to_owned(),
                    format!("video{i}").to_owned(),
                ))
            })
            .collect(),
    );

    // Open a UDP Listener for RTP Packets
    // Read RTP packets forever and send them to the WebRTC Client
    for (camera_address, video_track) in std::iter::zip(
        config.camera_addresses.iter().map(|a| a.clone()),
        video_tracks.iter().map(|v| v.clone()),
    ) {
        tokio::spawn(async move {
            info!("[RTP {camera_address}] Starting.");
            let listener = match UdpSocket::bind(&camera_address).await {
                Err(err) => {
                    error!("[RTP {camera_address}] Error binding to UDP socket: {err}.");
                    return;
                }
                Ok(listener) => listener,
            };
            let mut inbound_rtp_packet = vec![0u8; 1600]; // UDP MTU
            while let Ok((n, _)) = listener.recv_from(&mut inbound_rtp_packet).await {
                if let Err(err) = video_track.write(&inbound_rtp_packet[..n]).await {
                    error!("[RTP {camera_address}] Error writing to video track: {err}.");
                    return;
                }
            }
            info!("[RTP {camera_address}] Stopped.");
        });
    }

    info!("[WS] Starting on {}.", config.websocket_address);
    let listener = TcpListener::bind(&config.websocket_address)
        .await
        .with_context(|| format!("[WS] Failed to bind to {}", &config.websocket_address))?;
    tokio::select! {
        _ = async {loop {
            match listener.accept().await {
                Err(err) => warn!("[WS] Connection error: {err}."),
                Ok((stream, addr)) => {
                    info!("[WS {addr}] Connected.");
                    let video_tracks2 = video_tracks.clone();
                    tokio::spawn(async move {
                        match ws_handler(addr, stream, video_tracks2).await {
                            Err(err) => warn!("[WS {addr}] {err:?}."),
                            Ok(_) => info!("[WS {addr}] Disconnected."),
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

async fn ws_handler(
    addr: std::net::SocketAddr,
    stream: TcpStream,
    video_tracks: Arc<Vec<Arc<TrackLocalStaticRTP>>>,
) -> Result<()> {
    let mut ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .context("Websocket handshake failed")?;

    // Wait for a SDP to be received
    debug!("[WS {addr}] Waiting for remote session description.");
    let offer = read_offer(addr, &mut ws_stream)
        .await
        .context("Failed to read and decode remote session description")?;
    debug!("[WS {addr}] Received remote session description.");

    // Create a MediaEngine object to configure the supported codec
    let mut m = MediaEngine::default();

    m.register_default_codecs()
        .context("Failed to register default codecs")?;

    // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
    // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
    // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
    // for each PeerConnection.
    let registry = webrtc::interceptor::registry::Registry::new();

    // Use the default set of Interceptors
    let registry = register_default_interceptors(registry, &mut m)
        .context("Failed to register default interceptors")?;

    // Create the API object with the MediaEngine
    let api = webrtc::api::APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    // Prepare the configuration
    let config = webrtc::peer_connection::configuration::RTCConfiguration {
        ice_servers: vec![],
        ..Default::default()
    };

    // Create a new RTCPeerConnection
    debug!("[WS {addr}] Creating RTCPeerConnection.");
    let peer_connection = Arc::new(
        api.new_peer_connection(config)
            .await
            .context("Failed to create a new RTCPeerConnection")?,
    );

    // Add this newly created track to the PeerConnection
    let (running_tx, running_rx) = tokio::sync::watch::channel(true);
    for (i, video_track) in <Vec<Arc<TrackLocalStaticRTP>> as Clone>::clone(&video_tracks)
        .into_iter()
        .enumerate()
    {
        debug!("[WS {addr} TRACK {i}] Adding track to RTCPeerConnection.");
        let rtp_sender = peer_connection
            .add_track(video_track)
            .await
            .with_context(|| format!("Failed to add track {i} to RTCPeerConnection"))?;

        // Read incoming RTCP packets
        let mut running_rx2 = running_rx.clone();
        tokio::spawn(async move {
            debug!("[WS {addr} TRACK {i}] Starting RTCP listener.");
            let mut rtcp_buf = vec![0u8; 1500];
            loop {
                tokio::select! {
                    Err(err) = rtp_sender.read(&mut rtcp_buf) => {
                        warn!("[WS {addr} TRACK {i}] Error reading RTCP listener: {err}.");
                        break;
                    },
                    _ = running_rx2.wait_for(|running| *running == false) => {
                        debug!("[WS {addr} TRACK {i}] Received stop signal.");
                        break;
                    },
                }
            }
            debug!("[WS {addr} TRACK {i}] Stopping RTCP listener.");
            if let Err(err) = rtp_sender.stop().await {
                warn!("[WS {addr} TRACK {i}] Error stopping RTCP listener: {err}.");
            };
            debug!("[WS {addr} TRACK {i}] Stopped RTCP listener.");
            Result::<()>::Ok(())
        });
    }

    // Set the handler for ICE connection state
    peer_connection.on_ice_connection_state_change(Box::new(
        move |connection_state: webrtc::ice_transport::ice_connection_state::RTCIceConnectionState| {
            debug!("[WS {addr}] ICE connection state has changed: {connection_state}.");
            Box::pin(async {})
        },
    ));

    // Spawn closer
    let mut running_rx2 = running_rx.clone();
    let peer_connection2 = peer_connection.clone();
    tokio::spawn(async move {
        let _ = running_rx2.wait_for(|running| *running == false).await;
        debug!("[PC {addr}] Received stop signal.");
        if let Err(err) = peer_connection2.close().await {
            warn!("[PC {addr}] Error closing peer connection: {err}.");
        } else {
            debug!("[PC {addr}] Closed peer connection.");
        }
    });

    // Set the handler for Peer connection state
    peer_connection.on_peer_connection_state_change(Box::new(
        move |connection_state: RTCPeerConnectionState| {
            debug!("[WS {addr}] Peer connection state has changed: {connection_state}.");
            if connection_state == RTCPeerConnectionState::Disconnected {
                // Send stop signal
                debug!("[WS {addr}] Stopping RTCP listeners.");
                if let Err(err) = running_tx.send(false) {
                    warn!("[WS {addr}] Error sending stop signal: {err}.");
                }
            }
            Box::pin(async {})
        },
    ));

    // Set the remote SessionDescription
    debug!("[WS {addr}] Setting remote session description.");
    peer_connection
        .set_remote_description(offer)
        .await
        .context("Failed to set remote session description")?;

    // Create an answer
    debug!("[WS {addr}] Creating session description answer.");
    let answer = peer_connection
        .create_answer(None)
        .await
        .context("Failed to create session description answer")?;

    // Create channel that is blocked until ICE Gathering is complete
    debug!("[WS {addr}] Creating gathering waiter.");
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    debug!("[WS {addr}] Setting local session description.");
    peer_connection
        .set_local_description(answer)
        .await
        .context("Failed to set local SDP")?;

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    debug!("[WS {addr}] Waiting for gathering to complete.");
    let _ = gather_complete.recv().await;

    // Send local session description
    debug!("[WS {addr}] Sending local session description.");
    write_offer(addr, &mut ws_stream, &peer_connection)
        .await
        .context("Failed to send local session description")?;

    Ok(())
}

async fn read_offer(
    addr: std::net::SocketAddr,
    stream: &mut WebSocketStream<TcpStream>,
) -> Result<RTCSessionDescription> {
    // Read base 64
    let Some(next) = stream.next().await else {
        return Err(anyhow!("No message"));
    };
    let b64 = next?.into_data();

    // Decode into json
    let json = BASE64_STANDARD
        .decode(b64)
        .context("Failed to decode base 64")?;
    debug!(
        "[WS {addr}] Read offer {}",
        std::str::from_utf8(&json).unwrap_or_default()
    );

    // Deserialize into RTCSessionDescription
    Ok(serde_json::from_slice::<RTCSessionDescription>(&json)?)
}

async fn write_offer(
    addr: std::net::SocketAddr,
    stream: &mut WebSocketStream<TcpStream>,
    peer_connection: &Arc<webrtc::peer_connection::RTCPeerConnection>,
) -> Result<()> {
    // Serialize into json
    let local_description = peer_connection
        .local_description()
        .await
        .ok_or(anyhow::anyhow!("Failed to get local session description"))?;
    let json = serde_json::to_vec(&local_description).context("Failed to serialize to json")?;
    debug!(
        "[WS {addr}] Writing offer {}",
        std::str::from_utf8(&json).unwrap_or_default()
    );

    // Encode into base 64
    let b64 = BASE64_STANDARD.encode(&json);

    // Write base 64
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(b64.into()))
        .await
        .context("Failed to write local session description")?;
    Ok(())
}
