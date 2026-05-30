#![allow(unused)]

//! # Stroop Unit Convention
//!
//! **All monetary amounts in this contract are denominated in stroops.**
//!
//! | Unit  | Value                      |
//! |-------|----------------------------|
//! | 1 XLM | 10,000,000 stroops         |
//! | 1 stroop | 0.0000001 XLM           |
//!
//! This applies to every `i128` field or parameter that represents a token
//! amount (stakes, loan principals, yield, fees, minimums, etc.).
//! When displaying values to end-users, divide by `10_000_000` to convert
//! to XLM. When accepting user input in XLM, multiply by `10_000_000`
//! before passing to contract functions.

use soroban_sdk::{contracttype, Address, Vec};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Yield earned by vouchers on full repayment, in basis points (200 = 2%).
pub const DEFAULT_YIELD_BPS: i128 = 200;
/// Fraction of stake burned when a borrower defaults, in basis points (5000 = 50%).
pub const DEFAULT_SLASH_BPS: i128 = 5000;
/// Basis-point denominator (10_000 = 100%).
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Minimum stake amount, in stroops (50 stroops), required for non-zero yield at
/// the default 2% rate. Amounts below this truncate to zero yield.
/// 1 XLM = 10,000,000 stroops.
pub const DEFAULT_MIN_YIELD_STAKE: i128 = 50;
/// Referral bonus paid to the referrer on full repayment, in basis points (100 = 1% of loan amount).
pub const DEFAULT_REFERRAL_BONUS_BPS: u32 = 100; // 1% of loan amount
/// Minimum age of a vouch before it can be used for a loan, in seconds (60 = 1 minute).
pub const MIN_VOUCH_AGE: u64 = 60; // 1 minute
/// Default minimum vouch age before loan eligibility, in seconds (24 hours).
pub const DEFAULT_MIN_VOUCH_AGE_SECS: u64 = 24 * 60 * 60;
/// Default maximum number of distinct vouchers per borrower.
pub const DEFAULT_MAX_VOUCHERS: u32 = 100;
/// Default minimum loan amount, in stroops (100,000 stroops = 0.01 XLM).
/// 1 XLM = 10,000,000 stroops.
pub const DEFAULT_MIN_LOAN_AMOUNT: i128 = 100_000;
/// Default loan duration, in seconds (30 days).
pub const DEFAULT_LOAN_DURATION: u64 = 30 * 24 * 60 * 60;
/// Default maximum loan-to-stake ratio (150 = 150% — loan ≤ 1.5× total staked).
pub const DEFAULT_MAX_LOAN_TO_STAKE_RATIO: u32 = 150;
/// Minimum elapsed time between vouch calls from the same address, in seconds (24 hours).
pub const DEFAULT_VOUCH_COOLDOWN_SECS: u64 = 24 * 60 * 60; // 24 hours
/// Default maximum number of vouchers that may back a single borrower.
pub const DEFAULT_MAX_VOUCHERS_PER_BORROWER: u32 = 50;
/// Default governance voting period for slash-threshold proposals, in seconds (7 days).
pub const DEFAULT_VOTING_PERIOD_SECONDS: u64 = 7 * 24 * 60 * 60;
/// Minimum delay before a timelocked governance action may be executed, in seconds (24 hours).
pub const TIMELOCK_DELAY: u64 = 24 * 60 * 60;
/// Maximum window after `eta` within which a timelocked action must be executed, in seconds (72 hours).
pub const TIMELOCK_EXPIRY: u64 = 72 * 60 * 60;
/// Minimum lock period for a vouch before it can be withdrawn, in seconds (7 days).
/// Protects against flash-loan-style attacks where an attacker stakes, borrows, then
/// immediately withdraws.
pub const MIN_VOUCH_LOCK_PERIOD: u64 = 7 * 24 * 60 * 60;

/// Fraction of slashed funds routed to the insurance pool (2000 = 20%).
pub const SLASH_TO_INSURANCE_BPS: u32 = 2_000;
/// Default insurance fee on loan disbursement (50 = 0.5%).
pub const DEFAULT_INSURANCE_FEE_BPS: u32 = 50;
/// Default max insurance payout as % of slashed stake (2500 = 25%).
pub const DEFAULT_INSURANCE_COVERAGE_BPS: u32 = 2_500;

