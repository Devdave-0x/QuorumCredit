from typing import List, Optional, Dict, Any
from dataclasses import dataclass
from stellar_sdk import (
    Keypair,
    Network,
    TransactionBuilder,
    BASE_FEE,
    SorobanServer,
    Contract,
    Address,
    Uint64,
    Int64,
    String,
    Vec,
)


@dataclass
class ClientConfig:
    contract_id: str
    rpc_url: str
    network_passphrase: str
    keypair: Keypair


@dataclass
class LoanRecord:
    id: str
    borrower: str
    amount: str
    amount_repaid: str
    total_yield: str
    status: str
    created_at: int
    deadline: int
    loan_purpose: str


@dataclass
class VouchRecord:
    voucher: str
    stake: str
    vouch_timestamp: int
    token: str


@dataclass
class Config:
    admins: List[str]
    admin_threshold: int
    token: str
    allowed_tokens: List[str]
    yield_bps: int
    slash_bps: int
    min_loan_amount: str
    max_loan_amount: str
    loan_duration: int


class QuorumCreditClient:
    def __init__(self, config: ClientConfig):
        self.config = config
        self.soroban_server = SorobanServer(config.rpc_url)
        self.contract = Contract(config.contract_id)

    async def initialize(
        self,
        deployer: str,
        admins: List[str],
        admin_threshold: int,
        token: str,
    ) -> str:
        """Initialize the contract with admin addresses and token."""
        account = await self.soroban_server.get_account(self.config.keypair.public_key)
        tx = (
            TransactionBuilder(
                account,
                base_fee=BASE_FEE,
                network_passphrase=self.config.network_passphrase,
            )
            .add_text_memo("QuorumCredit Initialize")
            .append_invoke_soroban_host_function_op(
                auth=[],
                invoke_contract=self.contract.invoke(
                    "initialize",
                    Address(deployer),
                    Vec([Address(admin) for admin in admins]),
                    Uint64(admin_threshold),
                    Address(token),
                ),
            )
            .set_timeout(30)
            .build()
        )

        return await self._submit_transaction(tx)

    async def vouch(
        self, voucher: str, borrower: str, stake: str, token: str
    ) -> str:
        """Stake tokens to vouch for a borrower."""
        account = await self.soroban_server.get_account(self.config.keypair.public_key)
        tx = (
            TransactionBuilder(
                account,
                base_fee=BASE_FEE,
                network_passphrase=self.config.network_passphrase,
            )
            .add_text_memo("QuorumCredit Vouch")
            .append_invoke_soroban_host_function_op(
                auth=[],
                invoke_contract=self.contract.invoke(
                    "vouch",
                    Address(voucher),
                    Address(borrower),
                    Int64(stake),
                    Address(token),
                ),
            )
            .set_timeout(30)
            .build()
        )

        return await self._submit_transaction(tx)

    async def batch_vouch(
        self,
        voucher: str,
        borrowers: List[str],
        stakes: List[str],
        token: str,
    ) -> str:
        """Stake for multiple borrowers atomically."""
        if len(borrowers) != len(stakes):
            raise ValueError("borrowers and stakes must have the same length")

        account = await self.soroban_server.get_account(self.config.keypair.public_key)
        tx = (
            TransactionBuilder(
                account,
                base_fee=BASE_FEE,
                network_passphrase=self.config.network_passphrase,
            )
            .add_text_memo("QuorumCredit Batch Vouch")
            .append_invoke_soroban_host_function_op(
                auth=[],
                invoke_contract=self.contract.invoke(
                    "batch_vouch",
                    Address(voucher),
                    Vec([Address(b) for b in borrowers]),
                    Vec([Int64(s) for s in stakes]),
                    Address(token),
                ),
            )
            .set_timeout(30)
            .build()
        )

        return await self._submit_transaction(tx)

    async def request_loan(
        self,
        borrower: str,
        amount: str,
        threshold: str,
        loan_purpose: str,
        token: str,
    ) -> str:
        """Request a loan if sufficient vouches exist."""
        account = await self.soroban_server.get_account(self.config.keypair.public_key)
        tx = (
            TransactionBuilder(
                account,
                base_fee=BASE_FEE,
                network_passphrase=self.config.network_passphrase,
            )
            .add_text_memo("QuorumCredit Request Loan")
            .append_invoke_soroban_host_function_op(
                auth=[],
                invoke_contract=self.contract.invoke(
                    "request_loan",
                    Address(borrower),
                    Int64(amount),
                    Int64(threshold),
                    String(loan_purpose),
                    Address(token),
                ),
            )
            .set_timeout(30)
            .build()
        )

        return await self._submit_transaction(tx)

    async def repay(self, borrower: str, payment: str) -> str:
        """Repay loan principal and distribute yield to vouchers."""
        account = await self.soroban_server.get_account(self.config.keypair.public_key)
        tx = (
            TransactionBuilder(
                account,
                base_fee=BASE_FEE,
                network_passphrase=self.config.network_passphrase,
            )
            .add_text_memo("QuorumCredit Repay")
            .append_invoke_soroban_host_function_op(
                auth=[],
                invoke_contract=self.contract.invoke(
                    "repay",
                    Address(borrower),
                    Int64(payment),
                ),
            )
            .set_timeout(30)
            .build()
        )

        return await self._submit_transaction(tx)

    async def slash(self, admin_signers: List[str], borrower: str) -> str:
        """Slash a defaulted borrower."""
        account = await self.soroban_server.get_account(self.config.keypair.public_key)
        tx = (
            TransactionBuilder(
                account,
                base_fee=BASE_FEE,
                network_passphrase=self.config.network_passphrase,
            )
            .add_text_memo("QuorumCredit Slash")
            .append_invoke_soroban_host_function_op(
                auth=[],
                invoke_contract=self.contract.invoke(
                    "slash",
                    Vec([Address(signer) for signer in admin_signers]),
                    Address(borrower),
                ),
            )
            .set_timeout(30)
            .build()
        )

        return await self._submit_transaction(tx)

    async def get_loan(self, borrower: str) -> Optional[LoanRecord]:
        """Get loan record for a borrower."""
        try:
            result = await self.soroban_server.simulate_transaction(
                self._build_read_transaction("get_loan", Address(borrower))
            )

            if result.error:
                return None

            result_value = result.results[0].result.retval if result.results else None
            return self._parse_loan_record(result_value) if result_value else None
        except Exception:
            return None

    async def get_vouches(self, borrower: str) -> List[VouchRecord]:
        """Get all vouches for a borrower."""
        try:
            result = await self.soroban_server.simulate_transaction(
                self._build_read_transaction("get_vouches", Address(borrower))
            )

            if result.error:
                return []

            result_value = result.results[0].result.retval if result.results else None
            return self._parse_vouch_records(result_value) if result_value else []
        except Exception:
            return []

    async def is_eligible(
        self, borrower: str, threshold: str, token: str
    ) -> bool:
        """Check if borrower is eligible for a loan."""
        try:
            result = await self.soroban_server.simulate_transaction(
                self._build_read_transaction(
                    "is_eligible",
                    Address(borrower),
                    Int64(threshold),
                    Address(token),
                )
            )

            if result.error:
                return False

            result_value = result.results[0].result.retval if result.results else None
            return bool(result_value) if result_value else False
        except Exception:
            return False

    async def get_config(self) -> Config:
        """Get protocol configuration."""
        result = await self.soroban_server.simulate_transaction(
            self._build_read_transaction("get_config")
        )

        if result.error:
            raise Exception("Failed to fetch config")

        result_value = result.results[0].result.retval if result.results else None
        return self._parse_config(result_value) if result_value else Config(
            admins=[],
            admin_threshold=0,
            token="",
            allowed_tokens=[],
            yield_bps=0,
            slash_bps=0,
            min_loan_amount="0",
            max_loan_amount="0",
            loan_duration=0,
        )

    async def _submit_transaction(self, tx: Any) -> str:
        """Submit a transaction to the network."""
        signed_tx = tx.sign(self.config.keypair)
        result = await self.soroban_server.send_transaction(signed_tx)

        if result.error_result_xdr:
            raise Exception(f"Transaction failed: {result.error_result_xdr}")

        return result.hash

    def _build_read_transaction(self, *args: Any) -> Any:
        """Build a read-only transaction."""
        account = self.soroban_server.get_account(self.config.keypair.public_key)
        tx = (
            TransactionBuilder(
                account,
                base_fee=BASE_FEE,
                network_passphrase=self.config.network_passphrase,
            )
            .append_invoke_soroban_host_function_op(
                auth=[],
                invoke_contract=self.contract.invoke(*args),
            )
            .set_timeout(30)
            .build()
        )

        return tx

    @staticmethod
    def _parse_loan_record(sc_val: Any) -> LoanRecord:
        """Parse a loan record from Soroban value."""
        return LoanRecord(
            id=sc_val.id,
            borrower=sc_val.borrower,
            amount=sc_val.amount,
            amount_repaid=sc_val.amount_repaid,
            total_yield=sc_val.total_yield,
            status=sc_val.status,
            created_at=sc_val.created_at,
            deadline=sc_val.deadline,
            loan_purpose=sc_val.loan_purpose,
        )

    @staticmethod
    def _parse_vouch_records(sc_val: Any) -> List[VouchRecord]:
        """Parse vouch records from Soroban value."""
        if not isinstance(sc_val, list):
            return []

        return [
            VouchRecord(
                voucher=v.voucher,
                stake=v.stake,
                vouch_timestamp=v.vouch_timestamp,
                token=v.token,
            )
            for v in sc_val
        ]

    @staticmethod
    def _parse_config(sc_val: Any) -> Config:
        """Parse config from Soroban value."""
        return Config(
            admins=sc_val.admins,
            admin_threshold=sc_val.admin_threshold,
            token=sc_val.token,
            allowed_tokens=sc_val.allowed_tokens,
            yield_bps=sc_val.yield_bps,
            slash_bps=sc_val.slash_bps,
            min_loan_amount=sc_val.min_loan_amount,
            max_loan_amount=sc_val.max_loan_amount,
            loan_duration=sc_val.loan_duration,
        )
