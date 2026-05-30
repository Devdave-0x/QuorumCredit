import {
  Keypair,
  Networks,
  TransactionBuilder,
  BASE_FEE,
  SorobanRpc,
  Contract,
  nativeToScVal,
  scValToNative,
} from '@stellar/js-sdk';

export interface ClientConfig {
  contractId: string;
  rpcUrl: string;
  networkPassphrase: string;
  keypair: Keypair;
}

export interface VouchParams {
  voucher: string;
  borrower: string;
  stake: string;
  token: string;
}

export interface BatchVouchParams {
  voucher: string;
  borrowers: string[];
  stakes: string[];
  token: string;
}

export interface RequestLoanParams {
  borrower: string;
  amount: string;
  threshold: string;
  loanPurpose: string;
  token: string;
}

export interface RepayParams {
  borrower: string;
  payment: string;
}

export interface SlashParams {
  adminSigners: string[];
  borrower: string;
}

export interface LoanRecord {
  id: string;
  borrower: string;
  amount: string;
  amountRepaid: string;
  totalYield: string;
  status: 'Active' | 'Repaid' | 'Defaulted';
  createdAt: number;
  deadline: number;
  loanPurpose: string;
}

export interface VouchRecord {
  voucher: string;
  stake: string;
  vouchTimestamp: number;
  token: string;
}

export interface Config {
  admins: string[];
  adminThreshold: number;
  token: string;
  allowedTokens: string[];
  yieldBps: number;
  slashBps: number;
  minLoanAmount: string;
  maxLoanAmount: string;
  loanDuration: number;
}

export class QuorumCreditClient {
  private config: ClientConfig;
  private sorobanRpc: SorobanRpc.Server;
  private contract: Contract;

  constructor(config: ClientConfig) {
    this.config = config;
    this.sorobanRpc = new SorobanRpc.Server(config.rpcUrl);
    this.contract = new Contract(config.contractId);
  }

  async initialize(
    deployer: string,
    admins: string[],
    adminThreshold: number,
    token: string
  ): Promise<string> {
    const account = await this.sorobanRpc.getAccount(this.config.keypair.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          'initialize',
          nativeToScVal(deployer, { type: 'address' }),
          nativeToScVal(admins, { type: 'vec' }),
          nativeToScVal(adminThreshold, { type: 'u32' }),
          nativeToScVal(token, { type: 'address' })
        )
      )
      .setTimeout(30)
      .build();

