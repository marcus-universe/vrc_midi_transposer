//! A running engine: custom controls triggered by MIDI send OSC and update the snapshot.

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use rosc::{OscPacket, OscType};
use transposer_engine::project::{MidiTrigger, Project, SendAddress, TriggerKind, TriggerMode, ValueType};
use transposer_engine::{ControlEvent, Engine, EngineCmd};

fn control(name: &str, addr: &str, ty: ValueType, trigger: TriggerKind) -> SendAddress {
    SendAddress {
        name: name.into(),
        addr: addr.into(),
        ty,
        default: 0.0,
        min: Some(0.0),
        max: Some(1.0),
        midi: Some(MidiTrigger {
            kind: trigger,
            channel: None,
            number: 20,
            mode: TriggerMode::Toggle,
            forward: false,
        }),
    }
}

fn next_message(socket: &UdpSocket) -> (String, OscType) {
    let mut buf = [0u8; 1024];
    let (n, _) = socket.recv_from(&mut buf).expect("an OSC message");
    match rosc::decoder::decode_udp(&buf[..n]).unwrap().1 {
        OscPacket::Message(m) => (m.addr, m.args[0].clone()),
        OscPacket::Bundle(_) => panic!("unexpected bundle"),
    }
}

#[test]
fn midi_triggered_controls_send_osc_and_reach_the_snapshot() {
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_read_timeout(Some(Duration::from_secs(3))).unwrap();

    let mut project = Project::named("test");
    project.osc.listen_port = 0;
    project.osc.target_port = receiver.local_addr().unwrap().port();
    project.osc.sending_addresses = vec![
        control(
            "Glasses",
            "/avatar/parameters/Glasses",
            ValueType::Bool,
            TriggerKind::Note,
        ),
        control("Hue", "/avatar/parameters/HUE", ValueType::Float, TriggerKind::Cc),
    ];
    let (engine, _events) = Engine::start(project);
    // Opening the project starts mapping generation 1.
    let generation = 1;

    let send = |event| engine.send(EngineCmd::MidiControl { generation, event });
    send(ControlEvent::Toggle { index: 0 });
    assert_eq!(
        next_message(&receiver),
        ("/avatar/parameters/Glasses".into(), OscType::Int(1))
    );
    send(ControlEvent::Toggle { index: 0 });
    assert_eq!(
        next_message(&receiver),
        ("/avatar/parameters/Glasses".into(), OscType::Int(0))
    );
    send(ControlEvent::Set { index: 1, value: 0.5 });
    assert_eq!(
        next_message(&receiver),
        ("/avatar/parameters/HUE".into(), OscType::Float(0.5))
    );

    // Events from older mappings are ignored.
    engine.send(EngineCmd::MidiControl {
        generation: 0,
        event: ControlEvent::Toggle { index: 0 },
    });
    receiver
        .set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    assert!(
        receiver.recv_from(&mut [0u8; 64]).is_err(),
        "stale event must not send"
    );

    // The display catches up shortly after (throttled).
    let deadline = Instant::now() + Duration::from_secs(2);
    while engine.snapshot().custom_values != [0.0, 0.5] {
        assert!(
            Instant::now() < deadline,
            "snapshot never updated: {:?}",
            engine.snapshot().custom_values
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    engine.shutdown();
}
