# IACA — Intelligent Autonomous Coding Agent

IACA explores a Meta-powered coding agent that combines generated implementations
with deterministic analysis and verification through
[Refal Supersystem](https://github.com/Abhinav-Rust/REFAL-SUPERSYSTEM).

The intended workflow takes a supported coding task through implementation,
automated checks, evidence-based feedback and revision. Each attempt has an
explicit outcome: **accepted**, **rejected** or **unresolved**.

## Current status

Initial project scaffold, created on 3 November 2026. The application, Meta API
integration, checking adapter and interface are planned. This repository
currently contains project documentation.

## Starting scope

The proposed first task family is small Refal list-processing functions with a
fixed input domain and reference behavior. An initial example is simplifying
nested list concatenation while preserving element order, empty-list behavior
and nested elements.

For each task, the reference, required checks and acceptance policy stay fixed
while the model revises its candidate.

## Planned workflow

1. Define a task and its explicit constraints.
2. Ask a Meta model to generate an implementation.
3. Run compiler checks, execution tests and supported symbolic analyses.
4. Feed the actual findings back to the model for revision.
5. Record the code, evidence, resource use and final decision.

Test results, proof verdicts and performance measurements will be presented
separately. Required proofs that remain open or incomplete leave the attempt
unresolved.

## Development setup

Clone this repository into your projects directory:

```sh
git clone https://github.com/Abhinav-Rust/IACA.git
cd IACA
```

The application runtime and launch command will be added with its implementation.

## Existing foundation and new work

[Refal Supersystem](https://github.com/Abhinav-Rust/REFAL-SUPERSYSTEM) is the
pre-existing Rust implementation of the compiler, runtime, reflection,
supercompilation and supported proof machinery. Its reviewed baseline is
[`c8f9542`](https://github.com/Abhinav-Rust/REFAL-SUPERSYSTEM/commit/c8f95421350b6d7d50a01e83278da4a33fae3b61),
version 0.10.0, under its own MIT license.

The planned hackathon contribution is the Meta coding loop, checking adapter,
acceptance policy, inspection interface and evaluation, together with any engine
repairs completed for this prototype.
