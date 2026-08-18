
use crate::{MailboxSender, RefcountDeltaPlan, TargetNotRegisteredError, Tier2Handle, Tier3};
use std::collections::HashSet;
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {

    Transfer(String),

    Capability(String),

    TargetNotRegistered,
}

impl From<TargetNotRegisteredError> for BoundaryError {
    fn from(_: TargetNotRegisteredError) -> Self {
        BoundaryError::TargetNotRegistered
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope<Msg, Cap> {
    pub message: Msg,
    pub capabilities: Vec<Cap>,
}

#[derive(Debug, Clone, Default)]
pub struct Crossing {
    pub clone_crossed: Vec<Tier2Handle>,
    pub transfer_crossed: Vec<Tier2Handle>,
}

#[derive(Debug, Clone)]
pub struct SendPlan<Cap> {
    refcount: RefcountDeltaPlan,
    capabilities: Vec<Cap>,
}

impl<Cap> SendPlan<Cap> {

    pub fn atomic_touches(&self) -> usize {
        crate::tier3_atomic_touches(&self.refcount)
    }
}

pub struct BoundaryWrapper<T, Msg, Cap> {
    tier3: Tier3<T>,

    senders: std::collections::HashMap<u64, MailboxSender<Envelope<Msg, Cap>>>,
}

impl<T, Msg, Cap> Default for BoundaryWrapper<T, Msg, Cap>
where
    Cap: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, Msg, Cap> BoundaryWrapper<T, Msg, Cap>
where
    Cap: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Self {
            tier3: Tier3::new(),
            senders: std::collections::HashMap::new(),
        }
    }

    pub fn tier3(&self) -> &Tier3<T> {
        &self.tier3
    }
    pub fn tier3_mut(&mut self) -> &mut Tier3<T> {
        &mut self.tier3
    }

    pub fn register_compartment(
        &mut self,
        compartment: u64,
        sender: MailboxSender<Envelope<Msg, Cap>>,
    ) {
        self.tier3.register(compartment);
        self.senders.insert(compartment, sender);
    }

    pub fn terminate_compartment(&mut self, compartment: u64) {
        self.tier3.unregister(compartment);
        self.senders.remove(&compartment);
    }

    pub fn is_registered(&self, compartment: u64) -> bool {
        self.tier3.is_registered(compartment)
    }

    pub fn validate<E>(
        &self,
        to_compartment: u64,
        crossing: &Crossing,
        capability_set: &[Cap],
        sender_capabilities: &HashSet<Cap>,
        classify: impl FnOnce() -> Result<(), E>,
    ) -> Result<SendPlan<Cap>, BoundaryError>
    where
        E: Into<String>,
    {

        classify().map_err(|e| BoundaryError::Transfer(e.into()))?;

        let refcount = self.tier3.plan_crossing(
            &crossing.clone_crossed,
            &crossing.transfer_crossed,
            to_compartment,
        );
        if !refcount.targets_valid {
            return Err(BoundaryError::TargetNotRegistered);
        }

        for cap in capability_set {
            if !sender_capabilities.contains(cap) {
                return Err(BoundaryError::Capability(
                    "capability not held by sender (no privilege escalation)".to_string(),
                ));
            }
        }

        Ok(SendPlan {
            refcount,
            capabilities: capability_set.to_vec(),
        })
    }