/// Extension fee charged when a borrower requests a loan extension, in basis points (100 = 1%).
pub const EXTENSION_FEE_BPS: i128 = 100;

/// Maximum number of extensions allowed per loan.
pub const MAX_EXTENSIONS_PER_LOAN: u32 = 2;

/// Default liquidity mining reward rate in basis points per epoch (50 = 0.5% per 7 days).
pub const DEFAULT_LIQUIDITY_MINING_RATE_BPS: u32 = 50;

/// Default dynamic slash threshold setting (false = disabled by default).
pub const DEFAULT_DYNAMIC_SLASH_THRESHOLD: bool = false;

/// Default loan-size-based slash scaling setting (false = disabled by default).
pub const DEFAULT_LOAN_SIZE_SLASH_ENABLED: bool = false;

/// Default maximum slash rate for the largest loans, in basis points (8000 = 80%).
/// When loan-size scaling is enabled, slash_bps is the floor (small loans) and
/// this is the ceiling (loans at or above the total staked collateral).
pub const DEFAULT_LOAN_SIZE_SLASH_MAX_BPS: i128 = 8_000;

/// Default borrower repayment confirmation requirement (false = disabled by default).
pub const DEFAULT_CONFIRMATION_REQUIRED: bool = false;
/// Timelock delay for decrease_stake during an active loan, in seconds (7 days).
pub const DECREASE_STAKE_TIMELOCK: u64 = 7 * 24 * 60 * 60;

/// Withdrawal request timelock delay, in seconds (24 hours).
pub const WITHDRAWAL_TIMELOCK_DELAY: u64 = 24 * 60 * 60;

/// Maximum number of deferment periods allowed per loan.
pub const MAX_DEFERMENT_PERIODS: u32 = 3;

/// Duration of each deferment period, in seconds (30 days).
pub const DEFERMENT_PERIOD_SECS: u64 = 30 * 24 * 60 * 60;

/// Penalty applied to partial mid-loan withdrawals, in basis points (1000 = 10%).
pub const PARTIAL_WITHDRAWAL_PENALTY_BPS: i128 = 1_000;

/// Maximum fraction of stake that can be partially withdrawn during an active loan (50%).
pub const PARTIAL_WITHDRAWAL_MAX_BPS: i128 = 5_000;

/// Minimum slash threshold when protocol health is excellent, in basis points (2500 = 25%).
pub const MIN_DYNAMIC_SLASH_BPS: i128 = 2_500;

/// Maximum slash threshold when protocol health is poor, in basis points (7500 = 75%).
pub const MAX_DYNAMIC_SLASH_BPS: i128 = 7_500;

/// Health threshold below which slash penalty increases, in basis points (8000 = 80%).
pub const HEALTH_THRESHOLD_BPS: i128 = 8_000;

/// Default slash delay period to allow for disputes, in seconds (7 days).
pub const DEFAULT_SLASH_DELAY_SECONDS: u64 = 7 * 24 * 60 * 60;

/// Duration of one reporting month, in seconds (30 days).
pub const MONTHLY_PERIOD_SECS: u64 = 30 * 24 * 60 * 60;

/// Default premium rate for slashing insurance opt-in, in basis points (100 = 1%).
pub const DEFAULT_INSURANCE_PREMIUM_BPS: u32 = 100;

// ── Loan Extension ────────────────────────────────────────────────────────────

/// A pending loan extension request. Created by the borrower; approved by vouchers.
#[contracttype]
#[derive(Clone)]
pub struct LoanExtensionRequest {
    /// The borrower requesting the extension.
    pub borrower: Address,
    /// Loan ID being extended.
    pub loan_id: u64,
    /// Requested additional duration in seconds.
    pub extension_secs: u64,
    /// Timestamp when the request was created.
    pub requested_at: u64,
    /// Vouchers who have approved this extension.
    pub approvals: Vec<Address>,
    /// Extension fee paid (in stroops), deducted from borrower on approval.
    pub fee_paid: i128,
    /// How many times this loan has already been extended.
    pub extension_count: u32,
}
/// Slash escrow period before funds are permanently burned, in seconds (30 days).
pub const SLASH_ESCROW_PERIOD: u64 = 30 * 24 * 60 * 60;

