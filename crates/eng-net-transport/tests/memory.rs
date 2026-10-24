use eng_net_transport::memory::MemoryNetwork;
use eng_net_transport::{PeerId, SendError, Transport, conformance};

#[test]
fn memory_passes_conformance() {
    let net = MemoryNetwork::new();
    let (mut a, mut b) = (net.endpoint(), net.endpoint());
    conformance::check(&mut a, &mut b);
}

#[test]
fn memory_keeps_order_and_forgets_dropped_peers() {
    let net = MemoryNetwork::new();
    let mut a = net.endpoint();
    let b = net.endpoint();
    let idb = b.local_id();
    for i in 0..10u8 {
        a.send(idb, &[i]).unwrap();
    }
    let mut b = b;
    let order: Vec<u8> = std::iter::from_fn(|| b.poll_recv())
        .map(|(_, d)| d[0])
        .collect();
    assert_eq!(order, (0..10).collect::<Vec<_>>());
    drop(b);
    assert_eq!(a.send(idb, b"x"), Err(SendError::UnknownPeer(idb)));
    assert_eq!(
        a.send(PeerId::from_raw(99), b"x"),
        Err(SendError::UnknownPeer(PeerId::from_raw(99)))
    );
}