    return this.submitTransaction(tx);
  }

  async vouch(params: VouchParams): Promise<string> {
    const account = await this.sorobanRpc.getAccount(this.config.keypair.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          'vouch',
          nativeToScVal(params.voucher, { type: 'address' }),
          nativeToScVal(params.borrower, { type: 'address' }),
          nativeToScVal(params.stake, { type: 'i128' }),
          nativeToScVal(params.token, { type: 'address' })
        )
      )
      .setTimeout(30)
      .build();

    return this.submitTransaction(tx);
  }

  async batchVouch(params: BatchVouchParams): Promise<string> {
    const account = await this.sorobanRpc.getAccount(this.config.keypair.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          'batch_vouch',
          nativeToScVal(params.voucher, { type: 'address' }),
          nativeToScVal(params.borrowers, { type: 'vec' }),
          nativeToScVal(params.stakes, { type: 'vec' }),
          nativeToScVal(params.token, { type: 'address' })
        )
      )
      .setTimeout(30)
      .build();

    return this.submitTransaction(tx);
  }

  async requestLoan(params: RequestLoanParams): Promise<string> {
    const account = await this.sorobanRpc.getAccount(this.config.keypair.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          'request_loan',
          nativeToScVal(params.borrower, { type: 'address' }),
          nativeToScVal(params.amount, { type: 'i128' }),
          nativeToScVal(params.threshold, { type: 'i128' }),
          nativeToScVal(params.loanPurpose, { type: 'string' }),
          nativeToScVal(params.token, { type: 'address' })
        )
      )
      .setTimeout(30)
      .build();

    return this.submitTransaction(tx);
  }

  async repay(params: RepayParams): Promise<string> {
    const account = await this.sorobanRpc.getAccount(this.config.keypair.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          'repay',
          nativeToScVal(params.borrower, { type: 'address' }),
          nativeToScVal(params.payment, { type: 'i128' })
        )
      )
      .setTimeout(30)
      .build();

    return this.submitTransaction(tx);
  }

  async slash(params: SlashParams): Promise<string> {
    const account = await this.sorobanRpc.getAccount(this.config.keypair.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(
        this.contract.call(
          'slash',
          nativeToScVal(params.adminSigners, { type: 'vec' }),
          nativeToScVal(params.borrower, { type: 'address' })
        )
      )
      .setTimeout(30)
      .build();

    return this.submitTransaction(tx);
  }

  async getLoan(borrower: string): Promise<LoanRecord | null> {
    const result = await this.sorobanRpc.simulateTransaction(
      this.buildReadTransaction('get_loan', nativeToScVal(borrower, { type: 'address' }))
    );

    if (result.error) {
      return null;
    }

    const resultValue = result.results?.[0]?.result.retval;
    return resultValue ? this.parseLoanRecord(resultValue) : null;
  }

  async getVouches(borrower: string): Promise<VouchRecord[]> {
    const result = await this.sorobanRpc.simulateTransaction(
      this.buildReadTransaction('get_vouches', nativeToScVal(borrower, { type: 'address' }))
    );

    if (result.error) {
      return [];
    }

    const resultValue = result.results?.[0]?.result.retval;
    return resultValue ? this.parseVouchRecords(resultValue) : [];
  }

  async isEligible(borrower: string, threshold: string, token: string): Promise<boolean> {
    const result = await this.sorobanRpc.simulateTransaction(
      this.buildReadTransaction(
        'is_eligible',
        nativeToScVal(borrower, { type: 'address' }),
        nativeToScVal(threshold, { type: 'i128' }),
        nativeToScVal(token, { type: 'address' })
      )
    );

    if (result.error) {
      return false;
    }

    const resultValue = result.results?.[0]?.result.retval;
    return resultValue ? scValToNative(resultValue) : false;
  }

  async getConfig(): Promise<Config> {
    const result = await this.sorobanRpc.simulateTransaction(
      this.buildReadTransaction('get_config')
    );

    if (result.error) {
      throw new Error('Failed to fetch config');
    }

    const resultValue = result.results?.[0]?.result.retval;
    return resultValue ? this.parseConfig(resultValue) : ({} as Config);
  }

  private async submitTransaction(tx: any): Promise<string> {
    const signedTx = tx.sign(this.config.keypair);
    const result = await this.sorobanRpc.sendTransaction(signedTx);

    if (result.errorResultXdr) {
      throw new Error(`Transaction failed: ${result.errorResultXdr}`);
    }

    return result.hash;
  }

  private buildReadTransaction(...args: any[]): any {
    const account = new SorobanRpc.Account(this.config.keypair.publicKey(), '0');
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.config.networkPassphrase,
    })
      .addOperation(this.contract.call(...args))
      .setTimeout(30)
      .build();

    return tx;
  }

  private parseLoanRecord(scVal: any): LoanRecord {
    const native = scValToNative(scVal);
    return {
      id: native.id,
      borrower: native.borrower,
      amount: native.amount,
      amountRepaid: native.amount_repaid,
      totalYield: native.total_yield,
      status: native.status,
      createdAt: native.created_at,
      deadline: native.deadline,
      loanPurpose: native.loan_purpose,
    };
  }

  private parseVouchRecords(scVal: any): VouchRecord[] {
    const native = scValToNative(scVal);
    return Array.isArray(native)
      ? native.map((v: any) => ({
          voucher: v.voucher,
          stake: v.stake,
          vouchTimestamp: v.vouch_timestamp,
          token: v.token,
        }))
      : [];
  }

  private parseConfig(scVal: any): Config {
    const native = scValToNative(scVal);
    return {
      admins: native.admins,
      adminThreshold: native.admin_threshold,
      token: native.token,
      allowedTokens: native.allowed_tokens,
      yieldBps: native.yield_bps,
      slashBps: native.slash_bps,
      minLoanAmount: native.min_loan_amount,
      maxLoanAmount: native.max_loan_amount,
      loanDuration: native.loan_duration,
    };
  }
}

export default QuorumCreditClient;