// ── Escrow Status ─────────────────────────────────────────────────────────────

/// Status of a repayment held in oracle-verified escrow (#666/#667).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    /// No escrow — repayment released immediately (default).
    None,
    /// Repayment held pending oracle verification.
    Pending,
    /// Oracle approved — funds released to vouchers.
    Released,
    /// Oracle rejected — funds returned to borrower.
    Rejected,
}

// ── Loan Status ───────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoanStatus {
    None,
    Active,
    Repaid,
    /// #663: Borrower repaid some but less than partial_default_threshold_bps of total owed.
    PartialDefault,
    Defaulted,
    /// #664: Default was forgiven by admin.
    ForgivenDefault,
}

/// Interest rate type for a loan.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RateType {
    /// Fixed rate locked at disbursement (yield_bps from Config).
    Fixed,
    /// Variable rate tied to an external index; recalculated on each repayment.
    Variable,
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
    PauseMode,                   // PauseMode enum: None, Paused, or Thawing
    ThawState,                   // ThawState: pause and thaw timestamps
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
    VouchCooldownSecs, // u64 cooldown between vouch calls (default 24 hours)
    Timelock(u64),   // proposal_id → TimelockProposal
    TimelockCounter, // u64 monotonically increasing proposal ID
    Blacklisted(Address), // borrower → bool permanently banned
    VoucherWhitelist(Address), // voucher → bool allowed to vouch
    WhitelistEnabled, // bool: true when voucher whitelist is enabled (opt-in)
    ExtensionConsents(Address), // borrower → Vec<Address> vouchers who consented to extension
    SlashVote(Address), // borrower → SlashVoteRecord
    SlashVoteQuorum, // u32 quorum in basis points (e.g. 5000 = 50%)
    ReferredBy(Address), // borrower → Address of referrer
    ReferralBonusBps, // u32 referral bonus in basis points (default 100 = 1%)
    MaxVouchersPerBorrower, // u32 maximum number of vouchers per borrower (default 50)
    BorrowerCollateral(Address), // borrower → i128 collateral amount deposited
    BorrowerCollateralToken(Address), // borrower → Address token used for collateral
    InsurancePool,           // i128 total funds contributed to the insurance pool
    InsuranceClaim(u64),     // loan_id → bool: has any claim been made (legacy single-claim guard)
    InsuranceFeeBps,         // u32: protocol fee routed to insurance pool per loan (default 50 = 0.5%)
    InsuranceCoverageBps,    // u32: max payout as % of slashed stake (default 2500 = 25%)
    InsuranceVoucherClaim(u64, Address), // (loan_id, voucher) → i128 amount already claimed
    VouchHistory(Address, Address, Address), // (borrower, voucher, token) → Vec<VouchHistoryEntry>
    VouchDelegation(Address, Address, Address), // (borrower, original_voucher, token) → Address (delegate)
    PendingSlashExecution(Address), // borrower → PendingSlashRecord
    YieldReserve,            // i128 balance of the yield reserve
    SlashEscrow(Address),    // borrower → (i128 amount, u64 release_timestamp)
    SlashAudit(Address),     // borrower → SlashRecord (latest slash for borrower)
    SlashRecord(u64),        // slash_id → SlashRecord
    SlashRecordCounter,      // u64 monotonic slash ID counter
    BorrowerRegistered(Address), // borrower → registration timestamp
    // Issue #598-601 additions
    PrepaymentPenaltyBps,    // u32: prepayment penalty in basis points
    YieldDistribution(u64),  // loan_id → Vec<YieldDistributionEntry>
    AdminAction(u64),        // action_id → AdminActionProposal
    AdminActionCounter,      // u64: monotonically increasing admin action ID
    SlashAppeal(Address, Address), // (borrower, voucher) → SlashAppealRecord
    /// Slash-threshold governance proposal id → proposal record.
    SlashThresholdProposal(u64),
    SlashThresholdProposalCounter,
    /// Per-borrower timestamp of the last successful slash.
    LastSlashedAt(Address),
    /// Admin config-update proposal id → proposal record.
    ConfigUpdateProposal(u64),
    ConfigUpdateProposalCounter,
    /// Issue #599/#600: (voucher, borrower) → WithdrawalRequest (pending timelock withdrawal)
    PendingWithdrawal(Address, Address),
    /// Issue #601: borrower → LoanExtensionRequest
    LoanExtension(Address),
    /// Issue #598: loan_id → Vec<PaymentRecord> (payment history)
    PaymentHistory(u64),
    /// Voucher cumulative reputation stats: voucher → VoucherStats
    VoucherStats(Address),
    /// Withdrawal queue: borrower → Vec<QueuedWithdrawal>
    WithdrawalQueue(Address),
    // #634: Liquidity Mining
    LastMiningClaim(Address),
    // #635: Vouch Snapshot for Governance
    VouchSnapshot(u32),
    // #636: Staking Derivatives
    StakingDerivative(Address, Address),
    // #637: Fraud Detection
    VoucherFraudScore(Address),
    // #667: Oracle address for repayment verification
    OracleAddress,
    // #667: External credit score per borrower
    ExternalCreditScore(Address),
    // #666: Escrowed repayment amount per borrower (held pending oracle verification)
    EscrowAmount(Address),
    /// Monthly slashing transparency report: month_id → SlashingReportRecord.
    /// month_id = unix_timestamp / MONTHLY_PERIOD_SECS
    SlashingReport(u64),
    /// Per-vouch insurance opt-in: (voucher, borrower) → bool (insured).
    VoucherInsurance(Address, Address),
    /// Cross-chain bridge validation status: (voucher, chain_id) → bool.
    BridgeValidated(Address, u32),
    /// Issue #687: admin removal proposal id → AdminRemovalProposal
    AdminRemovalProposal(u64),
    /// Issue #687: monotonically increasing admin removal proposal counter
    AdminRemovalProposalCounter,
    /// Issue #686: accumulated admin compensation pool balance (i128 stroops)
    AdminCompensation,
    /// Issue #686: last compensation claim timestamp per admin address
    AdminLastClaim(Address),
}

