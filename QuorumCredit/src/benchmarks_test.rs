/// Performance benchmarks for critical QuorumCredit contract operations.
///
/// Each test measures the CPU instruction cost of a single operation using
/// Soroban's built-in budget tracker, then asserts it stays within the
/// defined ceiling. These ceilings act as regression guards: a PR that
/// significantly increases the cost of vouch / request_loan / repay will
/// fail CI before it merges.
///
/// Note: costs are measured against the native Rust host, which
/// under-estimates WASM costs by roughly 2-4×. The ceilings below are
/// intentionally generous to remain stable across minor SDK updates.
#[cfg(test)]
mod benchmarks {
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Address, Env, String, Vec,
    };

    // ── CPU instruction ceilings (native host units) ──────────────────────────
    const VOUCH_CPU_CEILING: u64 = 50_000_000;
    const REQUEST_LOAN_CPU_CEILING: u64 = 80_000_000;
    const REPAY_CPU_CEILING: u64 = 80_000_000;

    struct Setup {
        env: Env,
        client: QuorumCreditContractClient<'static>,
        token: Address,
    }

    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();
        env.budget().reset_unlimited();

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let admins = Vec::from_array(&env, [admin.clone()]);
        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);

        // Pre-fund contract so loans can be disbursed
        StellarAssetClient::new(&env, &token_id.address()).mint(&contract_id, &100_000_000);

        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token_id.address());

        // Advance past MIN_VOUCH_AGE (60 s)
        env.ledger().with_mut(|l| l.timestamp = 120);

        Setup { env, client, token: token_id.address() }
    }

    fn mint(s: &Setup, to: &Address, amount: i128) {
        StellarAssetClient::new(&s.env, &s.token).mint(to, &amount);
    }

    fn purpose(env: &Env) -> String {
        String::from_str(env, "benchmark")
    }

    // ── vouch ─────────────────────────────────────────────────────────────────

    #[test]
    fn bench_vouch_cpu_cost() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        mint(&s, &voucher, 1_000_000);

        s.env.budget().reset_tracker();
        s.client.vouch(&voucher, &borrower, &1_000_000, &s.token);
        let cpu = s.env.budget().cpu_instruction_cost();

        println!("[bench] vouch cpu_instructions = {cpu}");
        assert!(
            cpu <= VOUCH_CPU_CEILING,
            "vouch exceeded CPU ceiling: {cpu} > {VOUCH_CPU_CEILING}"
        );
    }

    // ── request_loan ──────────────────────────────────────────────────────────

    #[test]
    fn bench_request_loan_cpu_cost() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        mint(&s, &voucher, 1_000_000);

        s.client.vouch(&voucher, &borrower, &1_000_000, &s.token);

        s.env.budget().reset_tracker();
        s.client.request_loan(&borrower, &500_000, &500_000, &purpose(&s.env), &s.token);
        let cpu = s.env.budget().cpu_instruction_cost();

        println!("[bench] request_loan cpu_instructions = {cpu}");
        assert!(
            cpu <= REQUEST_LOAN_CPU_CEILING,
            "request_loan exceeded CPU ceiling: {cpu} > {REQUEST_LOAN_CPU_CEILING}"
        );
    }

    // ── repay ─────────────────────────────────────────────────────────────────

    #[test]
    fn bench_repay_cpu_cost() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        mint(&s, &voucher, 1_000_000);
        mint(&s, &borrower, 600_000);

        s.client.vouch(&voucher, &borrower, &1_000_000, &s.token);
        s.client.request_loan(&borrower, &500_000, &500_000, &purpose(&s.env), &s.token);

        let loan = s.client.get_loan(&borrower).unwrap();
        let payment = loan.amount + loan.total_yield;

        s.env.budget().reset_tracker();
        s.client.repay(&borrower, &payment);
        let cpu = s.env.budget().cpu_instruction_cost();

        println!("[bench] repay cpu_instructions = {cpu}");
        assert!(
            cpu <= REPAY_CPU_CEILING,
            "repay exceeded CPU ceiling: {cpu} > {REPAY_CPU_CEILING}"
        );
    }

    // ── memory ────────────────────────────────────────────────────────────────

    #[test]
    fn bench_vouch_memory_cost() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        mint(&s, &voucher, 1_000_000);

        s.env.budget().reset_tracker();
        s.client.vouch(&voucher, &borrower, &1_000_000, &s.token);
        let mem = s.env.budget().memory_bytes_cost();

        println!("[bench] vouch memory_bytes = {mem}");
        // Sanity check: memory usage should be non-zero and reasonable
        assert!(mem > 0, "vouch memory cost should be non-zero");
    }

    #[test]
    fn bench_request_loan_memory_cost() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        mint(&s, &voucher, 1_000_000);

        s.client.vouch(&voucher, &borrower, &1_000_000, &s.token);

        s.env.budget().reset_tracker();
        s.client.request_loan(&borrower, &500_000, &500_000, &purpose(&s.env), &s.token);
        let mem = s.env.budget().memory_bytes_cost();

        println!("[bench] request_loan memory_bytes = {mem}");
        assert!(mem > 0, "request_loan memory cost should be non-zero");
    }

    #[test]
    fn bench_repay_memory_cost() {
        let s = setup();
        let voucher = Address::generate(&s.env);
        let borrower = Address::generate(&s.env);
        mint(&s, &voucher, 1_000_000);
        mint(&s, &borrower, 600_000);

        s.client.vouch(&voucher, &borrower, &1_000_000, &s.token);
        s.client.request_loan(&borrower, &500_000, &500_000, &purpose(&s.env), &s.token);

        let loan = s.client.get_loan(&borrower).unwrap();
        let payment = loan.amount + loan.total_yield;

        s.env.budget().reset_tracker();
        s.client.repay(&borrower, &payment);
        let mem = s.env.budget().memory_bytes_cost();

        println!("[bench] repay memory_bytes = {mem}");
        assert!(mem > 0, "repay memory cost should be non-zero");
    }
}
