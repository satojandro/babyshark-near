//! BabyShark escrow — SPIKE.
//!
//! Proves the round trip the real escrow depends on:
//! contract → OutLayer `request_execution` (yield) → TEE verifier checks GitHub
//! → OutLayer resumes → our callback receives the attested report.
//!
//! No money moves yet. The next step adds per-milestone NEP-141 balances and
//! releases them only inside `on_check`.

use near_sdk::store::LookupMap;
use near_sdk::{env, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseError};

/// OutLayer's minimum deposit for an execution (unused part is refunded).
const MIN_DEPOSIT: NearToken = NearToken::from_millinear(10);
const CALLBACK_GAS: Gas = Gas::from_tgas(10);

#[near(serializers = [json])]
#[derive(Clone)]
pub struct CheckRequest {
    pub repo: String,
    pub github_login: String,
    pub since: String,
}

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct CheckResult {
    pub requested_by: AccountId,
    pub block_height: u64,
    /// `true` only if OutLayer reported success AND the verifier said `ok`.
    pub ok: bool,
    /// Verifier output exactly as OutLayer delivered it (report or error).
    pub raw: String,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Escrow {
    owner: AccountId,
    outlayer: AccountId,
    verifier_url: String,
    verifier_sha256: String,
    last: LookupMap<AccountId, CheckResult>,
}

#[near]
impl Escrow {
    #[init]
    pub fn new(outlayer: AccountId, verifier_url: String, verifier_sha256: String) -> Self {
        Self {
            owner: env::predecessor_account_id(),
            outlayer,
            verifier_url,
            verifier_sha256,
            last: LookupMap::new(b"l"),
        }
    }

    /// Pin a new verifier build. Owner only; the hash is what OutLayer enforces.
    pub fn set_verifier(&mut self, verifier_url: String, verifier_sha256: String) {
        require!(env::predecessor_account_id() == self.owner, "owner only");
        self.verifier_url = verifier_url;
        self.verifier_sha256 = verifier_sha256;
    }

    /// Ask the TEE verifier about `github_login`'s commits in `repo` since `since`.
    /// Attach at least 0.01 NEAR (OutLayer fee; unused part refunded to the caller).
    #[payable]
    pub fn request_check(&mut self, request: CheckRequest) -> Promise {
        let deposit = env::attached_deposit();
        require!(deposit >= MIN_DEPOSIT, "attach at least 0.01 NEAR for OutLayer");
        let caller = env::predecessor_account_id();

        let source = serde_json::json!({
            "WasmUrl": {
                "url": self.verifier_url,
                "hash": self.verifier_sha256,
                "build_target": "wasm32-wasip2"
            }
        });
        let limits = serde_json::json!({
            "max_instructions": 10_000_000_000u64,
            "max_memory_mb": 128,
            "max_execution_seconds": 60
        });
        let input = serde_json::json!({
            "repo": request.repo,
            "github_login": request.github_login,
            "since": request.since,
        })
        .to_string();

        let outlayer_gas = env::prepaid_gas()
            .saturating_sub(CALLBACK_GAS)
            .saturating_sub(Gas::from_tgas(10));

        Promise::new(self.outlayer.clone())
            .function_call_weight(
                "request_execution".to_string(),
                serde_json::json!({
                    "source": source,
                    "resource_limits": limits,
                    "input_data": input,
                    "secrets_ref": null,
                    "response_format": "Text",
                    "payer_account_id": caller,
                    "params": null,
                })
                .to_string()
                .into_bytes(),
                deposit,
                outlayer_gas,
                near_sdk::GasWeight(1),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(CALLBACK_GAS)
                    .on_check(caller),
            )
    }

    /// OutLayer resumes here with the verifier's stdout.
    #[private]
    pub fn on_check(
        &mut self,
        requested_by: AccountId,
        #[callback_result] result: Result<Option<String>, PromiseError>,
    ) -> CheckResult {
        let (ok, raw) = match result {
            Ok(Some(out)) => (verifier_said_ok(&out), out),
            Ok(None) => (false, "OutLayer returned no output".to_string()),
            Err(_) => (false, "OutLayer execution failed".to_string()),
        };
        let res = CheckResult {
            requested_by: requested_by.clone(),
            block_height: env::block_height(),
            ok,
            raw,
        };
        env::log_str(&format!("babyshark check for {requested_by}: ok={ok}"));
        self.last.insert(requested_by, res.clone());
        res
    }

    pub fn get_last_check(&self, account_id: AccountId) -> Option<CheckResult> {
        self.last.get(&account_id).cloned()
    }

    pub fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "owner": self.owner,
            "outlayer": self.outlayer,
            "verifier_url": self.verifier_url,
            "verifier_sha256": self.verifier_sha256,
        })
    }
}

/// Accepts the verifier's own `{"ok":true,...}` or OutLayer's
/// `{"success":true,"output":{"ok":true,...}}` wrapper.
fn verifier_said_ok(out: &str) -> bool {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(out) else {
        return false;
    };
    let inner = if v.get("success").is_some() {
        if v["success"] != serde_json::Value::Bool(true) {
            return false;
        }
        match &v["output"] {
            serde_json::Value::String(s) => serde_json::from_str(s).unwrap_or_default(),
            other => other.clone(),
        }
    } else {
        v
    };
    inner["ok"] == serde_json::Value::Bool(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn setup() -> Escrow {
        testing_env!(VMContextBuilder::new().predecessor_account_id(accounts(0)).build());
        Escrow::new("outlayer.testnet".parse().unwrap(), "https://x/v.wasm".into(), "ab".into())
    }

    #[test]
    fn parses_verifier_output_shapes() {
        assert!(verifier_said_ok(r#"{"ok":true,"report":{}}"#));
        assert!(!verifier_said_ok(r#"{"ok":false,"error":"x"}"#));
        assert!(verifier_said_ok(r#"{"success":true,"output":{"ok":true}}"#));
        assert!(verifier_said_ok(r#"{"success":true,"output":"{\"ok\":true}"}"#));
        assert!(!verifier_said_ok(r#"{"success":false,"output":{"ok":true}}"#));
        assert!(!verifier_said_ok("not json"));
    }

    #[test]
    fn only_owner_can_repin_verifier() {
        let mut c = setup();
        c.set_verifier("https://y/v.wasm".into(), "cd".into());
        assert_eq!(c.get_config()["verifier_sha256"], "cd");
    }

    #[test]
    #[should_panic(expected = "owner only")]
    fn stranger_cannot_repin_verifier() {
        let mut c = setup();
        testing_env!(VMContextBuilder::new().predecessor_account_id(accounts(1)).build());
        c.set_verifier("https://evil/v.wasm".into(), "ee".into());
    }

    #[test]
    #[should_panic(expected = "attach at least 0.01 NEAR")]
    fn check_requires_deposit() {
        let mut c = setup();
        let _ = c.request_check(CheckRequest { repo: "a/b".into(), github_login: "a".into(), since: "2026-11-01T00:00:00Z".into() });
    }
}
