use soroban_sdk::{Env, String};

/// Generate OpenAPI 3.0 schema for the contract
pub fn generate_openapi_schema(env: Env) -> String {
    let mut schema = String::new(&env);
    
    schema.append(&String::from_slice(&env, r#"{"openapi":"3.0.0","info":{"title":"QuorumCredit API","version":"1.0.0","description":"Decentralized microlending platform powered by social trust"},"servers":[{"url":"https://soroban-testnet.stellar.org","description":"Testnet"},{"url":"https://rpc.mainnet.stellar.org","description":"Mainnet"}],"paths":{"#));
    
    // Initialize endpoint
    schema.append(&String::from_slice(&env, r#""/initialize":{"post":{"summary":"Initialize contract","operationId":"initialize","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","properties":{"deployer":{"type":"string"},"admins":{"type":"array","items":{"type":"string"}},"admin_threshold":{"type":"integer"},"token":{"type":"string"}}}}}},"responses":{"200":{"description":"Contract initialized successfully"}}}},"#));
    
    // Vouch endpoint
    schema.append(&String::from_slice(&env, r#""/vouch":{"post":{"summary":"Stake XLM to vouch for a borrower","operationId":"vouch","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","properties":{"voucher":{"type":"string"},"borrower":{"type":"string"},"stake":{"type":"integer"},"token":{"type":"string"}}}}}},"responses":{"200":{"description":"Vouch created successfully"}}}},"#));
    
    // Request loan endpoint
    schema.append(&String::from_slice(&env, r#""/request_loan":{"post":{"summary":"Request a loan","operationId":"request_loan","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","properties":{"borrower":{"type":"string"},"amount":{"type":"integer"},"threshold":{"type":"integer"},"loan_purpose":{"type":"string"},"token":{"type":"string"}}}}}},"responses":{"200":{"description":"Loan disbursed successfully"}}}},"#));
    
    // Repay endpoint
    schema.append(&String::from_slice(&env, r#""/repay":{"post":{"summary":"Repay a loan","operationId":"repay","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","properties":{"borrower":{"type":"string"},"payment":{"type":"integer"}}}}}},"responses":{"200":{"description":"Loan repaid successfully"}}}},"#));
    
    // Get loan endpoint
    schema.append(&String::from_slice(&env, r#""/get_loan/{borrower}":{"get":{"summary":"Get loan details","operationId":"get_loan","parameters":[{"name":"borrower","in":"path","required":true,"schema":{"type":"string"}}],"responses":{"200":{"description":"Loan details","content":{"application/json":{"schema":{"type":"object"}}}}}}},"#));
    
    // Get vouches endpoint
    schema.append(&String::from_slice(&env, r#""/get_vouches/{borrower}":{"get":{"summary":"Get all vouches for a borrower","operationId":"get_vouches","parameters":[{"name":"borrower","in":"path","required":true,"schema":{"type":"string"}}],"responses":{"200":{"description":"List of vouches","content":{"application/json":{"schema":{"type":"array"}}}}}}},"#));
    
    // Search endpoint
    schema.append(&String::from_slice(&env, r#""/search":{"get":{"summary":"Search loans and vouches","operationId":"search","parameters":[{"name":"q","in":"query","required":true,"schema":{"type":"string"}},{"name":"limit","in":"query","schema":{"type":"integer","default":10}}],"responses":{"200":{"description":"Search results","content":{"application/json":{"schema":{"type":"object"}}}}}}},"#));
    
    // Metrics endpoint
    schema.append(&String::from_slice(&env, r#""/metrics":{"get":{"summary":"Get Prometheus metrics","operationId":"metrics","responses":{"200":{"description":"Prometheus metrics","content":{"text/plain":{"schema":{"type":"string"}}}}}}}}"#));
    
    schema.append(&String::from_slice(&env, r#"},"components":{"schemas":{"LoanRecord":{"type":"object","properties":{"id":{"type":"integer"},"borrower":{"type":"string"},"amount":{"type":"integer"},"amount_repaid":{"type":"integer"},"status":{"type":"string","enum":["Active","Repaid","Defaulted"]},"created_at":{"type":"integer"},"deadline":{"type":"integer"}}},"VouchRecord":{"type":"object","properties":{"voucher":{"type":"string"},"stake":{"type":"integer"},"vouch_timestamp":{"type":"integer"}}}}}}"#));
    
    schema
}

/// Generate API documentation in Markdown format
pub fn generate_api_documentation(env: Env) -> String {
    let mut doc = String::new(&env);
    
    doc.append(&String::from_slice(&env, "# QuorumCredit API Documentation\n\n"));
    doc.append(&String::from_slice(&env, "## Overview\n"));
    doc.append(&String::from_slice(&env, "QuorumCredit is a decentralized microlending platform on Stellar Soroban.\n\n"));
    
    doc.append(&String::from_slice(&env, "## Base URL\n"));
    doc.append(&String::from_slice(&env, "- Testnet: `https://soroban-testnet.stellar.org`\n"));
    doc.append(&String::from_slice(&env, "- Mainnet: `https://rpc.mainnet.stellar.org`\n\n"));
    
    doc.append(&String::from_slice(&env, "## Endpoints\n\n"));
    
    doc.append(&String::from_slice(&env, "### Initialize Contract\n"));
    doc.append(&String::from_slice(&env, "`POST /initialize`\n\n"));
    doc.append(&String::from_slice(&env, "Initialize the contract with admin addresses and configuration.\n\n"));
    doc.append(&String::from_slice(&env, "**Parameters:**\n"));
    doc.append(&String::from_slice(&env, "- `deployer` (string): Deployer address\n"));
    doc.append(&String::from_slice(&env, "- `admins` (array): List of admin addresses\n"));
    doc.append(&String::from_slice(&env, "- `admin_threshold` (integer): Required admin signatures\n"));
    doc.append(&String::from_slice(&env, "- `token` (string): Primary token address\n\n"));
    
    doc.append(&String::from_slice(&env, "### Vouch\n"));
    doc.append(&String::from_slice(&env, "`POST /vouch`\n\n"));
    doc.append(&String::from_slice(&env, "Stake XLM to vouch for a borrower.\n\n"));
    doc.append(&String::from_slice(&env, "**Parameters:**\n"));
    doc.append(&String::from_slice(&env, "- `voucher` (string): Voucher address\n"));
    doc.append(&String::from_slice(&env, "- `borrower` (string): Borrower address\n"));
    doc.append(&String::from_slice(&env, "- `stake` (integer): Stake amount in stroops\n"));
    doc.append(&String::from_slice(&env, "- `token` (string): Token address\n\n"));
    
    doc.append(&String::from_slice(&env, "### Request Loan\n"));
    doc.append(&String::from_slice(&env, "`POST /request_loan`\n\n"));
    doc.append(&String::from_slice(&env, "Request a loan if sufficient vouches exist.\n\n"));
    doc.append(&String::from_slice(&env, "**Parameters:**\n"));
    doc.append(&String::from_slice(&env, "- `borrower` (string): Borrower address\n"));
    doc.append(&String::from_slice(&env, "- `amount` (integer): Loan amount in stroops\n"));
    doc.append(&String::from_slice(&env, "- `threshold` (integer): Minimum stake required\n"));
    doc.append(&String::from_slice(&env, "- `loan_purpose` (string): Purpose of the loan\n"));
    doc.append(&String::from_slice(&env, "- `token` (string): Token address\n\n"));
    
    doc.append(&String::from_slice(&env, "### Repay Loan\n"));
    doc.append(&String::from_slice(&env, "`POST /repay`\n\n"));
    doc.append(&String::from_slice(&env, "Repay a loan and distribute yield to vouchers.\n\n"));
    doc.append(&String::from_slice(&env, "**Parameters:**\n"));
    doc.append(&String::from_slice(&env, "- `borrower` (string): Borrower address\n"));
    doc.append(&String::from_slice(&env, "- `payment` (integer): Payment amount in stroops\n\n"));
    
    doc.append(&String::from_slice(&env, "### Get Loan\n"));
    doc.append(&String::from_slice(&env, "`GET /get_loan/{borrower}`\n\n"));
    doc.append(&String::from_slice(&env, "Get loan details for a borrower.\n\n"));
    
    doc.append(&String::from_slice(&env, "### Get Vouches\n"));
    doc.append(&String::from_slice(&env, "`GET /get_vouches/{borrower}`\n\n"));
    doc.append(&String::from_slice(&env, "Get all vouches for a borrower.\n\n"));
    
    doc.append(&String::from_slice(&env, "### Search\n"));
    doc.append(&String::from_slice(&env, "`GET /search?q=query&limit=10`\n\n"));
    doc.append(&String::from_slice(&env, "Search loans and vouches.\n\n"));
    doc.append(&String::from_slice(&env, "**Query Parameters:**\n"));
    doc.append(&String::from_slice(&env, "- `q` (string): Search query\n"));
    doc.append(&String::from_slice(&env, "- `limit` (integer): Maximum results (default: 10, max: 100)\n\n"));
    
    doc.append(&String::from_slice(&env, "### Metrics\n"));
    doc.append(&String::from_slice(&env, "`GET /metrics`\n\n"));
    doc.append(&String::from_slice(&env, "Get Prometheus-compatible metrics.\n\n"));
    
    doc
}
