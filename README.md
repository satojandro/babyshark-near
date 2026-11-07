# BabyShark VC 🦈👶

**An AI mentor-investor that reviews kids' coding work and pays out USDT when milestones are hit.**

> ## NEAR HACKATHON 2026 — what's new here
>
> This repository continues **BabyShark VC**, which won first place at
> [Aleph Hackathon 2026](https://hacki.crecimiento.build/h/aleph-hackathon-2026)
> (Tether WDK track, original repo: [satojandro/pocketvc](https://github.com/satojandro/pocketvc)).
>
> - **Baseline (not judged):** everything up to the tag
>   [`pre-hackathon`](https://github.com/satojandro/babyshark-near/tree/pre-hackathon)
>   — the EVM/Sepolia version described in the rest of this README.
> - **Built during NEAR HACKATHON (judged):**
>   [`pre-hackathon...main`](https://github.com/satojandro/babyshark-near/compare/pre-hackathon...main)
>   — moving escrow, verification and payouts onto NEAR + OutLayer.
>
> Plan and status: [`docs/NEAR_PLAN.md`](docs/NEAR_PLAN.md). This section will be
> replaced by NEAR-first "do X → see Y" instructions before submission.

## Why — Mateo's summer

Mateo (12) spent the summer wanting to build a Roblox game. His dad wanted to encourage him but can't read code — no way to tell progress from play. Meanwhile Mateo earned $5 taking out the trash: money for work that teaches nothing. Two gaps: **visibility** (nobody who understands the work is watching) and **funding trust** (money given on faith). BabyShark closes both: an AI mentor reviews the real code daily, and USDT payouts release only when milestones verify. Anyone can back a kid's project via QR.

> The parent sets the budget. The AI judges the work. Code controls how much can move.
> **The AI decides *who deserves* the reward; deterministic code decides *how much can move*.**

Not hostile like Shark Tank — a supportive coach who happens to hold the checkbook. The goal: the kid *wants* to show their work, because finally someone gets it.

## How it works

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for full diagrams (system map + agent anatomy).

```
Kid ──pitch──▶ Shark Agent (LLM reasoning)
                    │ propose_payout()
                    ▼
              Policy Engine (deterministic caps & checks — no AI)
                    │ approved
                    ▼
              Project Pool (platform-held escrow) ──USDT──▶ kid wallet (self-custodial)
```

- **Shark Agent** — LLM (Vercel AI SDK): reviews commits/diffs via GitHub API (`review_repo`), asks curiosity-driven questions, mentors between milestones with persistent memory, drafts milestone proposals with learning outcomes for parents, renders verdicts. Has NO send tool and no keys — only `propose_payout()`.
- **Policy Engine** — plain TypeScript, zero AI: enforces milestone budgets, duplicate-payout protection, recipient binding, parent-confirm thresholds. Every proposal/decision logged to an append-only audit file.
- **Wallet layer** — Tether WDK: three-wallet custody model (see below).

## Three-wallet custody model

| Wallet | Keys held by | Purpose | Seed shown? |
|---|---|---|---|
| Treasury | Parent (client-side) | Parent's own spending funds | Once |
| Kid wallet | Kid (client-side) | Reward destination — fully theirs | Once |
| Project pool | Server only | Functional escrow: committed + crowdfunded funds; exits ONLY via policy-approved payouts to the kid's pinned address | Never |

Production upgrade: swap the custodial pool for an escrow smart contract (see [ROADMAP](docs/ROADMAP.md)).

## WDK packages used

Track requirement: `@tetherto/wdk` as core dependency, `@tetherto/wdk-cli` as core building block.

| Package | Version | Where used |
|---|---|---|
| `@tetherto/wdk-cli` | 1.0.0-beta.3 | Wallet creation/unlock daemon, address derivation, balances, USDT transfers (`wdk send --json` receipts), token registry, MoonPay fiat module |
| `@tetherto/wdk-wallet-evm` | 1.0.0-beta.17 | Client-side key generation in setup wizard (browser), address derivation verified byte-for-byte against CLI |

**WDK integration permalinks** (judges look here first):
- Payout execution (only money path): [`web/src/lib/wallet-exec.ts`](web/src/lib/wallet-exec.ts)
- Agent's propose-only gateway: [`web/src/lib/tools.ts`](web/src/lib/tools.ts) → `propose_payout`
- Policy engine (no AI, unit-tested): [`src/policy/engine.ts`](src/policy/engine.ts)
- Pool wallet (functional escrow): [`web/src/lib/pool.ts`](web/src/lib/pool.ts)

## What's demonstrably working

- ✅ End-to-end payout chain on Sepolia: MockUSDT [`0x4804…ed4b4`](https://sepolia.etherscan.io/address/0x4804e2dad314454739600cf4be3b8290427ed4b4), treasury→kid transfers confirmed
- ✅ Multi-turn agent session with repo verification, milestone proposals, policy holds, real on-chain payouts
- ✅ Bluff detection against live GitHub data
- ✅ Client-side key generation verified byte-for-byte against WDK SDK derivation
- ✅ 10 policy-engine unit tests · full audit trail

## Run it

```bash
git clone https://github.com/satojandro/pocketvc.git && cd pocketvc/web
npm install
cp ../.env.example .env.local   # add OpenRouter key + wallet addresses
npm run dev                      # http://localhost:3000
```

Full setup guide: [`docs/SETUP.md`](docs/SETUP.md) · For judges: [`docs/JUDGES_GUIDE.md`](docs/JUDGES_GUIDE.md)

## Built for

[Aleph Hackathon 2026](https://hacki.crecimiento.build/h/aleph-hackathon-2026) — Tether WDK Track · Team: Alejandro Vasquez Betancourt