// ── Governance ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct SlashVoteRecord {
    /// Total stake (in stroops) that has voted to approve this slash.
    /// 1 XLM = 10,000,000 stroops.
    pub approve_stake: i128,
    /// Total stake (in stroops) that has voted to reject this slash.
    /// 1 XLM = 10,000,000 stroops.
    pub reject_stake: i128,
    /// Addresses that have already cast a vote on this proposal.
    pub voters: Vec<Address>,
    /// `true` once the slash has been auto-executed after quorum was reached.
    pub executed: bool,
}

/// Governance proposal to change the protocol slash threshold (`Config.slash_bps`).
#[contracttype]
#[derive(Clone)]
pub struct SlashThresholdProposal {
    pub id: u64,
    pub proposer: Address,
    pub proposed_threshold: i128,
    pub proposed_at: u64,
    pub approve_votes: u32,
    pub reject_votes: u32,
    pub voters: Vec<Address>,
    pub finalized: bool,
}

/// Config field targeted by an admin config-update proposal.
#[contracttype]
#[derive(Clone)]
pub enum ConfigUpdateKey {
    AdminThreshold,
}

/// Multi-sig admin proposal to update a config field.
#[contracttype]
#[derive(Clone)]
pub struct ConfigUpdateProposal {
    pub id: u64,
    pub proposer: Address,
    pub key: ConfigUpdateKey,
    pub new_value: u32,
    pub approvals: Vec<Address>,
    pub executed: bool,
}

