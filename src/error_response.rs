//! Error standardization for consistent API responses (Issue #725)
//!
//! This module provides utilities for creating standardized error responses
//! across the contract API.

use crate::types::ErrorResponse;
use crate::ContractError;
use soroban_sdk::{Env, String};

/// Map a ContractError to a standardized error response.
pub fn error_to_response(env: &Env, error: ContractError) -> ErrorResponse {
    let (code, message, details) = match error {
        ContractError::InsufficientFunds => (
            1,
            "Insufficient funds",
            Some("The contract or account does not have enough balance for this operation"),
        ),
        ContractError::ActiveLoanExists => (
            2,
            "Active loan exists",
            Some("Borrower already has an active loan that must be repaid or defaulted first"),
        ),
        ContractError::StakeOverflow => (
            3,
            "Stake overflow",
            Some("Total vouched stake would exceed i128 maximum"),
        ),
        ContractError::ZeroAddress => (
            4,
            "Zero address",
            Some("Admin or token address cannot be the zero address"),
        ),
        ContractError::DuplicateVouch => (
            5,
            "Duplicate vouch",
            Some("Voucher already has an active vouch for this borrower"),
        ),
        ContractError::NoActiveLoan => (
            6,
            "No active loan",
            Some("Borrower does not have an active loan"),
        ),
        ContractError::ContractPaused => (
            7,
            "Contract paused",
            Some("Contract is currently paused and state-changing operations are disabled"),
        ),
        ContractError::LoanPastDeadline => (
            8,
            "Loan past deadline",
            Some("Loan repayment deadline has passed"),
        ),
        ContractError::MinStakeNotMet => (
            13,
            "Minimum stake not met",
            Some("Vouch stake is below the configured minimum"),
        ),
        ContractError::LoanExceedsMaxAmount => (
            14,
            "Loan exceeds maximum amount",
            Some("Requested loan amount exceeds the configured maximum"),
        ),
        ContractError::InsufficientVouchers => (
            15,
            "Insufficient vouchers",
            Some("Number of vouchers is below the configured minimum"),
        ),
        ContractError::UnauthorizedCaller => (
            16,
            "Unauthorized caller",
            Some("Caller is not authorized to perform this operation"),
        ),
        ContractError::InvalidAmount => (
            17,
            "Invalid amount",
            Some("Amount parameter is invalid or out of range"),
        ),
        ContractError::InvalidStateTransition => (
            18,
            "Invalid state transition",
            Some("Operation is not valid for the current loan state"),
        ),
        ContractError::AlreadyInitialized => (
            19,
            "Already initialized",
            Some("Contract has already been initialized"),
        ),
        ContractError::VouchTooRecent => (
            20,
            "Vouch too recent",
            Some("Vouch was added too recently and must age before loan eligibility"),
        ),
        ContractError::Blacklisted => (
            24,
            "Blacklisted",
            Some("Borrower is blacklisted and cannot request loans"),
        ),
        ContractError::InvalidToken => (
            30,
            "Invalid token",
            Some("Token address is not allowed or does not implement SEP-41"),
        ),
        ContractError::AlreadyVoted => (
            31,
            "Already voted",
            Some("Voucher has already cast a vote on this proposal"),
        ),
        ContractError::SlashVoteNotFound => (
            32,
            "Slash vote not found",
            Some("No active slash vote exists for this borrower"),
        ),
        ContractError::SlashAlreadyExecuted => (
            33,
            "Slash already executed",
            Some("Slash vote has already been executed"),
        ),
        ContractError::LoanBelowMinAmount => (
            34,
            "Loan below minimum amount",
            Some("Requested loan amount is below the configured minimum"),
        ),
        ContractError::QuorumNotMet => (
            35,
            "Quorum not met",
            Some("Insufficient votes to execute the proposal"),
        ),
        _ => (
            255,
            "Unknown error",
            Some("An unexpected error occurred"),
        ),
    };

    ErrorResponse {
        code,
        message: String::from_slice(env, message),
        details: details.map(|d| String::from_slice(env, d)),
        timestamp: env.ledger().timestamp(),
    }
}

/// Create a standardized error response with custom details.
pub fn create_error_response(
    env: &Env,
    code: u32,
    message: &str,
    details: Option<&str>,
) -> ErrorResponse {
    ErrorResponse {
        code,
        message: String::from_slice(env, message),
        details: details.map(|d| String::from_slice(env, d)),
        timestamp: env.ledger().timestamp(),
    }
}
