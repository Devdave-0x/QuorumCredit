#![allow(unused)]

use soroban_sdk::{contracttype, Address, Vec};

// ── Constants ─────────────────────────────────────────────────────────────────

pub const DEFAULT_YIELD_BPS: i128 = 200;
pub const DEFAULT_SLASH_BPS: i128 = 5000;
pub const DEFAULT_MIN_YIELD_STAKE: i128 = 50;
pub const DEFAULT_REFERRAL_BONUS_BPS: u32 = 100; // 1% of loan amount
pub const MIN_VOUCH_AGE: u64 = 60; // 1 minute
pub const DEFAULT_MAX_VOUCHERS: u32 = 100;
pub const DEFAULT_MIN_LOAN_AMOUNT: i128 = 100_000;
pub const DEFAULT_LOAN_DURATION: u64 = 30 * 24 * 60 * 60;
pub const DEFAULT_MAX_LOAN_TO_STAKE_RATIO: u32 = 150;
pub const DEFAULT_VOUCH_COOLDOWN_SECS: u64 = 24 * 60 * 60; // 24 hours
pub const TIMELOCK_DELAY: u64 = 24 * 60 * 60;
pub const TIMELOCK_EXPIRY: u64 = 72 * 60 * 60;
pub const DEFAULT_ADMIN_TIMELOCK_SECONDS: u64 = 48 * 60 * 60; // 48 hours
// Task 2: Time-weighted yield constants
pub const TIME_WEIGHTED_YIELD_BONUS_THRESHOLD_DAYS: u64 = 90; // 90 days for bonus
pub const TIME_WEIGHTED_YIELD_BONUS_MULTIPLIER: i128 = 12; // 1.2x = 12/10
pub const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
// Task 4: Dispute constants
pub const DEFAULT_DISPUTE_WINDOW_SECS: u64 = 7 * 24 * 60 * 60; // 7 days

// ── Loan Size Tiers ───────────────────────────────────────────────────────────

pub const SMALL_LOAN_THRESHOLD: i128 = 1_000_000;      // 1M stroops
pub const MEDIUM_LOAN_THRESHOLD: i128 = 5_000_000;     // 5M stroops
pub const LARGE_LOAN_THRESHOLD: i128 = 10_000_000;     // 10M stroops
pub const CRITICAL_LOAN_THRESHOLD: i128 = 50_000_000;  // 50M stroops

pub const LARGE_LOAN_DELAY_SECONDS: u64 = 48 * 60 * 60; // 48 hours review period
pub const CANCELLATION_WINDOW_SECONDS: u64 = 60 * 60;   // 1 hour cancellation window
pub const MAX_VOUCH_DEPTH: u32 = 3;                     // max circular vouch depth

// ── Loan Status ───────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoanStatus {
    None,
    Pending,
    Active,
    Repaid,
    Defaulted,
    Cancelled,
}

// ── Loan Purpose Categories ───────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoanCategory {
    Business,
    Education,
    Medical,
    Agriculture,
    Personal,
    Other,
}

// ── Task 1: Granular Pause Flags ─────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PauseFlag {
    None,       // Not paused
    Vouch,      // Pause vouch operations
    LoanRequest, // Pause new loan requests
    Repay,      // Pause repayments
    Slash,      // Pause slash operations
    Withdraw,   // Pause withdrawals
}

impl PauseFlag {
    /// Convert a soroban String to PauseFlag by comparing against known flag names.
    pub fn from_string(env: &soroban_sdk::Env, s: &soroban_sdk::String) -> Option<PauseFlag> {
        if s == &soroban_sdk::String::from_str(env, "vouch") {
            Some(PauseFlag::Vouch)
        } else if s == &soroban_sdk::String::from_str(env, "loan_request") {
            Some(PauseFlag::LoanRequest)
        } else if s == &soroban_sdk::String::from_str(env, "repay") {
            Some(PauseFlag::Repay)
        } else if s == &soroban_sdk::String::from_str(env, "slash") {
            Some(PauseFlag::Slash)
        } else if s == &soroban_sdk::String::from_str(env, "withdraw") {
            Some(PauseFlag::Withdraw)
        } else {
            None
        }
    }
}

