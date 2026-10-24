//! Rules every [`Transport`] must follow. Lossy transports run the suite with
//! loss turned off.

use alloc::vec;
use alloc::vec::Vec;

use crate::{MAX_DATAGRAM, SendError, Transport};

/// Checks a connected pair of endpoints.
///
/// # Panics
/// With a description of the first rule broken.
pub fn check<T: Transport>(a: &mut T, b: &mut T) {
    let (ida, idb) = (a.local_id(), b.local_id());
    assert_ne!(ida, idb, "endpoints need distinct ids");
    assert!(
        a.poll_recv().is_none(),
        "a fresh endpoint has nothing queued"
    );

    let largest = vec![0xa5u8; MAX_DATAGRAM];
    a.send(idb, b"hello").expect("send a small datagram");
    a.send(idb, &largest).expect("send the largest datagram");
    a.send(idb, &[]).expect("send an empty datagram");
    let mut got: Vec<Vec<u8>> = Vec::new();
    while let Some((from, bytes)) = b.poll_recv() {
        assert_eq!(from, ida, "datagrams carry the sender's id");
        got.push(bytes);
    }
    got.sort();
    let mut want = vec![Vec::new(), b"hello".to_vec(), largest];
    want.sort();
    assert_eq!(got, want, "datagrams arrive intact");

    assert_eq!(
        a.send(idb, &vec![0u8; MAX_DATAGRAM + 1]),
        Err(SendError::TooLarge {
            len: MAX_DATAGRAM + 1
        }),
        "oversized datagrams are refused"
    );
    b.send(ida, b"back").expect("send the other way");
    assert_eq!(a.poll_recv(), Some((idb, b"back".to_vec())));
    assert!(a.poll_recv().is_none());
}
