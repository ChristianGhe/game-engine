use eng_core::{GenAllocator, GenId};
use eng_schema_core::{from_bytes, to_bytes};

#[test]
fn allocator_reuses_lifo_with_new_generation() {
    let mut ids = GenAllocator::new();
    let a = ids.alloc();
    let b = ids.alloc();
    let c = ids.alloc();
    assert_eq!((a.index, b.index, c.index), (0, 1, 2));
    assert!(ids.free(b));
    assert!(!ids.free(b), "double free is refused");
    assert!(ids.free(a));
    let d = ids.alloc();
    assert_eq!(
        d,
        GenId {
            index: 0,
            generation: 1
        }
    );
    assert!(!ids.is_alive(a));
    assert!(ids.is_alive(d));
    let alive: Vec<_> = ids.iter_alive().map(|id| id.index).collect();
    assert_eq!(alive, [0, 2]);
    assert!(ids.is_consistent());
    let back: GenAllocator = from_bytes(&to_bytes(&ids)).unwrap();
    assert_eq!(back, ids);
}