// ── Config ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admins: Vec<Address>,
    pub admin_threshold: u32,
    /// Admin addresses that are permitted to be configured as admins.
    /// If empty, any valid admin address may be used.
    pub admin_whitelist: Vec<Address>,
    /// Admin addresses that are explicitly forbidden from being configured as admins.
    pub admin_blacklist: Vec<Address>,
    /// Primary token contract address used for loans and vouches.
    pub token: Address,
    /// Additional token contract addresses accepted for loans/vouches.
    pub allowed_tokens: Vec<Address>,
    /// Yield rate in basis points (e.g. 200 = 2%). Applied to loan principal (in stroops)
    /// at repayment: `yield = principal_stroops * yield_bps / 10_000`.
    pub yield_bps: i128,
    /// Slash fraction in basis points (e.g. 5000 = 50%). Applied to voucher stake (in stroops)
    /// on borrower default: `slashed = stake_stroops * slash_bps / 10_000`.
    pub slash_bps: i128,
    pub max_vouchers: u32,
    /// Minimum loan amount, in stroops. 1 XLM = 10,000,000 stroops.
    pub min_loan_amount: i128,
    /// Maximum loan duration, in seconds.
    pub loan_duration: u64,
    /// Maximum ratio of loan amount to total staked collateral, expressed as a percentage
    /// (e.g. 150 means loan ≤ 1.5 × total stake in stroops).
    pub max_loan_to_stake_ratio: u32,
    /// Grace period after loan deadline before the loan can be slashed, in seconds.
    pub grace_period: u64,
    /// Minimum age of a vouch before it can be used for loan eligibility, in seconds (default 24 hours).
    pub min_vouch_age_secs: u64,
    /// Prepayment penalty in basis points (e.g. 100 = 1%). Applied to remaining principal
    /// when a borrower repays early. 0 means no penalty.
    pub prepayment_penalty_bps: u32,
    /// #634: Liquidity mining reward rate in basis points per epoch (e.g. 50 = 0.5% per 7 days).
    pub liquidity_mining_rate_bps: u32,
    /// Voting period for slash-threshold governance proposals, in seconds.
    pub voting_period_seconds: u64,
    /// Minimum seconds between slashes for the same borrower (0 = disabled).
    pub slash_cooldown_seconds: u64,
        /// When true, critical write paths are blocked until multi-sig emergency unpause.
    pub emergency_pause_enabled: bool,
    /// Issue #668: Discount applied to yield on early repayment, in basis points (0 = no discount).
    pub early_repayment_discount_bps: u32,
    /// Issue #666/#667: Optional oracle contract address for repayment verification.
    pub oracle_address: Option<soroban_sdk::Address>,
    /// Delay (in seconds) after a slash vote reaches quorum before it can be executed (0 = immediate).
    pub slash_delay_seconds: u64,
    /// Designated successor admin address that can claim admin rights without multi-sig approval
    /// when current admins are unavailable.
    pub successor_admin: Option<Address>,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct AmortizationEntry {
    pub due_date: u64,
    pub payment_due: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct LoanRecord {
    pub id: u64,
    pub borrower: Address,
    pub co_borrowers: Vec<Address>,
    /// Total loan principal disbursed, in stroops. 1 XLM = 10,000,000 stroops.
    pub amount: i128,
    /// Cumulative repayments received so far (principal + yield), in stroops.
    /// 1 XLM = 10,000,000 stroops.
    pub amount_repaid: i128,
    /// Yield owed to vouchers, locked in at disbursement time, in stroops.
    /// Computed as `amount * yield_bps / 10_000`. 1 XLM = 10,000,000 stroops.
    pub total_yield: i128,
    pub status: LoanStatus,
    /// Ledger timestamp when the loan record was created.
    pub created_at: u64,
    /// Ledger timestamp when the loan was disbursed to the borrower.
    pub disbursement_timestamp: u64,
    /// Ledger timestamp when the loan was fully repaid; `None` if not yet repaid.
    pub repayment_timestamp: Option<u64>,
    /// Repayment deadline as a ledger timestamp.
    pub deadline: u64,
    /// Borrower-supplied description of the loan purpose.
    pub loan_purpose: soroban_sdk::String,
    /// Address of the token contract used for this loan.
    pub token_address: Address,
    /// Amortization schedule for partial repayments.
    pub amortization_schedule: Vec<AmortizationEntry>,
    /// Whether a repayment reminder has been sent for this loan.
    pub reminder_sent: bool,
    /// Risk score for the borrower (0-100), used for dynamic yield calculation.
    pub risk_score: u32,
    /// Number of payment deferment periods used on this loan.
    pub deferment_periods: u32,
    /// Optional custom maturity date (ledger timestamp). When set, overrides the
    /// default `deadline` computed from `loan_duration`. `None` means use `deadline`.
    pub maturity_date: Option<u64>,
    /// Interest rate type for this loan.
    pub rate_type: RateType,
    /// For variable-rate loans: the oracle key or index name used to look up the
    /// current rate (e.g. `"SOFR"`, `"PRIME"`). `None` for fixed-rate loans.
    pub index_reference: Option<soroban_sdk::String>,
    /// Issue #666/#667: Escrow status for oracle-verified repayments.
    pub escrow_status: EscrowStatus,
    /// Issue #669: Retry count for failed repayments (max 3).
    pub retry_count: u32,
}

/// #645: Pending loan restructure request — borrower requests, vouchers approve.
#[contracttype]
#[derive(Clone)]
pub struct RestructureRequest {
    pub borrower: Address,
    /// New deadline (must be after current deadline).
    pub new_deadline: u64,
    /// Reduced outstanding amount (0 = no change to amount).
    pub new_amount: i128,
    /// Ledger timestamp when the request was created.
    pub requested_at: u64,
    /// Voucher addresses that have approved this request.
    pub approvals: Vec<Address>,
}

/// A single payment event recorded against a loan.
#[contracttype]
#[derive(Clone)]
pub struct PaymentRecord {
    /// Amount paid in this transaction, in stroops.
    pub amount: i128,
    /// Ledger timestamp of this payment.
    pub timestamp: u64,
    /// Cumulative amount repaid after this payment, in stroops.
    pub cumulative_repaid: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct VouchRecord {
    pub voucher: Address,
    /// Amount staked by the voucher, in stroops. 1 XLM = 10,000,000 stroops.
    pub stake: i128,
    /// Ledger timestamp when this vouch was created; immutable after set.
    pub vouch_timestamp: u64,
    /// Token contract address that this stake is denominated in.
    pub token: Address,
    /// Optional expiry timestamp; if set and current time > expiry, vouch is expired.
    pub expiry_timestamp: Option<u64>,
    /// Optional delegate address; if set, this address can manage the vouch.
    pub delegate: Option<Address>,
    /// Optional chain ID for cross-chain vouches. `None` means native Stellar.
    /// When set, the token must originate from a registered bridge for that chain.
    pub chain_id: Option<u32>,
}

/// Metadata for a registered cross-chain bridge.
#[contracttype]
#[derive(Clone)]
pub struct BridgeRecord {
    /// Numeric chain identifier (e.g. 1 = Ethereum mainnet, 137 = Polygon).
    pub chain_id: u32,
    /// Human-readable chain name (e.g. "ethereum", "polygon").
    pub chain_name: soroban_sdk::String,
    /// The Stellar-side bridge contract address that wraps/unwraps tokens.
    pub bridge_address: Address,
    /// Whether this bridge is currently active and accepted for new vouches.
    pub active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct VouchHistoryEntry {
    /// Timestamp of the modification.
    pub timestamp: u64,
    /// Type of modification: "created", "increased", "decreased", "withdrawn", "delegated".
    pub modification_type: soroban_sdk::String,
    /// Stake amount involved in the modification, in stroops.
    pub stake_amount: i128,
    /// Optional delegate address if this is a delegation event.
    pub delegate: Option<Address>,
}

#[contracttype]
#[derive(Clone)]
pub struct LoanPoolRecord {
    pub pool_id: u64,
    pub borrowers: Vec<Address>,
    /// Per-borrower loan amounts in this pool, in stroops. 1 XLM = 10,000,000 stroops.
    pub amounts: Vec<i128>,
    /// Ledger timestamp when this pool was created.
    pub created_at: u64,
    /// Total amount disbursed from this pool across all borrowers, in stroops.
    /// 1 XLM = 10,000,000 stroops.
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

/// A pending slash awaiting execution after the mandatory delay period.
/// Created when a slash vote reaches quorum; executed via `execute_pending_slash`.
#[contracttype]
#[derive(Clone)]
pub struct PendingSlashRecord {
    pub borrower: Address,
    pub approved_at: u64,
    pub executable_at: u64,
    pub executed: bool,
}

/// Controls where redistributable slash funds flow after insurance allocation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RedistributionRule {
    /// Route to the slash treasury (default).
    Treasury,
    /// Redistribute pro-rata to remaining active vouchers of the borrower.
    Vouchers,
}

#[contracttype]
#[derive(Clone)]
pub struct SlashRecord {
    pub slash_id: u64,
    pub borrower: Address,
    pub loan_id: u64,
    pub loan_amount: i128,
    pub total_slashed: i128,
    pub slash_timestamp: u64,
    /// Amount returned to borrower from treasury on full repay (0 until recovered).
    pub recovery_amount: i128,
    /// Set by admin on reversal; None when not reversed.
    pub reversal_reason: Option<soroban_sdk::String>,
    /// True once an admin has reversed this slash.
    pub reversed: bool,
}

/// Monthly aggregated report of all slashing events.
#[contracttype]
#[derive(Clone)]
pub struct SlashingReportRecord {
    /// Month identifier: unix_timestamp / MONTHLY_PERIOD_SECS.
    pub month_id: u64,
    /// Total number of slash events in this month.
    pub total_slashes: u32,
    /// Total amount slashed across all events, in stroops.
    pub total_slashed: i128,
    /// Number of slashes subsequently reversed by admins.
    pub total_reversed: u32,
    /// Slash IDs recorded during this month.
    pub slash_ids: Vec<u64>,
}

#[contracttype]
#[derive(Clone)]
pub struct WithdrawalRequest {
    pub voucher: Address,
    pub borrower: Address,
    pub token: Address,
    pub requested_at: u64,
}

/// A queued withdrawal request submitted during an active loan.
/// Processed automatically when the loan is repaid or slashed.
#[contracttype]
#[derive(Clone)]
pub struct QueuedWithdrawal {
    /// The voucher requesting withdrawal.
    pub voucher: Address,
    /// Token the stake is denominated in.
    pub token: Address,
    /// Ledger timestamp when the request was submitted.
    pub requested_at: u64,
    /// Whether this is a partial withdrawal (up to 50% of stake with penalty).
    pub partial: bool,
    /// Priority fee paid by the voucher (in stroops), distributed to remaining vouchers.
    pub priority_fee: i128,
}

/// Current API version of the contract.
pub const API_VERSION: u32 = 1;

#[contracttype]
#[derive(Clone)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

// ── API Caching (Issue #724) ──────────────────────────────────────────────────

/// Issue #687: Governance proposal to remove a compromised admin address.
/// Passes when `approve_votes >= Config.removal_vote_threshold`.
#[contracttype]
#[derive(Clone)]
pub struct AdminRemovalProposal {
    pub id: u64,
    /// Admin address to be removed if the proposal passes.
    pub admin_to_remove: Address,
    /// Address that created the proposal (must be a governance participant).
    pub proposer: Address,
    /// Number of approve votes cast so far.
    pub approve_votes: u32,
    /// Number of reject votes cast so far.
    pub reject_votes: u32,
    /// Addresses that have already voted (prevent double-voting).
    pub voters: Vec<Address>,
    /// Ledger timestamp when the proposal was created.
    pub proposed_at: u64,
    /// True once the proposal has been finalized (admin removed or rejected).
    pub finalized: bool,
}

// ── Pagination ────────────────────────────────────────────────────────────────

#[contracttype]
pub enum CacheKey {
    LoanCache(u64),           // loan_id → CachedLoanRecord
    VouchesCache(Address),    // borrower → CachedVouchesRecord
    ConfigCache,              // CachedConfigRecord
}

#[contracttype]
#[derive(Clone)]
pub struct CachedLoanRecord {
    pub data: LoanRecord,
    pub cached_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct CachedVouchesRecord {
    pub data: Vec<VouchRecord>,
    pub cached_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct CachedConfigRecord {
    pub data: Config,
    pub cached_at: u64,
}

// ── Error Standardization (Issue #725) ────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct ErrorResponse {
    /// Numeric error code matching ContractError enum.
    pub code: u32,
    /// Human-readable error message.
    pub message: soroban_sdk::String,
    /// Optional additional context or details.
    pub details: Option<soroban_sdk::String>,
    /// Timestamp when the error occurred.
    pub timestamp: u64,
}
