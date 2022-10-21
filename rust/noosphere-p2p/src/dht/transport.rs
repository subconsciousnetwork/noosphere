use libp2p::{
    core::muxing::StreamMuxerBox, core::transport::Boxed, core::upgrade, dns, mplex, noise, tcp,
    yamux, PeerId, Transport,
};
use std::{io, result::Result};

/// Creates the Transport mechanism that describes how peers communicate.
/// Currently, mostly an inlined form of `libp2p::tokio_development_transport`.
pub(crate) fn build_transport(
    keypair: &libp2p::identity::Keypair,
) -> Result<Boxed<(PeerId, StreamMuxerBox)>, io::Error> {
    let transport = dns::TokioDnsConfig::system(tcp::TokioTcpTransport::new(
        tcp::GenTcpConfig::new().nodelay(true),
    ))?;

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
