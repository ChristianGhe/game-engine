//! What the sorted-graph tests share: every order of declaration.
// Each test file uses some of these.
#![allow(dead_code)]

use eng_gpu::{Graph, GraphError, PassNode, Target};

/// Every order of `items` (Heap's algorithm), the first as given.
pub fn permutations<T: Copy>(items: &[T]) -> Vec<Vec<T>> {
    fn heap<T: Copy>(k: usize, items: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if k <= 1 {
            out.push(items.clone());
            return;
        }
        heap(k - 1, items, out);
        for i in 0..k - 1 {
            if k.is_multiple_of(2) {
                items.swap(i, k - 1);
            } else {
                items.swap(0, k - 1);
            }
            heap(k - 1, items, out);
        }
    }
    let mut out = Vec::new();
    heap(items.len(), &mut items.to_vec(), &mut out);
    out
}

/// `nodes` sorted with `off` switched off, after checking that every order
/// of declaration gives the same passes, culled and bypassed passes, and
/// pool.
pub fn in_every_order(nodes: &[PassNode], off: &[&'static str], target: Target) -> Graph {
    let graph = Graph::sorted_without(nodes, off, target).unwrap_or_else(|e| panic!("{e}"));
    for order in permutations(nodes) {
        let other = Graph::sorted_without(&order, off, target).unwrap_or_else(|e| panic!("{e}"));
        let names = |g: &Graph| g.passes().iter().map(|p| p.name).collect::<Vec<_>>();
        assert_eq!(
            names(&other),
            names(&graph),
            "declared as {:?}",
            names_of(&order)
        );
        assert_eq!(other.passes(), graph.passes());
        assert_eq!(other.culled(), graph.culled());
        assert_eq!(other.bypassed(), graph.bypassed());
        assert_eq!(other.pool(), graph.pool());
    }
    graph
}

/// The error `nodes` give with `off` switched off, after checking that
/// every order of declaration gives the same one.
pub fn error_in_every_order(
    nodes: &[PassNode],
    off: &[&'static str],
    target: Target,
) -> GraphError {
    let error = Graph::sorted_without(nodes, off, target).unwrap_err();
    for order in permutations(nodes) {
        assert_eq!(
            Graph::sorted_without(&order, off, target).unwrap_err(),
            error,
            "declared as {:?}",
            names_of(&order)
        );
    }
    error
}

fn names_of(nodes: &[PassNode]) -> Vec<&'static str> {
    nodes.iter().map(|n| n.name).collect()
}
