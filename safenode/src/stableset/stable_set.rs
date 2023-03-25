use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
};

use crate::comms::NetworkNode;

use super::membership::Elders;

pub fn majority(m: usize, n: usize) -> bool {
    2 * m > n
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Member {
    pub ord_idx: u64,
    pub id: NetworkNode,
}

impl Debug for Member {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{:?}", self.ord_idx, self.id)
    }
}

#[derive(
    Clone, Eq, Hash, PartialEq, PartialOrd, Ord, Default, serde::Serialize, serde::Deserialize,
)]
pub struct StableSet {
    members: BTreeSet<Member>,
    // dead: BTreeSet<Id>,
    pub joining_members: BTreeMap<Member, BTreeSet<NetworkNode>>,
    pub leaving_members: BTreeMap<Member, BTreeSet<NetworkNode>>,
}

impl Debug for StableSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SS({:?}", self.members)?;

        if !self.joining_members.is_empty() {
            write!(f, ", joining:{:?}", self.joining_members)?;
        }

        if !self.leaving_members.is_empty() {
            write!(f, ", leaving:{:?}", self.leaving_members)?;
        }

        write!(f, ")")
    }
}

impl StableSet {
    pub fn process_ready_actions(&mut self, elders: &Elders) -> bool {
        let mut updated = false;

        let ready_to_join = Vec::from_iter(
            self.joining_members
                .iter()
                .filter(|(_, witnesses)| {
                    majority(witnesses.intersection(elders).count(), elders.len())
                })
                .map(|(member, _)| member)
                .cloned(),
        );

        updated |= !ready_to_join.is_empty();

        for member in ready_to_join {
            self.joining_members.remove(&member);

            if let Some(existing_member_with_id) = self.member_by_id(member.id) {
                if existing_member_with_id.ord_idx >= member.ord_idx {
                    continue;
                } else {
                    self.members.remove(&existing_member_with_id);
                }
            }

            self.members.insert(member);
        }

        let ready_to_leave = Vec::from_iter(
            self.leaving_members
                .iter()
                .filter(|(_, witnesses)| {
                    majority(witnesses.intersection(elders).count(), elders.len())
                })
                .map(|(member, _)| member)
                .cloned(),
        );

        updated |= !ready_to_leave.is_empty();

        for member in ready_to_leave {
            self.leaving_members.remove(&member);
            self.members.remove(&member);
        }

        updated
    }

    pub fn add(&mut self, member: Member, witness: NetworkNode) -> bool {
        if !self.is_member(&member) {
            self.joining_members
                .entry(member)
                .or_default()
                .insert(witness)
        } else {
            false
        }
    }

    pub fn remove(&mut self, member: Member, witness: NetworkNode) -> bool {
        if self.is_member(&member) {
            self.leaving_members
                .entry(member)
                .or_default()
                .insert(witness)
        } else {
            false
        }
    }

    pub fn joining_witnesses(&mut self, member: &Member) -> BTreeSet<NetworkNode> {
        self.joining_members
            .get(member)
            .cloned()
            .unwrap_or_default()
    }

    pub fn leaving_witnesses(&mut self, member: &Member) -> BTreeSet<NetworkNode> {
        self.leaving_members
            .get(member)
            .cloned()
            .unwrap_or_default()
    }

    pub fn is_leaving(&mut self, member: &Member) -> bool {
        self.leaving_members.contains_key(member)
    }

    pub fn member_by_id(&self, id: NetworkNode) -> Option<Member> {
        self.members.iter().find(|m| m.id == id).cloned()
    }

    pub fn is_member(&self, member: &Member) -> bool {
        self.members.contains(member)
    }

    pub fn contains(&self, id: NetworkNode) -> bool {
        self.ids().any(|m| m == id)
    }

    pub fn ids(&self) -> impl Iterator<Item = NetworkNode> + '_ {
        self.members.iter().map(|m| m.id)
    }

    pub fn members(&self) -> BTreeSet<Member> {
        self.members.clone()
    }

    pub fn leaving(&self) -> impl Iterator<Item = Member> + '_ {
        self.leaving_members.keys().cloned()
    }

    pub fn joining(&self) -> impl Iterator<Item = Member> + '_ {
        self.joining_members.keys().cloned()
    }
}
