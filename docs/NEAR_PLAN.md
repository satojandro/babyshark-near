# BabyShark on NEAR — build plan

NEAR HACKATHON 2026 · development window ends **2026-11-14T23:59Z** ·
rules: https://nearhackathon.com/HACKATHON.md

## Thesis

> The AI decides *who deserves* the reward. Attested code checks the work is
> real. A contract decides *how much can move*.

The Aleph version proved the loop with an LLM, an off-chain policy engine and a
server-held pool wallet. On NEAR, the trust-critical parts move on-chain and
into a TEE:

| Concern | Aleph baseline | NEAR version |
|---|---|---|
| Where funds sit | Server-held WDK pool wallet | `babyshark` escrow contract, per-milestone NEP-141 (USDT) balance |
| Who can create milestones | Anyone (no auth on API routes) | Only the parent's NEAR account (`ft_transfer_call` funds it) |
| Payout recipient | From a JSON file | Kid's NEAR account pinned on-chain at creation |
| "Is the work real?" | LLM reads GitHub in our server | OutLayer WASI verifier in Intel TDX: GitHub facts + LLM verdict, attested |
| Large payouts | `HOLD_FOR_PARENT`, never resolvable | Contract waits for the parent's signature (`parent_approve`) |
| Agent keys | None (good) | None — the contract only accepts the OutLayer callback |
| Mentor chat | Next.js + OpenRouter | Next.js + NEAR AI Cloud (not trust-critical) |

## Architecture

```
Parent (NEAR wallet) ──ft_transfer_call(USDT, msg=milestone)──▶ babyshark contract
Kid ──chat──▶ mentor agent (web)                                 │ kid account, GitHub handle, repo, window pinned
                    └── request_verdict(milestone_id) ──────────▶│
                                                                 │──request_execution──▶ OutLayer (yield)
                                                                 │                       TEE verifier (WASI, Rust)
                                                                 │                       • GitHub API: commits by handle in window
                                                                 │                       • LLM verdict on diff (sealed key)
                                                                 │◀──resume: {verdict, amount, evidence_hash}
                                                                 ├─ amount ≤ threshold → ft_transfer to kid
                                                                 └─ amount > threshold → pending until parent_approve
```

Contract rules (deterministic, ported from `src/policy/engine.ts`):
amount > 0 · paid + amount ≤ budget · recipient = pinned kid · milestone open ·
only the OutLayer callback can release · parent can cancel → refund remainder ·
milestone auto-closes when fully paid.

## Decision: escrow contract, not an OutLayer custody wallet, holds the money

OutLayer Agent Custody (skill: https://skills.outlayer.ai/agent-custody/SKILL.md)
was evaluated as the escrow instead of writing a contract:

- **Custody wallet + policy:** caps, kid-only allowlist and parent approval above a
  threshold come for free, and it works on testnet. But whoever holds the `wk_`
  API key can transfer at any time within the policy. Verification would be a
  convention, not a rule: a prompt-injected agent could pay the kid up to the
  cap without any work being checked. That breaks the core thesis.
- **Payment checks** (the skill's "milestone payments" pattern): funds are claimed
  by whoever holds the bearer `check_key`, and they settle through NEAR Intents,
  which are mainnet-only. Neither fits.
- **Escrow contract:** funds move only in the callback carrying OutLayer's
  attested verdict. Verification is enforced by the chain. **Chosen.**

Custody stays a stretch goal for the agent's own operating budget (paying for
OutLayer runs and NEAR AI calls), where a capped hot wallet is the right tool.

## Schedule

| Day | Work | Done when |
|---|---|---|
| Nov 7–8 | ✅ Spike: OutLayer WASI verifier called from a testnet contract via yield/resume ([result](SPIKE_OUTLAYER.md)) | Callback receives attested output on testnet |
| Nov 8–10 | Escrow contract + unit/sandbox tests; mock USDT NEP-141 on testnet | Fund → verdict stub → payout / hold / refund all tested |
| Nov 10–11 | Verifier: GitHub checks + LLM verdict, secrets in OutLayer | Real repo yields attested verdict that the contract honours |
| Nov 12–13 | Web: NEAR wallet login, dashboard reads contract, mentor chat on NEAR AI Cloud | Full loop clickable on testnet |
| Nov 14 | Prompt-injection demo, README for judges, video, X post, submit | Submission returns 201 |

Cut first if late: web port (fall back to CLI + dashboard screenshots + video).

## Tracks (one submission, several tracks)

`security` (primary: AI with a checkbook but no keys), `realworld`, `privacy`, `open`.

## Open questions

- OutLayer execution limit is 180 s max (60 s default) — confirm the LLM call fits.
- USDT on testnet: deploy a mock NEP-141; NEAR Intents is mainnet-only (stretch goal).
- Minors and money: frame the kid account as parent-controlled for the demo.

## Needs from the human

- [ ] NEAR testnet account (dev) and mainnet account (prize payout)
- [ ] Log in at app.outlayer.ai (testnet) with the dev account
- [ ] Optional: NEAR AI Cloud credits (https://nearbuilders.org/join)
- [ ] Install the Claude GitHub App on this repo so the session can push
