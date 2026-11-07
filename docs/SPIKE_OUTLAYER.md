# Spike: NEAR contract → OutLayer TEE → GitHub → callback (PASSED, 2026-11-07)

Goal: prove the round trip the escrow depends on before writing money logic.

## Result

One testnet transaction went contract → OutLayer (yield) → verifier in Intel TDX
→ GitHub API → OutLayer (resume) → our callback, and returned the report on-chain.

| | |
|---|---|
| Contract | [`babyshark-spike-1107.testnet`](https://testnet.nearblocks.io/address/babyshark-spike-1107.testnet) (code: `contracts/escrow`) |
| Deploy tx | [`Hv86pcc2uGVnmgC19GXCf98iakfckmG1qU99u48RWY4X`](https://testnet.nearblocks.io/txns/Hv86pcc2uGVnmgC19GXCf98iakfckmG1qU99u48RWY4X) |
| OutLayer request | `request_id` 12799 on `outlayer.testnet`, callback at block 276686996 |
| Verifier build | `verifier/dist/babyshark-verifier.wasm`, sha256 `944d4b1ccc819dd8dd4b8012e27137a4096d0b430aa370b78a4efdd9642c47c2` (OutLayer refuses bytes that don't match) |
| Input | `{"repo":"satojandro/pocketvc","github_login":"satojandro","since":"2026-08-01T00:00:00Z"}` |
| Output | `ok: true`, `commits_by_login: 57`, `latest_sha: f8b484f…`, 20 SHAs as evidence |
| Cost | 0.0012 NEAR charged of 0.02 attached (rest refunded) |
| Time in enclave | 1.3 s (plus 0.7 s first-run compile); 8.6 M instructions |

## Reproduce

```bash
# read the stored result (no signing)
near contract call-function as-read-only babyshark-spike-1107.testnet \
  get_last_check json-args '{"account_id":"babyshark-spike-1107.testnet"}' \
  network-config testnet now
```

To run a new check you need a funded testnet account:

```bash
near contract call-function as-transaction babyshark-spike-1107.testnet request_check \
  json-args '{"request":{"repo":"OWNER/REPO","github_login":"LOGIN","since":"2026-11-01T00:00:00Z"}}' \
  prepaid-gas '300 Tgas' attached-deposit '0.02 NEAR' \
  sign-as YOUR_ACCOUNT.testnet network-config testnet sign-with-keychain send
```

The callback stores results per caller; read yours with `get_last_check`.

## What we learned

- **Hosting the WASM:** a public `raw.githubusercontent.com` URL pinned to a commit
  works as `WasmUrl`. No OutLayer CLI or FastFS upload needed. The repo must be public.
- **Root `.gitignore` had `dist/`**, which silently excluded the build. It's now
  explicitly un-ignored for `verifier/dist/`.
- **Cost and time are tiny**, so an LLM call inside the 60 s default limit is realistic.
- **GitHub rate limits:** unauthenticated calls share a 60/hour per-IP budget. The
  verifier already reads an optional `GITHUB_TOKEN` secret; set one before the demo.
- **Trust boundary:** the contract accepts the result because it arrives in the
  resume from `outlayer.testnet`. OutLayer's workers are TEE-attested; per-run
  attestation is visible in the OutLayer dashboard (Executions). Next step: surface
  the attestation link in the UI.
- **Dev account key** lives only in the cloud container that created it. If lost,
  create a new faucet account and redeploy (`cargo near build non-reproducible-wasm`).

## Next

Escrow v1: per-milestone NEP-141 balances funded by the parent via
`ft_transfer_call`, released only inside `on_check`, with caps, kid pinning,
parent approval above a threshold and refunds. See `NEAR_PLAN.md`.