    pub fn commit<E>(
        &mut self,
        to_compartment: u64,
        message: Msg,
        plan: SendPlan<Cap>,
        clone: impl FnOnce(Msg) -> Result<Msg, E>,
    ) -> Result<(), BoundaryError>
    where
        E: Into<String>,
    {

        let message = clone(message).map_err(|e| BoundaryError::Transfer(e.into()))?;

        self.tier3.apply(&plan.refcount)?;

        let env = Envelope {
            message,
            capabilities: plan.capabilities,
        };

        self.senders
            .get(&to_compartment)
            .expect("registered target has a mailbox sender (liveness invariant)")
            .enqueue(env)
            .map_err(|_| BoundaryError::TargetNotRegistered)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn boundary_send<E1, E2>(
        &mut self,
        to_compartment: u64,
        message: Msg,
        crossing: &Crossing,
        capability_set: &[Cap],
        sender_capabilities: &HashSet<Cap>,
        classify: impl FnOnce() -> Result<(), E1>,
        clone: impl FnOnce(Msg) -> Result<Msg, E2>,
    ) -> Result<(), BoundaryError>
    where
        E1: Into<String>,
        E2: Into<String>,
    {

        let plan = self.validate(
            to_compartment,
            crossing,
            capability_set,
            sender_capabilities,
            classify,
        )?;

        self.commit(to_compartment, message, plan, clone)
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use crate::Mailbox;

    fn cloneable() -> Result<(), String> {
        Ok(())
    }

    fn id_clone<M>(m: M) -> Result<M, String> {
        Ok(m)
    }

    fn caps(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    type Env = Envelope<&'static str, String>;

    fn wrapper_with(id: u64) -> (BoundaryWrapper<i64, &'static str, String>, Mailbox<Env>) {
        let mut bw: BoundaryWrapper<i64, &str, String> = BoundaryWrapper::new();
        let mb: Mailbox<Env> = Mailbox::new();
        bw.register_compartment(id, mb.sender());
        (bw, mb)
    }

    #[test]
    fn valid_send_commits_and_enqueues_with_capability_symmetry() {
        let (mut bw, mb) = wrapper_with(1);
        let h = bw.tier3_mut().arena_mut().freeze(7, vec![]).unwrap();
        assert_eq!(bw.tier3().arena().refcount(h), Some(1));

        let sender = caps(&["net", "fs"]);
        let crossing = Crossing {
            clone_crossed: vec![h],
            transfer_crossed: vec![],
        };
        bw.boundary_send(
            1,
            "hello",
            &crossing,
            &["net".to_string()],
            &sender,
            cloneable,
            id_clone,
        )
        .expect("valid send");

        assert_eq!(
            bw.tier3().arena().refcount(h),
            Some(2),
            "clone-crossed handle incref'd"
        );

        let env = mb
            .try_dequeue()
            .expect("one envelope enqueued onto the scheduler mailbox");
        assert_eq!(env.message, "hello");
        assert_eq!(env.capabilities, vec!["net".to_string()]);
        assert!(mb.try_dequeue().is_none(), "exactly one envelope");
    }

    #[test]
    fn unregistered_target_throws_in_phase_a_with_zero_mutation() {
        let (mut bw, mb) = wrapper_with(1);
        let h = bw.tier3_mut().arena_mut().freeze(7, vec![]).unwrap();
        let sender = caps(&["net"]);
        let crossing = Crossing {
            clone_crossed: vec![h],
            transfer_crossed: vec![],
        };

        let r = bw.boundary_send(
            999,
            "hi",
            &crossing,
            &["net".to_string()],
            &sender,
            cloneable,
            id_clone,
        );
        assert_eq!(r, Err(BoundaryError::TargetNotRegistered));

        assert_eq!(
            bw.tier3().arena().refcount(h),
            Some(1),
            "rejected send incref'd nothing"
        );
        assert!(mb.try_dequeue().is_none(), "rejected send enqueued nothing");
    }

    #[test]
    fn capability_escalation_throws_in_phase_a_with_zero_mutation() {
        let (mut bw, mb) = wrapper_with(1);
        let h = bw.tier3_mut().arena_mut().freeze(7, vec![]).unwrap();

        let sender = caps(&["net"]);
        let crossing = Crossing {
            clone_crossed: vec![h],
            transfer_crossed: vec![],
        };

        let r = bw.boundary_send(
            1,
            "hi",
            &crossing,
            &["fs".to_string()],
            &sender,
            cloneable,
            id_clone,
        );
        assert!(
            matches!(r, Err(BoundaryError::Capability(_))),
            "escalation rejected"
        );

        assert_eq!(
            bw.tier3().arena().refcount(h),
            Some(1),
            "rejected send incref'd nothing"
        );
        assert!(mb.try_dequeue().is_none(), "rejected send enqueued nothing");
    }

    #[test]
    fn no_ambient_capability_each_send_carries_only_its_own_set() {

        let (mut bw, mb) = wrapper_with(1);
        let sender = caps(&["net"]);
        let empty = Crossing::default();

        bw.boundary_send(
            1,
            "first",
            &empty,
            &["net".to_string()],
            &sender,
            cloneable,
            id_clone,
        )
        .unwrap();
        bw.boundary_send(1, "second", &empty, &[], &sender, cloneable, id_clone)
            .unwrap();

        let drained = mb.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].capabilities, vec!["net".to_string()]);
        assert!(
            drained[1].capabilities.is_empty(),
            "no ambient: second send carries nothing"
        );
    }

    #[test]
    fn classify_failure_throws_transfer_error_in_phase_a() {
        let (mut bw, mb) = wrapper_with(1);
        let h = bw.tier3_mut().arena_mut().freeze(7, vec![]).unwrap();
        let sender = caps(&["net"]);
        let crossing = Crossing {
            clone_crossed: vec![h],
            transfer_crossed: vec![],
        };

        let r = bw.boundary_send(
            1,
            "x",
            &crossing,
            &["net".to_string()],
            &sender,
            || Err::<(), String>("non-cloneable (no Symbol.cloneAs)".to_string()),
            id_clone,
        );
        assert!(matches!(r, Err(BoundaryError::Transfer(_))));
        assert_eq!(
            bw.tier3().arena().refcount(h),
            Some(1),
            "rejected send mutated nothing"
        );
        assert!(mb.try_dequeue().is_none());
    }

    #[test]
    fn transfer_crossed_decrements_on_commit() {

        let (mut bw, _mb) = wrapper_with(1);
        let h = bw.tier3_mut().arena_mut().freeze(7, vec![]).unwrap();
        bw.tier3().arena().incref(h);
        assert_eq!(bw.tier3().arena().refcount(h), Some(2));

        let sender = caps(&[]);
        let crossing = Crossing {
            clone_crossed: vec![],
            transfer_crossed: vec![h],
        };
        bw.boundary_send(1, "moved", &crossing, &[], &sender, cloneable, id_clone)
            .unwrap();
        assert_eq!(
            bw.tier3().arena().refcount(h),
            Some(1),
            "transfer released the sender ref"
        );
    }

    #[test]
    fn clone_failure_in_phase_b_throws_before_refcount_apply() {

        let (mut bw, mb) = wrapper_with(1);
        let h = bw.tier3_mut().arena_mut().freeze(7, vec![]).unwrap();
        let holder = caps(&["net"]);
        let crossing = Crossing {
            clone_crossed: vec![h],
            transfer_crossed: vec![],
        };

        let r = bw.boundary_send(
            1,
            "has-a-symbol",
            &crossing,
            &["net".to_string()],
            &holder,
            cloneable,
            |_m| Err::<&str, String>("DataCloneError: Symbol not cloneable".to_string()),
        );
        assert!(
            matches!(r, Err(BoundaryError::Transfer(_))),
            "clone failure -> TransferError"
        );

        assert_eq!(
            bw.tier3().arena().refcount(h),
            Some(1),
            "clone failure incref'd nothing"
        );
        assert!(mb.try_dequeue().is_none(), "clone failure enqueued nothing");
    }

    #[test]
    fn end_to_end_cross_thread_boundary_send_to_affinity_worker() {

        use std::sync::mpsc as std_mpsc;
        use std::thread;

        let mut bw: BoundaryWrapper<i64, &str, String> = BoundaryWrapper::new();
        let mb: Mailbox<Env> = Mailbox::new();
        bw.register_compartment(7, mb.sender());

        let (report_tx, report_rx) = std_mpsc::channel::<(&str, Vec<String>)>();
        let worker = thread::spawn(move || {

            let env = mb
                .recv()
                .expect("worker receives the boundary-sent envelope");
            report_tx.send((env.message, env.capabilities)).unwrap();
        });

        let holder = caps(&["net"]);
        bw.boundary_send(
            7,
            "cross-thread",
            &Crossing::default(),
            &["net".to_string()],
            &holder,
            cloneable,
            id_clone,
        )
        .expect("valid cross-thread send");

        let (msg, gc_caps) = report_rx.recv().expect("worker reported");
        assert_eq!(msg, "cross-thread");
        assert_eq!(
            gc_caps,
            vec!["net".to_string()],
            "capabilities ⊆ capability_set across the thread boundary"
        );
        worker.join().unwrap();
    }
}