// ── Task 4: Dispute Records ──────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct DisputeRecord {
    pub borrower: Address,
    pub loan_id: u64,
    pub evidence_hash: soroban_sdk::String, // IPFS or other evidence reference
    pub disputed_at: u64,                    // timestamp when dispute was filed
    pub resolved: bool,                      // true if dispute has been resolved
    pub resolved_at: Option<u64>,            // timestamp when resolved
    pub resolution: Option<DisputeResolution>, // outcome of the dispute
    pub voters: Vec<Address>,                // vouchers who voted on dispute
    pub approve_votes: i128,                 // stake voting to uphold dispute
    pub reject_votes: i128,                  // stake voting to reject dispute
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeResolution {
    Upheld,   // Dispute valid, slash reversed
    Rejected, // Dispute invalid, slash stands
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Loan(u64),                   // loan_id → LoanRecord
    ActiveLoan(Address),         // borrower → active loan_id
    LatestLoan(Address),         // borrower → latest loan_id
    Vouches(Address),            // borrower → Vec<VouchRecord>
    VoucherHistory(Address),     // voucher → Vec<Address> (borrowers backed)
    Config,                      // Config struct: all configurable protocol parameters
    Deployer,                    // Address that deployed the contract; guards initialize
    SlashTreasury,               // i128 accumulated slashed funds
    Paused,                      // bool: true when contract is paused
    BorrowerList,                // Vec<Address> of all borrowers who have ever requested a loan
    ReputationNft,               // Address of the ReputationNftContract
    MinStake,                    // i128 minimum stake amount per vouch
    MaxLoanAmount,               // i128 maximum individual loan size (0 = no cap)
    MinVouchers,     // u32 minimum number of distinct vouchers required (0 = no minimum)
    LoanCounter,     // u64: monotonically increasing loan ID counter
    LoanPool(u64),   // pool_id → LoanPoolRecord
    LoanPoolCounter, // u64: monotonically increasing pool ID counter
    PendingAdmin,    // Address of the pending admin (two-step transfer)
    RepaymentCount(Address), // borrower → u32 total successful repayments
    LoanCount(Address), // borrower → u32 total historical loans disbursed
    DefaultCount(Address), // borrower → u32 total defaults (slash + auto_slash + claim_expired)
    ProtocolFeeBps,  // u32: protocol fee in basis points
    FeeTreasury,     // Address: recipient of collected protocol fees
    LastVouchTimestamp(Address), // voucher → u64 last vouch timestamp
    Timelock(u64),   // proposal_id → TimelockProposal
    TimelockCounter, // u64 monotonically increasing proposal ID
    Blacklisted(Address), // borrower → bool permanently banned
    VoucherWhitelist(Address), // voucher → bool allowed to vouch
    VoucherWhitelistEnabled, // bool: true when voucher whitelist is enforced
    BorrowerWhitelist(Address), // borrower → bool allowed to request loans
    BorrowerWhitelistEnabled, // bool: true when borrower whitelist is enforced
    TokenConfig(Address), // token → TokenConfig (per-token yield/slash overrides)
    ExtensionConsents(Address), // borrower → Vec<Address> vouchers who consented to extension
    SlashVote(Address), // borrower → SlashVoteRecord
    SlashVoteQuorum, // u32 quorum in basis points (e.g. 5000 = 50%)
    ReferredBy(Address), // borrower → Address of referrer
    ReferralBonusBps, // u32 referral bonus in basis points (default 100 = 1%)
    AdminAuditLog,   // Vec<AdminAuditEntry> audit log of all admin actions
    AdminKeyExpiry(Address), // admin → u64 expiry timestamp (0 = no expiry)
    GovernanceToken, // Address of governance token for voting
    GovernanceProposal(u64), // proposal_id → GovernanceProposal
    GovernanceProposalCounter, // u64 monotonically increasing proposal ID
    AdminActionTimelock(u64), // action_id → AdminTimelockAction
    AdminActionTimelockCounter, // u64 monotonically increasing action ID
    GovernanceTokenAddress, // Address of governance token for voting
    LargeLoanApproval(Address), // borrower → LargeLoanApprovalRecord
    LargeLoanRequest(Address), // borrower → LargeLoanRequestRecord
    VouchGraph(Address, Address), // (voucher, borrower) → depth u32
    LoanCategoryLoans(LoanCategory), // category → Vec<loan_id>
    // #634: Liquidity Mining
    LastMiningClaim(Address),    // voucher → u64 timestamp of last mining reward claim
    // #635: Vouch Snapshot for Governance
    VouchSnapshot(u32),          // ledger_sequence → VouchSnapshotRecord
    // #636: Staking Derivatives
    StakingDerivative(Address, Address), // (voucher, borrower) → StakingDerivativeRecord
    DerivativeTransfer(Address, Address, Address), // (from, to, borrower) → bool (pending transfer)
    // #637: Fraud Detection
    VoucherFraudScore(Address),  // voucher → u32 fraud score (0-100)
}

