/// Tests for the withdrawal queue system (feat/withdrawal-queue).
///
/// Covers:
/// - request_withdrawal() queues during active loan
/// - request_withdrawal() executes immediately when no active loan
/// - partial_withdraw() deducts 50% with 10% penalty during active loan
/// - get_withdrawal_queue() returns queued entries
/// - duplicate queue entry rejected with WithdrawalAlreadyQueued
/// - decrease_stake() queues when active loan exists
/// - withdraw_vouch() queues when active loan exists
#[cfg(test)]
mod withdrawal_queue_tests {
    use crate::{ContractError, QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Address, Env, String, Vec,
    };

    struct Setup {
        env: Env,
        client: QuorumCreditContractClient<'static>,
        token_id: Address,
        borrower: Address,
        voucher: Address,
    }

    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);

        // Fund contract for loan disbursement
        StellarAssetClient::new(&env, &token_id.address()).mint(&contract_id, &50_000_000);

        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(
            &deployer,
            &Vec::from_array(&env, [admin]),
            &1,
            &token_id.address(),
        );

        // Advance past vouch cooldown (DEFAULT_VOUCH_COOLDOWN_SECS = 86400) and MIN_VOUCH_AGE
        env.ledger().with_mut(|l| l.timestamp = 90_000);

        let borrower = Address::generate(&env);
        let voucher = Address::generate(&env);

        // Mint and vouch
        StellarAssetClient::new(&env, &token_id.address()).mint(&voucher, &10_000_000);
        client.vouch(&voucher, &borrower, &10_000_000, &token_id.address());

        Setup {
            env,
            client,
            token_id: token_id.address(),
            borrower,
            voucher,
        }
    }

    fn disburse_loan(s: &Setup) {
        s.client.request_loan(
            &s.borrower,
            &5_000_000,
            &5_000_000,
            &String::from_str(&s.env, "test"),
            &s.token_id,
        );
    }

    // ── request_withdrawal ────────────────────────────────────────────────────

    #[test]
    fn test_request_withdrawal_queues_during_active_loan() {
        let s = setup();
        disburse_loan(&s);

        s.client.request_withdrawal(&s.voucher, &s.borrower, &0);

        let queue = s.client.get_withdrawal_queue(&s.borrower);
        assert_eq!(queue.len(), 1, "queue should have one entry");
        assert_eq!(queue.get(0).unwrap().voucher, s.voucher);
        assert_eq!(queue.get(0).unwrap().priority_fee, 0);
    }

    #[test]
    fn test_request_withdrawal_with_priority_fee() {
        let s = setup();
        disburse_loan(&s);

        // Mint extra tokens for priority fee
        StellarAssetClient::new(&s.env, &s.token_id).mint(&s.voucher, &100_000);
        s.client.request_withdrawal(&s.voucher, &s.borrower, &100_000);

        let queue = s.client.get_withdrawal_queue(&s.borrower);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.get(0).unwrap().priority_fee, 100_000);
    }

    #[test]
    fn test_request_withdrawal_no_active_loan_executes_immediately() {
        let s = setup();
        // No loan disbursed — should execute immediately (withdraw_vouch path)
        let balance_before =
            soroban_sdk::token::Client::new(&s.env, &s.token_id).balance(&s.voucher);

        s.client.request_withdrawal(&s.voucher, &s.borrower, &0);

        let balance_after =
            soroban_sdk::token::Client::new(&s.env, &s.token_id).balance(&s.voucher);
        assert_eq!(
            balance_after - balance_before,
            10_000_000,
            "stake should be returned immediately when no active loan"
        );

        let queue = s.client.get_withdrawal_queue(&s.borrower);
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_duplicate_withdrawal_request_rejected() {
        let s = setup();
        disburse_loan(&s);

        s.client.request_withdrawal(&s.voucher, &s.borrower, &0);

        let result = s
            .client
            .try_request_withdrawal(&s.voucher, &s.borrower, &0);
        assert_eq!(
            result,
            Err(Ok(ContractError::WithdrawalAlreadyQueued)),
            "duplicate queue entry must be rejected"
        );
    }

    // ── partial_withdraw ──────────────────────────────────────────────────────

    #[test]
    fn test_partial_withdraw_during_active_loan() {
        let s = setup();
        disburse_loan(&s);

        let balance_before =
            soroban_sdk::token::Client::new(&s.env, &s.token_id).balance(&s.voucher);

        s.client.partial_withdraw(&s.voucher, &s.borrower);

        let balance_after =
            soroban_sdk::token::Client::new(&s.env, &s.token_id).balance(&s.voucher);

        // 50% of 10_000_000 = 5_000_000 withdrawn
        // 10% penalty on 5_000_000 = 500_000
        // net payout = 4_500_000
        assert_eq!(
            balance_after - balance_before,
            4_500_000,
            "voucher should receive 50% stake minus 10% penalty"
        );
    }

    #[test]
    fn test_partial_withdraw_reduces_stake() {
        let s = setup();
        disburse_loan(&s);

        s.client.partial_withdraw(&s.voucher, &s.borrower);

        let vouches = s.client.get_vouches(&s.borrower);
        let remaining_stake = vouches
            .iter()
            .find(|v| v.voucher == s.voucher)
            .map(|v| v.stake)
            .unwrap_or(0);
        assert_eq!(
            remaining_stake, 5_000_000,
            "stake should be halved after partial withdrawal"
        );
    }

    // ── decrease_stake queuing ────────────────────────────────────────────────

    #[test]
    fn test_decrease_stake_queues_during_active_loan() {
        let s = setup();
        disburse_loan(&s);

        s.client.decrease_stake(&s.voucher, &s.borrower, &1_000_000);

        let queue = s.client.get_withdrawal_queue(&s.borrower);
        assert_eq!(queue.len(), 1, "decrease_stake should queue during active loan");
    }

    #[test]
    fn test_decrease_stake_executes_immediately_no_loan() {
        let s = setup();

        let balance_before =
            soroban_sdk::token::Client::new(&s.env, &s.token_id).balance(&s.voucher);

        s.client.decrease_stake(&s.voucher, &s.borrower, &3_000_000);

        let balance_after =
            soroban_sdk::token::Client::new(&s.env, &s.token_id).balance(&s.voucher);
        assert_eq!(
            balance_after - balance_before,
            3_000_000,
            "decrease_stake should return tokens immediately when no active loan"
        );
    }

    // ── withdraw_vouch queuing ────────────────────────────────────────────────

    #[test]
    fn test_withdraw_vouch_queues_during_active_loan() {
        let s = setup();
        disburse_loan(&s);

        s.client.withdraw_vouch(&s.voucher, &s.borrower);

        let queue = s.client.get_withdrawal_queue(&s.borrower);
        assert_eq!(queue.len(), 1, "withdraw_vouch should queue during active loan");
    }

    #[test]
    fn test_withdraw_vouch_executes_immediately_no_loan() {
        let s = setup();

        let balance_before =
            soroban_sdk::token::Client::new(&s.env, &s.token_id).balance(&s.voucher);

        s.client.withdraw_vouch(&s.voucher, &s.borrower);

        let balance_after =
            soroban_sdk::token::Client::new(&s.env, &s.token_id).balance(&s.voucher);
        assert_eq!(
            balance_after - balance_before,
            10_000_000,
            "full stake should be returned immediately when no active loan"
        );
    }

    // ── get_withdrawal_queue ──────────────────────────────────────────────────

    #[test]
    fn test_get_withdrawal_queue_empty_initially() {
        let s = setup();
        let queue = s.client.get_withdrawal_queue(&s.borrower);
        assert_eq!(queue.len(), 0, "queue should be empty initially");
    }

    #[test]
    fn test_multiple_vouchers_in_queue() {
        let s = setup();

        let voucher2 = Address::generate(&s.env);
        StellarAssetClient::new(&s.env, &s.token_id).mint(&voucher2, &5_000_000);
        s.env.ledger().with_mut(|l| l.timestamp += 120);
        s.client.vouch(&voucher2, &s.borrower, &5_000_000, &s.token_id);

        disburse_loan(&s);

        s.client.request_withdrawal(&s.voucher, &s.borrower, &0);
        s.client.request_withdrawal(&voucher2, &s.borrower, &0);

        let queue = s.client.get_withdrawal_queue(&s.borrower);
        assert_eq!(queue.len(), 2, "both vouchers should be in the queue");
    }
}
