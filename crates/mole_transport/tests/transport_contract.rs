use mole_core::{Frame, PlayerInput};
use mole_transport::{InputPacket, LoopbackTransport, Transport};

#[test]
fn loopback_transport_delivers_input_packets() {
    let mut transport = LoopbackTransport::default();
    let packet = InputPacket {
        frame: Frame(9),
        player_index: 0,
        input: PlayerInput::neutral().with_attack(true),
    };

    transport.send(packet);

    assert_eq!(transport.try_recv(), Some(packet));
}

#[test]
fn loopback_transport_returns_none_when_empty() {
    let mut transport = LoopbackTransport::default();

    assert_eq!(transport.try_recv(), None);
}