// ── Audit Log ─────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct AdminAuditEntry {
    pub admin: Address,
    pub action: soroban_sdk::String,
    pub timestamp: u64,
}

// ── Governance ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct SlashVoteRecord {
    pub approve_stake: i128,  // total stake voting to approve slash
    pub reject_stake: i128,   // total stake voting to reject slash
    pub voters: Vec<Address>, // addresses that have already voted
    pub executed: bool,       // true once slash has been auto-executed
}

#[contracttype]
#[derive(Clone)]
pub struct GovernanceProposal {
    pub id: u64,
    pub proposer: Address,
    pub description: soroban_sdk::String,
    pub approve_votes: i128,
    pub reject_votes: i128,
    pub voters: Vec<Address>,
    pub voting_end: u64,
    pub executed: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum AdminTimelockAction {
    Pause,
    Unpause,
    UpdateConfig(Config),
    SetAdminThreshold(u32),
}

#[contracttype]
#[derive(Clone)]
pub struct AdminTimelock {
    pub id: u64,
    pub action: AdminTimelockAction,
    pub proposer: Address,
    pub eta: u64,
    pub executed: bool,
    pub cancelled: bool,
}

// ── Config ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admins: Vec<Address>,
    pub admin_threshold: u32,
    pub token: Address,
    pub allowed_tokens: Vec<Address>, // additional tokens accepted for loans/vouches
    pub yield_bps: i128,
    pub slash_bps: i128,
    pub max_vouchers: u32,
    pub min_loan_amount: i128,
    pub loan_duration: u64,
    pub max_loan_to_stake_ratio: u32,
    pub grace_period: u64,
    /// #634: Liquidity mining reward rate in basis points (e.g. 100 = 1% per epoch).
    /// Vouchers earn this rate on their staked amount per mining epoch.
    pub liquidity_mining_rate_bps: u32,
}

// ── Per-Token Config ──────────────────────────────────────────────────────────

/// Per-token overrides for yield and slash parameters.
/// When set, these values take precedence over the global `Config` values.
#[contracttype]
#[derive(Clone)]
pub struct TokenConfig {
    pub yield_bps: i128,
    pub slash_bps: i128,
}

// ── #649 Loan Subordination ───────────────────────────────────────────────────

/// Priority level for loan subordination (senior loans are repaid first on default).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoanPriority {
    Senior,        // Repaid first in liquidation
    Subordinated,  // Repaid after senior obligations
}

// ── #648 Milestone-Based Disbursement ────────────────────────────────────────

/// Status of an individual disbursement milestone.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MilestoneStatus {
    Pending,   // Not yet released
    Released,  // Funds disbursed for this milestone
}

