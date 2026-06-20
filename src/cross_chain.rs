//! Cross-chain loan portability through bridge-signed attestations.
//!
//! Bridge operators sign [`BridgeAttestationPayload`] off-chain. The destination
//! contract verifies the Ed25519 signature and consumes the nonce in the same
//! transaction that stores the mirrored loan and unified reputation.

use crate::{helpers, ContractError, PaymentRecord};
use soroban_sdk::{contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env, Vec};

/// Attestation freshness window. Old attestations are rejected even when signed.
pub const MAX_ATTESTATION_AGE_SECS: u64 = 60 * 60;
/// Small allowance for clock differences between bridge and destination ledgers.
pub const MAX_FUTURE_SKEW_SECS: u64 = 60;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeAttestation {
    pub signature: BytesN<64>,
    pub nonce: u64,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossChainLoanMetadata {
    pub loan_id: u64,
    pub borrower: Address,
    pub origin_chain: u32,
    pub destination_chain: u32,
    pub repayment_history: Vec<PaymentRecord>,
    pub defaults: u32,
    pub reputation_score: u32,
}

/// Exact, domain-separated value signed by the configured bridge key.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeAttestationPayload {
    pub contract: Address,
    pub metadata: CrossChainLoanMetadata,
    pub nonce: u64,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnifiedReputation {
    pub borrower: Address,
    pub score: u32,
    pub successful_repayments: u32,
    pub defaults: u32,
    pub authoritative_chain: u32,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum CrossChainKey {
    BridgeKey(u32),
    UsedNonce(u32, u64),
    MirroredLoan(u32, u64),
    LoanSettlement(u32, u64),
    Reputation(Address),
    ReputationVersion(Address),
}

/// Configure or rotate an origin chain's Stellar bridge verification key.
#[allow(deprecated)]
pub fn set_bridge_public_key(
    env: Env,
    admin_signers: Vec<Address>,
    origin_chain: u32,
    public_key: BytesN<32>,
) -> Result<(), ContractError> {
    helpers::require_admin_approval(&env, &admin_signers);
    if origin_chain == 0 {
        return Err(ContractError::InvalidBridgeChain);
    }
    env.storage()
        .persistent()
        .set(&CrossChainKey::BridgeKey(origin_chain), &public_key);
    env.events().publish(
        (
            soroban_sdk::symbol_short!("bridge"),
            soroban_sdk::symbol_short!("key_set"),
        ),
        origin_chain,
    );
    Ok(())
}

/// Return the canonical bytes that the bridge must sign.
pub fn bridge_attestation_message(
    env: &Env,
    metadata: &CrossChainLoanMetadata,
    nonce: u64,
    timestamp: u64,
) -> Bytes {
    BridgeAttestationPayload {
        contract: env.current_contract_address(),
        metadata: metadata.clone(),
        nonce,
        timestamp,
    }
    .to_xdr(env)
}

fn verify_bridge_attestation(
    env: &Env,
    metadata: &CrossChainLoanMetadata,
    attestation: &BridgeAttestation,
) -> Result<(), ContractError> {
    if metadata.origin_chain == 0 || metadata.origin_chain == metadata.destination_chain {
        return Err(ContractError::InvalidBridgeChain);
    }

    let now = env.ledger().timestamp();
    if attestation.timestamp > now.saturating_add(MAX_FUTURE_SKEW_SECS) {
        return Err(ContractError::AttestationFromFuture);
    }
    if now.saturating_sub(attestation.timestamp) > MAX_ATTESTATION_AGE_SECS {
        return Err(ContractError::AttestationExpired);
    }

    let nonce_key = CrossChainKey::UsedNonce(metadata.origin_chain, attestation.nonce);
    if env.storage().persistent().has(&nonce_key) {
        return Err(ContractError::ReplayAttackDetected);
    }

    let public_key: BytesN<32> = env
        .storage()
        .persistent()
        .get(&CrossChainKey::BridgeKey(metadata.origin_chain))
        .ok_or(ContractError::BridgeNotConfigured)?;
    let message =
        bridge_attestation_message(env, metadata, attestation.nonce, attestation.timestamp);

    // Soroban aborts the invocation on an invalid Ed25519 signature. Because
    // nonce/state writes occur afterwards, signature failure cannot consume a
    // nonce or leave partial mirrored state.
    env.crypto()
        .ed25519_verify(&public_key, &message, &attestation.signature);
    Ok(())
}

/// Verify an attestation and consume its nonce.
pub fn validate_bridge_attestation(
    env: Env,
    metadata: CrossChainLoanMetadata,
    attestation: BridgeAttestation,
) -> Result<(), ContractError> {
    verify_bridge_attestation(&env, &metadata, &attestation)?;
    env.storage().persistent().set(
        &CrossChainKey::UsedNonce(metadata.origin_chain, attestation.nonce),
        &true,
    );
    Ok(())
}

/// Atomically verify and import a loan, consuming both its nonce and its
/// canonical `(origin_chain, loan_id)` settlement slot.
#[allow(deprecated)]
pub fn mirror_loan_to_chain(
    env: Env,
    metadata: CrossChainLoanMetadata,
    attestation: BridgeAttestation,
) -> Result<(), ContractError> {
    verify_bridge_attestation(&env, &metadata, &attestation)?;

    let settlement_key = CrossChainKey::LoanSettlement(metadata.origin_chain, metadata.loan_id);
    if env.storage().persistent().has(&settlement_key) {
        return Err(ContractError::ReputationAlreadySpent);
    }

    let version_key = CrossChainKey::ReputationVersion(metadata.borrower.clone());
    let current_version: u64 = env.storage().persistent().get(&version_key).unwrap_or(0);
    if current_version >= attestation.timestamp && current_version != 0 {
        return Err(ContractError::StaleBridgeAttestation);
    }

    let successful_repayments = metadata.repayment_history.len();
    let reputation = UnifiedReputation {
        borrower: metadata.borrower.clone(),
        score: metadata.reputation_score,
        successful_repayments,
        defaults: metadata.defaults,
        authoritative_chain: metadata.destination_chain,
        updated_at: attestation.timestamp,
    };

    // These writes are one Soroban transaction: any failure rolls all of them back.
    env.storage().persistent().set(
        &CrossChainKey::UsedNonce(metadata.origin_chain, attestation.nonce),
        &true,
    );
    env.storage().persistent().set(
        &CrossChainKey::MirroredLoan(metadata.origin_chain, metadata.loan_id),
        &metadata,
    );
    env.storage()
        .persistent()
        .set(&settlement_key, &metadata.destination_chain);
    env.storage().persistent().set(
        &CrossChainKey::Reputation(metadata.borrower.clone()),
        &reputation,
    );
    env.storage()
        .persistent()
        .set(&version_key, &attestation.timestamp);

    env.events().publish(
        (
            soroban_sdk::symbol_short!("bridge"),
            soroban_sdk::symbol_short!("settled"),
        ),
        (
            metadata.origin_chain,
            metadata.destination_chain,
            metadata.loan_id,
            metadata.borrower,
            attestation.nonce,
        ),
    );
    Ok(())
}

pub fn query_reputation_cross_chain(env: Env, borrower: Address) -> Option<UnifiedReputation> {
    env.storage()
        .persistent()
        .get(&CrossChainKey::Reputation(borrower))
}

pub fn query_mirrored_loan(
    env: Env,
    origin_chain: u32,
    loan_id: u64,
) -> Option<CrossChainLoanMetadata> {
    env.storage()
        .persistent()
        .get(&CrossChainKey::MirroredLoan(origin_chain, loan_id))
}

pub fn is_bridge_nonce_used(env: Env, origin_chain: u32, nonce: u64) -> bool {
    env.storage()
        .persistent()
        .has(&CrossChainKey::UsedNonce(origin_chain, nonce))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use soroban_sdk::{
        contract,
        testutils::{Address as _, Ledger},
        Address,
    };

    #[contract]
    struct StorageContract;

    struct Fixture {
        env: Env,
        contract: Address,
        signer: SigningKey,
        borrower: Address,
    }

    impl Fixture {
        fn new() -> Self {
            let env = Env::default();
            env.ledger().set_timestamp(10_000);
            let contract = env.register(StorageContract, ());
            let signer = SigningKey::from_bytes(&[7; 32]);
            let borrower = Address::generate(&env);
            let public_key = BytesN::from_array(&env, signer.verifying_key().as_bytes());
            env.as_contract(&contract, || {
                env.storage()
                    .persistent()
                    .set(&CrossChainKey::BridgeKey(1), &public_key);
            });
            Self {
                env,
                contract,
                signer,
                borrower,
            }
        }

        fn metadata(&self, loan_id: u64) -> CrossChainLoanMetadata {
            let history = Vec::from_array(
                &self.env,
                [PaymentRecord {
                    amount: 500,
                    timestamp: 9_000,
                    cumulative_repaid: 500,
                }],
            );
            CrossChainLoanMetadata {
                loan_id,
                borrower: self.borrower.clone(),
                origin_chain: 1,
                destination_chain: 2,
                repayment_history: history,
                defaults: 0,
                reputation_score: 720,
            }
        }

        fn sign(
            &self,
            metadata: &CrossChainLoanMetadata,
            nonce: u64,
            timestamp: u64,
        ) -> BridgeAttestation {
            let message = self.env.as_contract(&self.contract, || {
                bridge_attestation_message(&self.env, metadata, nonce, timestamp)
            });
            let signature = self.signer.sign(&message.to_alloc_vec()).to_bytes();
            BridgeAttestation {
                signature: BytesN::from_array(&self.env, &signature),
                nonce,
                timestamp,
            }
        }

        fn mirror(
            &self,
            metadata: CrossChainLoanMetadata,
            attestation: BridgeAttestation,
        ) -> Result<(), ContractError> {
            self.env.as_contract(&self.contract, || {
                mirror_loan_to_chain(self.env.clone(), metadata, attestation)
            })
        }
    }

    #[test]
    fn valid_signature_succeeds() {
        let f = Fixture::new();
        let metadata = f.metadata(10);
        let attestation = f.sign(&metadata, 1, 10_000);
        let result = f.env.as_contract(&f.contract, || {
            validate_bridge_attestation(f.env.clone(), metadata, attestation)
        });
        assert_eq!(result, Ok(()));
    }

    #[test]
    #[should_panic]
    fn invalid_signature_fails() {
        let f = Fixture::new();
        let metadata = f.metadata(10);
        let mut attestation = f.sign(&metadata, 1, 10_000);
        attestation.signature = BytesN::from_array(&f.env, &[0; 64]);
        let _ = f.env.as_contract(&f.contract, || {
            validate_bridge_attestation(f.env.clone(), metadata, attestation)
        });
    }

    #[test]
    fn replayed_nonce_is_rejected() {
        let f = Fixture::new();
        let metadata = f.metadata(10);
        let attestation = f.sign(&metadata, 42, 10_000);
        assert_eq!(
            f.env
                .as_contract(&f.contract, || validate_bridge_attestation(
                    f.env.clone(),
                    metadata.clone(),
                    attestation.clone()
                )),
            Ok(())
        );
        assert_eq!(
            f.env
                .as_contract(&f.contract, || validate_bridge_attestation(
                    f.env.clone(),
                    metadata,
                    attestation
                )),
            Err(ContractError::ReplayAttackDetected)
        );
    }

    #[test]
    fn loan_is_mirrored_to_destination_chain() {
        let f = Fixture::new();
        let metadata = f.metadata(11);
        assert_eq!(
            f.mirror(metadata.clone(), f.sign(&metadata, 2, 10_000)),
            Ok(())
        );
        let mirrored = f
            .env
            .as_contract(&f.contract, || query_mirrored_loan(f.env.clone(), 1, 11));
        assert_eq!(mirrored, Some(metadata));
    }

    #[test]
    fn bridge_failure_does_not_consume_nonce() {
        let f = Fixture::new();
        let mut metadata = f.metadata(12);
        metadata.origin_chain = 9;
        let unsigned = BridgeAttestation {
            signature: BytesN::from_array(&f.env, &[0; 64]),
            nonce: 3,
            timestamp: 10_000,
        };
        assert_eq!(
            f.mirror(metadata, unsigned),
            Err(ContractError::BridgeNotConfigured)
        );
        assert!(!f
            .env
            .as_contract(&f.contract, || is_bridge_nonce_used(f.env.clone(), 9, 3)));
    }

    #[test]
    fn settlement_updates_all_state_together() {
        let f = Fixture::new();
        let metadata = f.metadata(13);
        assert_eq!(
            f.mirror(metadata.clone(), f.sign(&metadata, 4, 10_000)),
            Ok(())
        );
        assert!(f
            .env
            .as_contract(&f.contract, || is_bridge_nonce_used(f.env.clone(), 1, 4)));
        assert!(f
            .env
            .as_contract(&f.contract, || query_mirrored_loan(f.env.clone(), 1, 13))
            .is_some());
        assert!(f
            .env
            .as_contract(&f.contract, || query_reputation_cross_chain(
                f.env.clone(),
                f.borrower.clone()
            ))
            .is_some());
    }

    #[test]
    fn canonical_loan_cannot_spend_reputation_twice() {
        let f = Fixture::new();
        let metadata = f.metadata(14);
        assert_eq!(
            f.mirror(metadata.clone(), f.sign(&metadata, 5, 10_000)),
            Ok(())
        );
        assert_eq!(
            f.mirror(metadata.clone(), f.sign(&metadata, 6, 10_001)),
            Err(ContractError::ReputationAlreadySpent)
        );
    }

    #[test]
    fn unified_reputation_is_queryable() {
        let f = Fixture::new();
        let metadata = f.metadata(15);
        assert_eq!(
            f.mirror(metadata.clone(), f.sign(&metadata, 7, 10_000)),
            Ok(())
        );
        let rep = f
            .env
            .as_contract(&f.contract, || {
                query_reputation_cross_chain(f.env.clone(), f.borrower.clone())
            })
            .unwrap();
        assert_eq!(rep.score, 720);
        assert_eq!(rep.successful_repayments, 1);
        assert_eq!(rep.defaults, 0);
        assert_eq!(rep.authoritative_chain, 2);
    }

    #[test]
    fn defaults_are_mirrored() {
        let f = Fixture::new();
        let mut metadata = f.metadata(16);
        metadata.defaults = 3;
        metadata.reputation_score = 410;
        assert_eq!(
            f.mirror(metadata.clone(), f.sign(&metadata, 8, 10_000)),
            Ok(())
        );
        let rep = f
            .env
            .as_contract(&f.contract, || {
                query_reputation_cross_chain(f.env.clone(), f.borrower.clone())
            })
            .unwrap();
        assert_eq!((rep.defaults, rep.score), (3, 410));
    }

    #[test]
    fn expired_attestation_is_rejected() {
        let f = Fixture::new();
        let metadata = f.metadata(17);
        let attestation = f.sign(&metadata, 9, 10_000 - MAX_ATTESTATION_AGE_SECS - 1);
        assert_eq!(
            f.mirror(metadata, attestation),
            Err(ContractError::AttestationExpired)
        );
    }

    #[test]
    fn future_attestation_is_rejected() {
        let f = Fixture::new();
        let metadata = f.metadata(18);
        let attestation = f.sign(&metadata, 10, 10_000 + MAX_FUTURE_SKEW_SECS + 1);
        assert_eq!(
            f.mirror(metadata, attestation),
            Err(ContractError::AttestationFromFuture)
        );
    }

    #[test]
    fn same_origin_and_destination_is_rejected() {
        let f = Fixture::new();
        let mut metadata = f.metadata(19);
        metadata.destination_chain = 1;
        let attestation = f.sign(&metadata, 11, 10_000);
        assert_eq!(
            f.mirror(metadata, attestation),
            Err(ContractError::InvalidBridgeChain)
        );
    }

    #[test]
    fn zero_origin_chain_is_rejected() {
        let f = Fixture::new();
        let mut metadata = f.metadata(20);
        metadata.origin_chain = 0;
        let attestation = f.sign(&metadata, 12, 10_000);
        assert_eq!(
            f.mirror(metadata, attestation),
            Err(ContractError::InvalidBridgeChain)
        );
    }

    #[test]
    fn nonce_scope_is_per_origin_chain() {
        let f = Fixture::new();
        let public_key = BytesN::from_array(&f.env, f.signer.verifying_key().as_bytes());
        f.env.as_contract(&f.contract, || {
            f.env
                .storage()
                .persistent()
                .set(&CrossChainKey::BridgeKey(3), &public_key)
        });
        let first = f.metadata(21);
        assert_eq!(f.mirror(first.clone(), f.sign(&first, 13, 10_000)), Ok(()));
        let mut second = f.metadata(22);
        second.origin_chain = 3;
        assert_eq!(
            f.mirror(second.clone(), f.sign(&second, 13, 10_001)),
            Ok(())
        );
    }

    #[test]
    fn different_loans_can_be_mirrored() {
        let f = Fixture::new();
        let first = f.metadata(23);
        let second = f.metadata(24);
        assert_eq!(f.mirror(first.clone(), f.sign(&first, 14, 10_000)), Ok(()));
        assert_eq!(
            f.mirror(second.clone(), f.sign(&second, 15, 10_001)),
            Ok(())
        );
    }

    #[test]
    fn stale_reputation_update_is_rejected() {
        let f = Fixture::new();
        let first = f.metadata(25);
        assert_eq!(f.mirror(first.clone(), f.sign(&first, 16, 10_000)), Ok(()));
        let second = f.metadata(26);
        assert_eq!(
            f.mirror(second.clone(), f.sign(&second, 17, 9_999)),
            Err(ContractError::StaleBridgeAttestation)
        );
    }

    #[test]
    #[should_panic]
    fn signature_is_bound_to_metadata() {
        let f = Fixture::new();
        let signed = f.metadata(27);
        let attestation = f.sign(&signed, 18, 10_000);
        let mut tampered = signed;
        tampered.reputation_score = 999;
        let _ = f.mirror(tampered, attestation);
    }

    #[test]
    fn missing_reputation_returns_none() {
        let f = Fixture::new();
        let unknown = Address::generate(&f.env);
        assert_eq!(
            f.env
                .as_contract(&f.contract, || query_reputation_cross_chain(
                    f.env.clone(),
                    unknown
                )),
            None
        );
    }
}
