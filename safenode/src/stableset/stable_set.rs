use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    iter::FromIterator,
};

use serde::{Deserialize, Serialize};

use crate::comms::NetworkNode;

use super::StableSetMsg;

pub(crate) fn majority(m: usize, n: usize) -> bool {
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

pub(crate) type Elders = BTreeSet<NetworkNode>;

const ELDER_COUNT: usize = 4;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct StableSet {
    members: BTreeSet<Member>,
    joining_members: BTreeMap<Member, BTreeSet<NetworkNode>>,
    leaving_members: BTreeMap<Member, BTreeSet<NetworkNode>>,
}

impl Default for StableSet {
    fn default() -> Self {
        Self {
            members: BTreeSet::new(),
            joining_members: BTreeMap::new(),
            leaving_members: BTreeMap::new(),
        }
    }
}

impl StableSet {
    // Initialize a new StableSet with a given set of genesis nodes.
    pub(crate) fn new(genesis: &BTreeSet<NetworkNode>) -> Self {
        let mut stable_set = StableSet::default();

        for genesis_id in genesis.iter().copied() {
            let genesis_member = Member {
                id: genesis_id,
                ord_idx: 0,
            };
            for other_genesis_id in genesis.iter().copied() {
                stable_set.add(genesis_member.clone(), other_genesis_id);
            }
        }

        stable_set.process_ready_actions(genesis);

        assert_eq!(&BTreeSet::from_iter(stable_set.ids()), genesis);

        stable_set
    }
    // Process ready actions for joining and leaving members based on elder witnesses
    pub(crate) fn process_ready_actions(&mut self, elders: &Elders) -> bool {
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

    pub(crate) fn add(&mut self, member: Member, witness: NetworkNode) -> bool {
        if !self.is_member(&member) {
            self.joining_members
                .entry(member)
                .or_default()
                .insert(witness)
        } else {
            false
        }
    }

    pub(crate) fn remove(&mut self, member: Member, witness: NetworkNode) -> bool {
        if self.is_member(&member) {
            self.leaving_members
                .entry(member)
                .or_default()
                .insert(witness)
        } else {
            false
        }
    }

    pub(crate) fn joining_witnesses(&mut self, member: &Member) -> BTreeSet<NetworkNode> {
        self.joining_members
            .get(member)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn leaving_witnesses(&mut self, member: &Member) -> BTreeSet<NetworkNode> {
        self.leaving_members
            .get(member)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn is_leaving(&mut self, member: &Member) -> bool {
        self.leaving_members.contains_key(member)
    }

    pub(crate) fn member_by_id(&self, id: NetworkNode) -> Option<Member> {
        self.members.iter().find(|m| m.id == id).cloned()
    }

    pub(crate) fn is_member(&self, member: &Member) -> bool {
        self.members.contains(member)
    }

    pub(crate) fn contains(&self, id: NetworkNode) -> bool {
        self.ids().any(|m| m == id)
    }

    pub(crate) fn ids(&self) -> impl Iterator<Item = NetworkNode> + '_ {
        self.members.iter().map(|m| m.id)
    }

    pub(crate) fn members(&self) -> BTreeSet<Member> {
        self.members.clone()
    }

    pub(crate) fn leaving(&self) -> impl Iterator<Item = Member> + '_ {
        self.leaving_members.keys().cloned()
    }

    pub(crate) fn joining(&self) -> impl Iterator<Item = Member> + '_ {
        self.joining_members.keys().cloned()
    }
    fn build_msg(&self, msg: StableSetMsg) -> StableSetMsg {
        msg
    }

    pub(crate) fn req_join(&self, id: NetworkNode) -> StableSetMsg {
        self.build_msg(StableSetMsg::ReqJoin(id))
    }

    pub(crate) fn req_leave(&mut self, id: NetworkNode) -> StableSetMsg {
        if let Some(member) = self.member_by_id(id) {
            self.handle_leave_witness(id, member, id);
        }
        self.build_msg(StableSetMsg::LeaveWitness(id))
    }

    pub(crate) fn elders(&self) -> Elders {
        BTreeSet::from_iter(self.members().into_iter().take(ELDER_COUNT).map(|m| m.id))
    }

    pub(crate) fn merge(
        &mut self,
        stable_set: StableSet,
        id: NetworkNode,
        src: NetworkNode,
    ) -> BTreeSet<NetworkNode> {
        let mut additional_members_to_sync = BTreeSet::new();

        for member in stable_set.members() {
            let m_id = member.id;

            if self.handle_join_witness(id, member, src) {
                additional_members_to_sync.insert(m_id);
                additional_members_to_sync.extend(self.elders());
            }
        }

        for member in stable_set.joining() {
            let m_id = member.id;
            if self.handle_join_witness(id, member, src) {
                additional_members_to_sync.insert(m_id);
                additional_members_to_sync.extend(self.elders());
            }
        }

        for member in stable_set.leaving() {
            let m_id = member.id;
            if self.handle_leave_witness(id, member, src) {
                additional_members_to_sync.insert(m_id);
                additional_members_to_sync.extend(self.elders());
            }
        }

        // For each member we know is leaving, check if the other node has already removed it.
        let to_handle = Vec::from_iter(self.leaving().filter(|m| !stable_set.is_member(m)));
        for member in to_handle {
            let m_id = member.id;
            if self.handle_leave_witness(id, member, src) {
                additional_members_to_sync.insert(m_id);
                additional_members_to_sync.extend(self.elders());
            }
        }

        additional_members_to_sync
    }

    /// Handles msg, returns any nodes needing a sync afterwards
    pub(crate) fn on_msg_return_nodes_to_sync(
        &mut self,
        elders: &BTreeSet<NetworkNode>,
        id: NetworkNode,
        src: NetworkNode,
        msg: StableSetMsg,
    ) -> BTreeSet<NetworkNode> {
        debug!("Handling {msg:?}");
        let mut additional_members_to_sync = BTreeSet::new();
        match msg {
            StableSetMsg::Sync(stable_set) => {
                // TODO: stuff on sync...
                self.merge(stable_set, id, src);
                debug!("Nothing yet happening on sync");
            }
            StableSetMsg::Ping | StableSetMsg::Pong => {
                trace!("Nothing to do with ping/pong");
            }
            StableSetMsg::ReqJoin(candidate_id) => {
                if self.member_by_id(candidate_id).is_none() && elders.contains(&id) {
                    let latest_ord_idx =
                        self.members().iter().map(|m| m.ord_idx).max().unwrap_or(0);
                    let ord_idx = latest_ord_idx + 1;

                    let member = Member {
                        id: candidate_id,
                        ord_idx,
                    };

                    if self.handle_join_witness(id, member, id) {
                        additional_members_to_sync.insert(candidate_id);
                        additional_members_to_sync.extend(elders);
                    }
                }
            }
            StableSetMsg::LeaveWitness(to_remove) => {
                if let Some(member) = self.member_by_id(to_remove) {
                    if self.handle_leave_witness(id, member, src) {
                        additional_members_to_sync.insert(to_remove);
                        additional_members_to_sync.extend(elders);
                    }
                }
            }
            StableSetMsg::JoinWitness(member) => {
                let m_id = member.id;
                if self.handle_join_witness(id, member, src) {
                    additional_members_to_sync.insert(m_id);
                    additional_members_to_sync.extend(elders);
                }
            }
        }

        info!(
            "Current confirmed StableSet length is : {:?}",
            self.members().len()
        );
        info!(
            "The set is : {:?}",
            self.members()
        );
        additional_members_to_sync
    }

    pub(crate) fn process_pending_actions(&mut self, id: NetworkNode) -> BTreeSet<NetworkNode> {
        let stable_set_changed = self.process_ready_actions(&self.elders());

        if stable_set_changed && self.elders().contains(&id) {
            self.ids().filter(|e| e != &id).collect()
        } else {
            Default::default()
        }
    }

    fn handle_join_witness(
        &mut self,
        id: NetworkNode,
        member: Member,
        witness: NetworkNode,
    ) -> bool {
        if self.is_member(&member) {
            return false;
        }

        trace!("Adding member: {id:?}");

        let first_time_seeing_join = self.add(member.clone(), witness);
        self.add(member, id);

        first_time_seeing_join
    }

    fn handle_leave_witness(
        &mut self,
        id: NetworkNode,
        member: Member,
        witness: NetworkNode,
    ) -> bool {
        if !self.is_member(&member) {
            return false;
        }

        let first_time_seeing_leave = self.remove(member.clone(), witness);
        self.remove(member, id);

        first_time_seeing_leave
    }
}
