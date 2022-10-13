use libp2p::{
    core::muxing::StreamMuxerBox, core::transport::Boxed, core::transport::MemoryTransport,
    core::upgrade, dns, mplex, noise, tcp, websocket, yamux, PeerId, Transport,
};
use std::{io, result::Result};

/// Creates the Transport mechanism that describes how peers communicate.
/// Currently, mostly an inlined form of `libp2p::tokio_development_transport`.
pub(crate) fn build_transport(
    keypair: &libp2p::identity::Keypair,
) -> Result<Boxed<(PeerId, StreamMuxerBox)>, io::Error> {
    trace!("Build Tcp transport");
    let transport = {
        let dns_tcp = dns::TokioDnsConfig::system(tcp::TokioTcpTransport::new(
            tcp::GenTcpConfig::new().nodelay(true),
        ))?;
        let ws_dns_tcp = websocket::WsConfig::new(dns::TokioDnsConfig::system(
            tcp::TokioTcpTransport::new(tcp::GenTcpConfig::new().nodelay(true)),
        )?);
        dns_tcp.or_transport(ws_dns_tcp)
    };

    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&keypair)
        .expect("Noise key generation failed.");

    Ok(transport
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(upgrade::SelectUpgrade::new(
            yamux::YamuxConfig::default(),
            mplex::MplexConfig::default(),
        ))
        .timeout(std::time::Duration::from_secs(20))
        .boxed())
}

pub(crate) fn build_memory_transport(
    keypair: &libp2p::identity::Keypair,
) -> Result<Boxed<(PeerId, StreamMuxerBox)>, io::Error> {
    trace!("Build memory transport");
    let auth = {
        // The version released after 0.48.0 will have `NoiseAuthenticated`.
        // Use commented line below after upgrading.
        // noise::NoiseAuthenticated::xx(&keypair).unwrap()
        let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
            .into_authentic(&keypair)
            .expect("Noise key generation failed.");
        noise::NoiseConfig::xx(noise_keys).into_authenticated()
    };

    Ok(MemoryTransport::default()
        .upgrade(upgrade::Version::V1)
        .authenticate(auth)
        .multiplex(yamux::YamuxConfig::default())
        .boxed())
}
