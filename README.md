# IACA — Intelligent Autonomous Coding Agent

**Code proposes. Evidence decides.**

IACA connects **Meta Muse Wonder** to a deterministic **Rust / Refal** checking
loop. The model proposes an implementation; the application evaluates it against
an immutable contract. Actual failures become feedback for another attempt,
within fixed resource limits.

The prototype includes a Rust backend, React workbench, actual Refal execution
and equivalence checking, an independent Rust test oracle, a bounded Meta
provider, and exportable evidence for every attempt.

The [recorded live Meta run](docs/evidence/meta-live.json) succeeded on its first
proposal: 216 execution cases passed and equivalence was proved in 34 driving
steps. Reported usage was 0.000769M tokens, with an uncached-price estimate of
$0.007348. This is one demonstration, not a model-quality benchmark.

## Run locally

Requires a current stable Rust toolchain and Node.js 22.12 or later. Cargo fetches
the engine at a pinned Git revision; no separate engine installation is required.

```sh
git clone https://github.com/Abhinav-Rust/IACA.git
cd IACA
npm --prefix web ci
npm --prefix web run build
cargo run --locked -- serve
```

Open **http://127.0.0.1:8787**. Choose **Demo fixtures → Reject → revise → accept**
to run the complete checking loop without an API key or API charges. The same
commands work in PowerShell. Use SSH forwarding to access a remote server.

## Connect Meta

Set `MODEL_API_KEY` through your server environment or secret manager. Do not
put the key in source, screenshots or a public repository. The application does
not automatically load `.env` files.

Live calls are **disabled by default**. After configuring the key, allow a single
request:

```sh
IACA_LIVE_CALL_LIMIT=1 cargo run --locked -- serve
```

In PowerShell:

```powershell
$env:IACA_LIVE_CALL_LIMIT = "1"
cargo run --locked -- serve
```

Choose **Meta Wonder → Start Meta run**. The provider calls
`https://api.meta.ai/v1/chat/completions` using `muse-wonder-1.0`, minimal
reasoning, and a JSON Schema for the candidate response. The verification worker
never receives the API credential.

The process reserves a call before sending it, including failed calls. There
are no automatic HTTP retries. The configured allowance has a hard maximum of
two calls per process; each run has at most two attempts. Requests are limited
to 12 KB and **0.002048M completion tokens**. Restarting resets the process
allowance; return `IACA_LIVE_CALL_LIMIT` to `0` after your demonstration.

The UI shows actual usage when returned by Meta. Cost is an uncached-price
estimate using [published rates](https://dev.meta.ai/docs/pricing-rate-limits)
checked on 3 November 2026: $4 per 1M input tokens and $20 per 1M output tokens.
The application does not read your billing balance.

## Supported tasks

| Task | Required transformation | Execution cases |
| --- | --- | ---: |
| Three-list concatenation | Reassociate to the right, preserving order | 216 |
| Empty-list identity | Remove an unnecessary append | 6 |
| Four-list concatenation | Reassociate to the right, preserving order | 1,296 |

Only the candidate **right-hand expression** can change. The application owns
the input pattern, reference, helpers, oracle and acceptance rules. AST validation
permits bracketed data and total calls to the fixed `Append` helper. It excludes
arbitrary functions, directives, conditions, builtins, executable blocks and
model-controlled recursion.

Five gates inspect each attempt: expression boundary, strict compiler checks,
requested structural transformation, execution against an independent oracle,
and symbolic equivalence. A credential-free worker has an eight-second deadline
and bounded output.

| Decision | Meaning |
| --- | --- |
| **Accepted** | Every required gate passed and the equivalence proof completed. |
| **Rejected** | A contract violation, compiler failure, test counterexample or symbolic refutation was found. |
| **Unresolved** | Required evidence is incomplete or unavailable; passing tests cannot replace a missing proof. |

Fixture modes are explicitly labelled. Incorrect proposals are deliberate
mutations and fixture revisions are scripted, not evidence of actual model
mistakes or repairs. Every fixture still runs the real engine. The unresolved
fixture uses a real zero-step symbolic budget.

This research prototype supports finite Refal lists, including nested terms.
It uses the engine's **equivalence** API, not its predicate-proof mode. It does
not certify arbitrary source code. Interpreter reduction counts are reported
separately from latency and are not general performance claims.

## Inspect and reproduce

The workbench exposes attempts, source differences, test counterexamples, proof
branches, exact revision feedback, history and JSON exports. Completed runs
are saved in the ignored `runs/` directory; reopening history makes no model call.

```sh
cargo run --locked -- verify-demo accepted
cargo run --locked -- verify-demo rejected
cargo run --locked -- verify-demo unresolved
cargo run --locked -- verify '<Append (e.A) <Append (e.B) (e.C)>>'
```

Verification prints JSON and exits `0` for accepted, `1` for rejected and `2`
for unresolved. Nonzero exits from rejected/unresolved demos are expected.

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
npm --prefix web run build
```

Browser tests require a running server and use only fixtures/manual candidates:

```sh
cd web
npx playwright install chromium
npm run test:e2e
```

Set `IACA_TEST_URL` for a forwarded server or `IACA_CHROME_PATH` for an existing
Chrome executable. See [architecture](docs/architecture.md) and
[the demonstration guide](docs/demo.md).

## Foundation and new contribution

[Refal Supersystem](https://github.com/Abhinav-Rust/REFAL-SUPERSYSTEM) is the
pre-existing MIT-licensed engine, pinned to
[`c8f9542`](https://github.com/Abhinav-Rust/REFAL-SUPERSYSTEM/commit/c8f95421350b6d7d50a01e83278da4a33fae3b61),
version 0.10.0. It supplies parsing, semantic analysis, execution and supported
symbolic machinery, building on Valentin Turchin's Refal and supercompilation
work.

IACA's new contribution is the Meta implementation/feedback loop, constrained
candidate boundary, independent oracle and acceptance policy, process isolation,
credit controls, evidence records and React workbench. The prior engine is
attributed as a foundation, not presented as new hackathon work.

MIT license. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
