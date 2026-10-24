//! In-process transport: lossless and in order. Single-player runs the
//! server and client over it, still encoding every message to bytes, so
//! there is one code path.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::{MAX_DATAGRAM, PeerId, SendError, Transport};

type Queue = VecDeque<(PeerId, Vec<u8>)>;

#[derive(Default)]
struct Hub {
    next: u64,
    inboxes: BTreeMap<PeerId, Queue>,
}

/// A set of connected in-memory endpoints.
#[derive(Clone, Default)]
pub struct MemoryNetwork {
    hub: Rc<RefCell<Hub>>,
}

impl MemoryNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new endpoint with the next free id.
    pub fn endpoint(&self) -> MemoryEndpoint {
        let mut hub = self.hub.borrow_mut();
        let id = PeerId::from_raw(hub.next);
        hub.next += 1;
        hub.inboxes.insert(id, Queue::new());
        MemoryEndpoint {
            id,
            hub: Rc::clone(&self.hub),
        }
    }
}

pub struct MemoryEndpoint {
    id: PeerId,
    hub: Rc<RefCell<Hub>>,
}

impl Transport for MemoryEndpoint {
    fn local_id(&self) -> PeerId {
        self.id
    }

    fn send(&mut self, to: PeerId, bytes: &[u8]) -> Result<(), SendError> {
        if bytes.len() > MAX_DATAGRAM {
            return Err(SendError::TooLarge { len: bytes.len() });
        }
        let mut hub = self.hub.borrow_mut();
        let inbox = hub.inboxes.get_mut(&to).ok_or(SendError::UnknownPeer(to))?;
        inbox.push_back((self.id, Vec::from(bytes)));
        Ok(())
    }

    fn poll_recv(&mut self) -> Option<(PeerId, Vec<u8>)> {
        self.hub
            .borrow_mut()
            .inboxes
            .get_mut(&self.id)
            .and_then(VecDeque::pop_front)
    }
}

impl Drop for MemoryEndpoint {
    fn drop(&mut self) {
        self.hub.borrow_mut().inboxes.remove(&self.id);
    }
}
