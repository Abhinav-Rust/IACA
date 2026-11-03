export type Decision = 'accepted' | 'rejected' | 'unresolved'
export type Provider = 'fixture' | 'meta' | 'manual'
export type Scenario = 'repair' | 'accepted' | 'rejected' | 'unresolved'
export interface Contract {
  id: string
  title: string
  instruction: string
  domain: string
  variables: string[]
  pattern: string
  reference: string
  requirements: string[]
  proof_scope: string
  fingerprint: string
}
export interface Health {
  status: string
  version: string
  engine_revision: string
  model: string
  meta_configured: boolean
  live_call_limit: number
  live_calls_used: number
  live_calls_remaining: number
  completion_token_limit: number
  max_attempts: number
  worker_deadline_seconds: number
}
export interface Gate { passed: boolean; details: string[] }
export interface TestCase {
  name: string
  input: string
  expected: string
  actual: string
  passed: boolean
  reference_steps: number
  candidate_steps: number
}
export interface Proof {
  verdict: string
  complete: boolean
  steps: number
  budget: number
  leaves: { kind: string; depth: number | null; ancestor: number | null; left: string | null; right: string | null }[]
  report: string
}
export interface Verification {
  decision: Decision
  reason: string
  candidate: string
  source: string | null
  boundary: Gate
  compiler: Gate | null
  transformation: Gate | null
  tests: { total: number; passed: number; cases: TestCase[]; oracle: string; reference_steps: number; candidate_steps: number } | null
  proof: Proof | null
  elapsed_ms: number
}
export interface Attempt {
  number: number
  candidate: string
  explanation: string
  provenance: string
  generation_ms: number
  usage: { prompt_tokens: number | null; completion_tokens: number | null; total_tokens: number | null; estimated_cost_usd?: number | null }
  verification: Verification
  feedback: string
}
export interface Run {
  id: string
  created_at: number
  finished_at: number | null
  task: string
  provider: Provider
  scenario: Scenario
  contract_fingerprint: string
  status: string
  phase: string
  decision: Decision | null
  attempts: Attempt[]
  events: { at_ms: number; message: string }[]
  error: string | null
  cancel_requested: boolean
}
