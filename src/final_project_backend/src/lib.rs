use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;

const MAX_VALUE_SIZE: u32 = 5000;

#[derive(CandidType, Deserialize, Debug)]
enum Choice {
    Approve,
    Reject,
    Pass,
}

#[derive(CandidType, Deserialize, Debug)]
enum VoteError {
    AlreadyVoted,
    ProposalIsNotActive,
    NoSuchProposal,
    AccessRejected,
    UpdateError,
}

#[derive(CandidType, Deserialize, Debug)]
struct Proposal {
    description: String,
    approve: u32,
    reject: u32,
    pass: u32,
    is_active: bool,
    voted: Vec<Principal>,
    owner: Principal,
}

#[derive(CandidType, Deserialize, Debug)]
struct CreateProposal {
    description: String,
    is_active: bool,
}

impl Storable for Proposal {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Proposal {
    const MAX_SIZE: u32 = MAX_VALUE_SIZE;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static PROPOSAL_MAP: RefCell<StableBTreeMap<u64, Proposal, Memory>> = RefCell::new(StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0)))));
}

#[ic_cdk::query]
fn get_proposal(key: u64) -> Option<Proposal> {
    PROPOSAL_MAP.with(|p| p.borrow().get(&key))
}

#[ic_cdk::query]
fn get_proposal_count() -> u64 {
    PROPOSAL_MAP.with(|p| p.borrow().len() as u64)
}

#[ic_cdk::update]
fn create_proposal(key: u64, proposal: CreateProposal) -> Option<Proposal> {
    PROPOSAL_MAP.with(|p| {
        p.borrow_mut().insert(
            key,
            Proposal {
                description: proposal.description,
                approve: 0u32,
                reject: 0u32,
                pass: 0u32,
                is_active: proposal.is_active,
                voted: Vec::new(),
                owner: ic_cdk::caller(),
            },
        )
    })
}

#[ic_cdk::update]
fn edit_proposal(key: u64, proposal: CreateProposal) -> Result<(), VoteError> {
    PROPOSAL_MAP.with(|p| {
        if let Some(old_proposal) = p.borrow().get(&key) {
            if old_proposal.owner == ic_cdk::caller() {
                match p.borrow_mut().insert(
                    key,
                    Proposal {
                        description: proposal.description,
                        approve: old_proposal.approve,
                        reject: old_proposal.reject,
                        pass: old_proposal.pass,
                        is_active: proposal.is_active,
                        voted: old_proposal.voted,
                        owner: old_proposal.owner,
                    },
                ) {
                    Some(_) => Ok(()),
                    None => Err(VoteError::UpdateError),
                }
            } else {
                Err(VoteError::AccessRejected)
            }
        } else {
            Err(VoteError::NoSuchProposal)
        }
    })
}

#[ic_cdk::update]
fn end_proposal(key: u64) -> Result<(), VoteError> {
    PROPOSAL_MAP.with(|p| {
        if let Some(mut old_proposal) = p.borrow_mut().get(&key) {
            if old_proposal.owner == ic_cdk::caller() {
                old_proposal.is_active = false;
                match p.borrow_mut().insert(key, old_proposal) {
                    Some(_) => Ok(()),
                    None => Err(VoteError::UpdateError),
                }
            } else {
                Err(VoteError::AccessRejected)
            }
        } else {
            Err(VoteError::NoSuchProposal)
        }
    })
}

#[ic_cdk::update]
fn vote(key: u64, choice: Choice) -> Result<(), VoteError> {
    PROPOSAL_MAP.with(|p| {
        if let Some(mut old_proposal) = p.borrow_mut().get(&key) {
            if old_proposal.is_active {
                if !old_proposal.voted.contains(&ic_cdk::caller()) {
                    match choice {
                        Choice::Approve => old_proposal.approve += 1,
                        Choice::Reject => old_proposal.reject += 1,
                        Choice::Pass => old_proposal.pass += 1,
                    }
                    old_proposal.voted.push(ic_cdk::caller());
                    match p.borrow_mut().insert(key, old_proposal) {
                        Some(_) => Ok(()),
                        None => Err(VoteError::UpdateError),
                    }
                } else {
                    Err(VoteError::AlreadyVoted)
                }
            } else {
                Err(VoteError::ProposalIsNotActive)
            }
        } else {
            Err(VoteError::NoSuchProposal)
        }
    })
}
