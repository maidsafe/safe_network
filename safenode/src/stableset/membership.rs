use std::collections::BTreeSet;
use std::fmt::Debug;

use super::{
    stable_set::{Member, StableSet},
    StableSetMsg,
};
use crate::comms::NetworkNode;

pub(crate) type Elders = BTreeSet<NetworkNode>;

const ELDER_COUNT: usize = 4;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Membership {
    pub(crate) stable_set: StableSet,
}

impl Membership {
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

        Self { stable_set }
    }

    fn build_msg(&self, msg: StableSetMsg) -> StableSetMsg {
        msg
    }

    pub(crate) fn req_join(&self, id: NetworkNode) -> StableSetMsg {
        self.build_msg(StableSetMsg::ReqJoin(id))
    }

    pub(crate) fn req_leave(&mut self, id: NetworkNode) -> StableSetMsg {
        if let Some(member) = self.stable_set.member_by_id(id) {
            self.handle_leave_witness(id, member, id);
        }
        self.build_msg(StableSetMsg::LeaveWitness(id))
    }

    pub(crate) fn is_member(&self, id: NetworkNode) -> bool {
        self.stable_set.contains(id)
    }

    pub(crate) fn members(&self) -> BTreeSet<Member> {
        self.stable_set.members()
    }

    pub(crate) fn members_from_our_pov(&self) -> BTreeSet<Member> {
        let mut members = self.stable_set.members();

        members.extend(self.stable_set.joining());

        members
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
        let to_handle = Vec::from_iter(
            self.stable_set
                .leaving()
                .filter(|m| !stable_set.is_member(m)),
        );
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
                if self.stable_set.member_by_id(candidate_id).is_none() && elders.contains(&id) {
                    let latest_ord_idx = self
                        .stable_set
                        .members()
                        .iter()
                        .map(|m| m.ord_idx)
                        .max()
                        .unwrap_or(0);
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
                if let Some(member) = self.stable_set.member_by_id(to_remove) {
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
            self.stable_set.members().len()
        );
        additional_members_to_sync
    }

    pub(crate) fn process_pending_actions(&mut self, id: NetworkNode) -> BTreeSet<NetworkNode> {
        let stable_set_changed = self.stable_set.process_ready_actions(&self.elders());

        if stable_set_changed && self.elders().contains(&id) {
            self.stable_set.ids().filter(|e| e != &id).collect()
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
        if self.stable_set.is_member(&member) {
            return false;
        }

        let first_time_seeing_join = self.stable_set.add(member.clone(), witness);
        self.stable_set.add(member, id);

        first_time_seeing_join
    }

    fn handle_leave_witness(
        &mut self,
        id: NetworkNode,
        member: Member,
        witness: NetworkNode,
    ) -> bool {
        if !self.stable_set.is_member(&member) {
            return false;
        }

        let first_time_seeing_leave = self.stable_set.remove(member.clone(), witness);
        self.stable_set.remove(member, id);

        first_time_seeing_leave
    }
}
