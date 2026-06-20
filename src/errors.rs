use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    InsufficientFunds = 1,
    ActiveLoanExists = 2,
    StakeOverflow = 3,
    ZeroAddress = 4,
    DuplicateVouch = 5,
    NoActiveLoan = 6,
    ContractPaused = 7,
    LoanPastDeadline = 8,
    PoolLengthMismatch = 9,
    PoolEmpty = 10,
    PoolBorrowerActiveLoan = 11,
    PoolInsufficientFunds = 12,
    MinStakeNotMet = 13,
    LoanExceedsMaxAmount = 14,
    InsufficientVouchers = 15,
    UnauthorizedCaller = 16,
    InvalidAmount = 17,
    InvalidStateTransition = 18,
    AlreadyInitialized = 19,
    VouchTooRecent = 20,
    VouchCooldownActive = 21,
    VoucherNotWhitelisted = 23,
    Blacklisted = 24,
    TimelockNotFound = 25,
    TimelockNotReady = 26,
    TimelockExpired = 27,
    NoVouchesForBorrower = 28,
    VoucherNotFound = 29,
    InvalidToken = 30,
    AlreadyVoted = 31,
    SlashVoteNotFound = 32,
    SlashAlreadyExecuted = 33,
    LoanBelowMinAmount = 34,
    QuorumNotMet = 35,
    DelayNotElapsed = 36,
    MaxVouchersPerBorrowerExceeded = 37,
    InsufficientVoucherBalance = 38,
    SelfVouchNotAllowed = 39,
    DuplicateToken = 40,
    InvalidAdminThreshold = 41,
    InsufficientYieldReserve = 42,
    ReminderAlreadySent = 43,
    /// Insurance pool has no funds to cover the claim.
    InsurancePoolEmpty = 44,
    /// Insurance claim already made for this loan.
    InsuranceClaimAlreadyMade = 45,
    /// Basis points value is invalid (must be 0–10000).
    InvalidBps = 46,
    /// Withdrawal request already queued for this voucher/borrower pair.
    WithdrawalAlreadyQueued = 57,
    /// No queued withdrawal found for this voucher/borrower pair.
    WithdrawalNotQueued = 47,
    /// Partial withdrawal amount exceeds the 50% cap.
    PartialWithdrawalExceedsCap = 48,
    /// Borrower was slashed too recently; slash cooldown is still active.
    SlashCooldownActive = 49,
    /// Caller is not an admin or protocol-token holder allowed to govern.
    NotGovernanceParticipant = 50,
    /// Governance action is not allowed after the voting period has ended.
    VotingPeriodEnded = 51,
    /// Governance proposal was not found.
    ProposalNotFound = 52,
    /// Governance proposal was already finalized.
    ProposalAlreadyFinalized = 53,
    /// Oracle caller is not the registered oracle contract (#666/#667).
    OracleUnauthorized = 54,
    /// Repayment retry limit has been exceeded (#669).
    MaxRetriesExceeded = 55,
    /// No escrow record found for this borrower (#666/#667).
    NoEscrowFound = 56,
    /// No slash record found for the given slash ID.
    SlashRecordNotFound = 57,
    /// Slash has already been reversed and cannot be reversed again.
    SlashAlreadyReversed = 58,
    /// Caller has exceeded the configured rate limit.
    RateLimitExceeded = 59,
    /// Caller does not have the required role or permission.
    PermissionDenied = 60,
    /// Cryptographic proof validation failed.
    InvalidProof = 61,
    /// Arithmetic overflow or underflow occurred.
    ArithmeticError = 62,
    /// No rollback snapshot found for the requested deployment index (#744).
    RollbackSnapshotNotFound = 63,
    /// No Ed25519 verification key is configured for the origin chain.
    BridgeNotConfigured = 100,
    /// The origin/destination chain combination is invalid.
    InvalidBridgeChain = 101,
    /// This origin-chain nonce has already been consumed.
    ReplayAttackDetected = 102,
    /// The attestation is outside the accepted freshness window.
    AttestationExpired = 103,
    /// The attestation timestamp is too far ahead of the ledger clock.
    AttestationFromFuture = 104,
    /// This canonical loan has already moved its reputation to another chain.
    ReputationAlreadySpent = 105,
    /// A newer reputation attestation has already been applied.
    StaleBridgeAttestation = 106,
}
