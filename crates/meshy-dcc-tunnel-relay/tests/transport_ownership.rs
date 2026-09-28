use meshy_dcc_tunnel_protocol::FrameIoError;

fn accepts_canonical(_: Option<FrameIoError>) {}

#[test]
#[allow(deprecated)]
fn historical_transport_errors_alias_the_protocol_owner() {
    accepts_canonical(None::<meshy_dcc_tunnel_agent::transport::TransportError>);
    accepts_canonical(None::<meshy_dcc_tunnel_relay::transport::TransportError>);
}
