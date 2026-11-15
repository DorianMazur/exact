//! The Contract instance owns its children for their whole lifetime. Only
//! their kernel parent changes when a matching shared-element box appears.
use super::*;
use exact_kernel::PropId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub(super) struct Placement {
    active: BTreeMap<ViewId, Content>,
}

#[derive(Debug, PartialEq, Eq)]
struct Content {
    owner: ViewId,
    children: Vec<ViewId>,
}

#[derive(Default)]
struct Candidates<'a> {
    owners: Vec<&'a NodeInst>,
    slots: Vec<&'a NodeInst>,
}

impl Placement {
    pub(super) fn authored_parent(&self, parent: ViewId) -> ViewId {
        self.active.get(&parent).map_or(parent, |c| c.owner)
    }
}

impl Tree {
    pub(crate) fn place_shared_content(&mut self, u: &mut Update<'_>) {
        let Some(placement) = &mut self.shared_content else {
            return;
        };
        if u.discard
            || !u.ops.iter().any(|op| {
                matches!(
                    op,
                    Op::SetChildren { .. }
                        | Op::CreateView { .. }
                        | Op::DestroyView { .. }
                        | Op::ClearProp {
                            prop: PropId::SharedContent | PropId::SharedElement,
                            ..
                        }
                        | Op::SetProp {
                            prop: PropId::SharedContent | PropId::SharedElement,
                            ..
                        }
                )
            })
        {
            return;
        }
        let mut candidates = BTreeMap::new();
        collect(&self.children, u, None, false, &mut candidates);
        let next: BTreeMap<_, _> = candidates
            .into_values()
            .filter_map(|c| {
                let ([owner], [slot]) = (c.owners.as_slice(), c.slots.as_slice()) else {
                    return None;
                };
                if !slot.last_children.is_empty() || slot.collection().is_some() {
                    return None;
                }
                Some((
                    slot.view,
                    Content {
                        owner: owner.view,
                        children: owner.last_children.to_vec(),
                    },
                ))
            })
            .collect();
        let rewritten: BTreeSet<_> = u
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::SetChildren { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        let touched = |slot: &ViewId, content: &Content| {
            rewritten.contains(slot) || rewritten.contains(&content.owner)
        };
        // A destroy recursively removes the kernel subtree. Restore the
        // authored parents before the batch so a retiring slot cannot take a
        // live child with it, and a retiring owner releases all its children.
        let restore = placement
            .active
            .iter()
            .filter(|(slot, old)| next.get(slot) != Some(old) || touched(slot, old))
            .map(|(_, old)| Op::SetChildren {
                id: old.owner,
                children: old.children.clone(),
            });
        u.ops.splice(0..0, restore);
        for (slot, content) in &next {
            if placement.active.get(slot) != Some(content) || touched(slot, content) {
                u.ops.push(Op::SetChildren {
                    id: *slot,
                    children: content.children.clone(),
                });
            }
        }
        placement.active = next;
    }
}

fn collect<'a>(
    children: &'a [Child],
    u: &Update<'_>,
    root: Option<ViewId>,
    inside_owner: bool,
    candidates: &mut BTreeMap<(ViewId, String), Candidates<'a>>,
) {
    for child in children {
        match child {
            Child::Node(n) => {
                let root = root.unwrap_or(n.view);
                let name = |prop| {
                    n.bound_prop(u.env.plan, u.sites, prop).and_then(|v| {
                        v.as_str()
                            .filter(|name| !name.is_empty())
                            .map(str::to_owned)
                    })
                };
                let mut within = inside_owner;
                if u.env.plan.node(n.node).node_type == NodeType::View as u8 {
                    if let Some(owner) = name(PropId::SharedContent) {
                        candidates.entry((root, owner)).or_default().owners.push(n);
                        within = true;
                    } else if !within {
                        if let Some(slot) = name(PropId::SharedElement) {
                            candidates.entry((root, slot)).or_default().slots.push(n);
                        }
                    }
                }
                for (_, row) in n.collection().into_iter().flat_map(|c| c.mounted_rows()) {
                    collect(&row.roots, u, Some(root), within, candidates);
                }
                collect(&n.children, u, Some(root), within, candidates);
            }
            Child::Region(r) => match &r.active {
                Active::Arm { roots, .. } => collect(roots, u, root, inside_owner, candidates),
                Active::Rows { rows } => {
                    for row in rows {
                        collect(&row.roots, u, root, inside_owner, candidates);
                    }
                }
            },
        }
    }
}