/// A single milestone in a tranche-based disbursement schedule.
#[contracttype]
#[derive(Clone)]
pub struct Milestone {
    pub amount: i128,              // tranche amount in stroops
    pub release_timestamp: u64,   // earliest timestamp this tranche can be released
    pub status: MilestoneStatus,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct LoanRecord {
    pub id: u64,
    pub borrower: Address,
    pub co_borrowers: Vec<Address>,
    pub amount: i128,        // total loan principal in stroops
    pub amount_repaid: i128, // cumulative repayments received so far (principal + yield)
    pub total_yield: i128,   // yield owed to vouchers, locked in at disbursement
    pub yield_bps: i128,     // effective yield rate (risk-adjusted) at disbursement (#646)
    pub slash_bps: i128,     // effective slash rate (risk-adjusted) at disbursement (#646)
    pub status: LoanStatus,
    pub created_at: u64,                   // ledger timestamp
    pub disbursement_timestamp: u64,       // ledger timestamp
    pub repayment_timestamp: Option<u64>,  // set once the loan is fully repaid
    pub deadline: u64,                     // repayment deadline (ledger timestamp)
    pub loan_purpose: soroban_sdk::String, // borrower-supplied purpose string
    pub loan_category: LoanCategory,       // category of the loan
    pub token_address: Address,            // token used for this loan
    pub syndicate_id: Option<u64>,         // syndicate pool ID if syndicated (#647)
}

#[contracttype]
#[derive(Clone)]
pub struct LargeLoanApprovalRecord {
    pub borrower: Address,
    pub amount: i128,
    pub approved_by: Vec<Address>, // admins who approved
    pub approval_timestamp: u64,
    pub executed: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct LargeLoanRequestRecord {
    pub borrower: Address,
    pub amount: i128,
    pub requested_at: u64,
    pub token_address: Address,
    pub threshold: i128,
    pub loan_purpose: soroban_sdk::String,
    pub loan_category: LoanCategory,
}

#[contracttype]
#[derive(Clone)]
pub struct VouchRecord {
    pub voucher: Address,
    pub amount: i128,         // in stroops
    pub vouch_timestamp: u64, // ledger timestamp when vouch was created; immutable after set
    pub token: Address,       // token this stake is denominated in
    pub sector: soroban_sdk::String, // #642: sector/region of the voucher for diversification
}

#[contracttype]
#[derive(Clone)]
pub struct LoanPoolRecord {
    pub pool_id: u64,
    pub borrowers: Vec<Address>,
    pub amounts: Vec<i128>,
    pub created_at: u64,
    pub total_disbursed: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct TimelockProposal {
    pub id: u64,
    pub action: TimelockAction,
    pub proposer: Address,
    pub eta: u64,
    pub executed: bool,
    pub cancelled: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum TimelockAction {
    Slash(Address),
    SetConfig(Config),
}

// ── #634: Liquidity Mining ────────────────────────────────────────────────────

/// Epoch duration for liquidity mining rewards (7 days).
pub const LIQUIDITY_MINING_EPOCH_SECS: u64 = 7 * 24 * 60 * 60;
/// Default liquidity mining rate: 50 bps = 0.5% per epoch.
pub const DEFAULT_LIQUIDITY_MINING_RATE_BPS: u32 = 50;

// ── #635: Vouch Snapshot ──────────────────────────────────────────────────────

/// A snapshot of a borrower's total vouched stake at a given ledger sequence.
/// Used for governance voting weight calculations.
#[contracttype]
#[derive(Clone)]
pub struct VouchSnapshotEntry {
    pub borrower: Address,
    pub total_stake: i128,
}

/// All vouch snapshots captured at a given ledger sequence.
#[contracttype]
#[derive(Clone)]
pub struct VouchSnapshotRecord {
    pub ledger_sequence: u32,
    pub timestamp: u64,
    pub entries: Vec<VouchSnapshotEntry>,
}

// ── #636: Staking Derivatives ─────────────────────────────────────────────────

/// Represents a derivative token minted by a voucher against their stake.
/// The derivative can be transferred to enable secondary market trading.
#[contracttype]
#[derive(Clone)]
pub struct StakingDerivativeRecord {
    pub voucher: Address,       // original voucher who minted the derivative
    pub borrower: Address,      // borrower whose vouch backs this derivative
    pub stake_amount: i128,     // underlying stake amount in stroops
    pub minted_at: u64,         // timestamp when derivative was minted
    pub current_holder: Address, // current holder (may differ from original voucher after transfer)
    pub is_active: bool,        // false once redeemed or underlying vouch withdrawn
}

// ── #637: Fraud Detection ─────────────────────────────────────────────────────

/// Fraud score thresholds.
pub const FRAUD_SCORE_HIGH_THRESHOLD: u32 = 70;
pub const FRAUD_SCORE_MAX: u32 = 100;
/// Weight per defaulted borrower backed (out of 100 total score).
pub const FRAUD_SCORE_DEFAULT_WEIGHT: u32 = 20;
/// Weight for vouching many borrowers simultaneously.
pub const FRAUD_SCORE_CONCENTRATION_WEIGHT: u32 = 10;
