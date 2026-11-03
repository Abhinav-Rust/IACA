import { useCallback, useEffect, useRef, useState } from 'react'
import { Activity, ArrowDown, ArrowRight, ArrowUpRight, BookOpen, Braces, Check, CheckCircle2, ChevronDown, CircleDashed, Clock3, Code2, Copy, Download, FlaskConical, GitBranch, History, Info, Layers3, LoaderCircle, LockKeyhole, Play, RefreshCcw, ShieldCheck, Square, Terminal, Workflow, X, XCircle, Zap } from 'lucide-react'
import type { Attempt, Contract, Decision, Health, Provider, Run, Scenario } from './types'

async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`/api${path}`, init)
  if (!response.ok) {
    const content = await response.text()
    let message = content
    try { message = JSON.parse(content).error ?? content } catch { /* Non-JSON protocol error. */ }
    throw new Error(message || `Request failed (${response.status})`)
  }
  return response.json() as Promise<T>
}

function Badge({ decision, small = false }: { decision: Decision | null; small?: boolean }) {
  const Icon = decision === 'accepted' ? CheckCircle2 : decision === 'rejected' ? XCircle : CircleDashed
  return <span className={`badge ${decision ?? 'pending'} ${small ? 'small' : ''}`}><Icon size={small ? 12 : 14} />{decision ?? 'Awaiting evidence'}</span>
}

function Refal({ text }: { text: string }) {
  return <code>{text.split(/(e\.[A-Za-z]+|Append(?:-Contents)?|[<>()[\]{};=])/g).map((part, index) => <span key={index} className={part.startsWith('e.') ? 'syntax-variable' : part.startsWith('Append') ? 'syntax-function' : /^[<>()[\]{};=]$/.test(part) ? 'syntax-punctuation' : undefined}>{part}</span>)}</code>
}

const scenarioLabels: Record<Scenario, string> = { repair: 'Reject → revise → accept', accepted: 'Correct transformation', rejected: 'Deliberately incorrect', unresolved: 'Insufficient proof budget' }
const tokenCount = (count: number) => `${(count / 1_000_000).toFixed(6)}M`

export default function App() {
  const [contracts, setContracts] = useState<Contract[]>([])
  const [health, setHealth] = useState<Health | null>(null)
  const [history, setHistory] = useState<Run[]>([])
  const [taskId, setTaskId] = useState('append_three')
  const [provider, setProvider] = useState<Provider>('fixture')
  const [scenario, setScenario] = useState<Scenario>('repair')
  const [candidate, setCandidate] = useState('<Append (e.A) <Append (e.B) (e.C)>>')
  const [run, setRun] = useState<Run | null>(null)
  const [selectedAttempt, setSelectedAttempt] = useState(0)
  const [pending, setPending] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [drawer, setDrawer] = useState<'history' | 'about' | null>(null)
  const [evidenceTab, setEvidenceTab] = useState<'tests' | 'proof' | 'feedback' | 'source'>('tests')
  const [copied, setCopied] = useState(false)
  const runRef = useRef<Run | null>(null)
  const active = pending || run?.status === 'running'
  const contract = contracts.find(item => item.id === taskId)
  const attempt = run?.attempts[selectedAttempt]
  const verification = attempt?.verification
  const liveAvailable = !!health?.meta_configured && health.live_calls_remaining > 0

  useEffect(() => {
    if (!drawer) return
    const previous = document.activeElement as HTMLElement | null
    const panel = document.querySelector<HTMLElement>('.drawer')
    const controls = () => Array.from(panel?.querySelectorAll<HTMLElement>('button:not(:disabled), a[href], select:not(:disabled), textarea:not(:disabled)') ?? [])
    controls()[0]?.focus()
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setDrawer(null)
      if (event.key !== 'Tab') return
      const items = controls()
      const first = items[0], last = items[items.length - 1]
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus() }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus() }
    }
    document.addEventListener('keydown', handleKey)
    return () => { document.removeEventListener('keydown', handleKey); previous?.focus() }
  }, [drawer])

  const refresh = useCallback(async () => {
    const [status, runs] = await Promise.all([api<Health>('/health'), api<Run[]>('/runs')])
    setHealth(status)
    setHistory(runs)
    return runs
  }, [])

  useEffect(() => {
    let stopped = false
    Promise.all([api<Contract[]>('/tasks'), refresh()]).then(([tasks, runs]) => {
      if (stopped) return
      setContracts(tasks)
      const current = runs.find(item => item.status === 'running')
      if (current) { setRun(current); runRef.current = current; setTaskId(current.task); setProvider(current.provider); setScenario(current.scenario) }
    }).catch(cause => { if (!stopped) setError(`Cannot reach IACA: ${String(cause.message)}`) })
    return () => { stopped = true }
  }, [refresh])

  useEffect(() => {
    if (run?.status !== 'running') return
    let stopped = false
    const timer = window.setInterval(async () => {
      try {
        const updated = await api<Run>(`/runs/${run.id}`)
        if (stopped) return
        if (updated.attempts.length > (runRef.current?.attempts.length ?? 0)) setSelectedAttempt(updated.attempts.length - 1)
        runRef.current = updated
        setRun(updated)
        if (updated.status !== 'running') await refresh()
      } catch (cause) { if (!stopped) setError((cause as Error).message) }
    }, 450)
    return () => { stopped = true; window.clearInterval(timer) }
  }, [run?.id, run?.status, refresh])

  async function start() {
    setPending(true); setError(null); setSelectedAttempt(0); setEvidenceTab('tests')
    try {
      const created = await api<Run>('/runs', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ task: taskId, provider, scenario: provider === 'fixture' ? scenario : 'repair', candidate: provider === 'manual' ? candidate : null }) })
      setRun(created); runRef.current = created
    } catch (cause) { setError((cause as Error).message) }
    finally { setPending(false) }
  }

  async function cancel() {
    if (!run) return
    try { await api(`/runs/${run.id}/cancel`, { method: 'POST' }); const updated = await api<Run>(`/runs/${run.id}`); setRun(updated) }
    catch (cause) { setError((cause as Error).message) }
  }

  function chooseTask(id: string) {
    setTaskId(id); setRun(null); runRef.current = null
    setCandidate(id === 'append_identity' ? '(e.A)' : id === 'append_four' ? '<Append (e.A) <Append (e.B) <Append (e.C) (e.D)>>>' : '<Append (e.A) <Append (e.B) (e.C)>>')
  }

  function openRun(item: Run) {
    setRun(item); runRef.current = item; setTaskId(item.task); setProvider(item.provider); setScenario(item.scenario)
    setSelectedAttempt(Math.max(0, item.attempts.length - 1)); setDrawer(null); setError(null)
  }

  async function copyCandidate() {
    if (!attempt) return
    try { await navigator.clipboard.writeText(attempt.candidate); setCopied(true); window.setTimeout(() => setCopied(false), 1500) }
    catch { setError('Clipboard unavailable. You can select and copy the expression directly.') }
  }

  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="/" aria-label="IACA home"><img src="/favicon.svg" alt="" /><span>IACA<span className="brand-period">.</span></span><span className="version">01</span></a>
      <div className="sidebar-caption">DEVELOPMENT WORKSPACE</div>
      <nav aria-label="Main navigation">
        <button className={!drawer ? 'nav-item selected' : 'nav-item'} onClick={() => setDrawer(null)}><Workflow size={18} />Workbench<span className="nav-dot" /></button>
        <button className={drawer === 'history' ? 'nav-item selected' : 'nav-item'} onClick={() => setDrawer('history')}><History size={18} />Run history<span className="nav-count">{history.length}</span></button>
        <button className={drawer === 'about' ? 'nav-item selected' : 'nav-item'} onClick={() => setDrawer('about')}><BookOpen size={18} />How it works<ArrowUpRight size={14} className="nav-end" /></button>
      </nav>
      <div className="sidebar-divider" />
      <div className="sidebar-caption">THE CODING LOOP</div>
      <div className="loop-nav">
        <div><span>01</span><p>Propose<small>Meta Wonder</small></p></div><i />
        <div><span>02</span><p>Verify<small>Rust / Refal engine</small></p></div><i />
        <div><span>03</span><p>Revise<small>Evidence as feedback</small></p></div>
      </div>
      <div className="sidebar-bottom"><div className="engine-status"><span className={`status-light ${health ? 'online' : ''}`} /><span>Deterministic engine<small>{health ? 'Connected · Refal v0.10.0' : 'Connecting…'}</small></span></div><div className="hackathon-label">META GLOBAL AI<br /><strong>DEVELOPER HACKATHON</strong><span>2026 ↗</span></div></div>
    </aside>

    <div className="main-shell">
      <header className="topbar"><div><span className="workspace-icon"><Braces size={15} /></span><span>IACA workspace</span><span className="breadcrumb-slash">/</span><strong>Workbench</strong></div><div className="topbar-right"><span className="private-label"><LockKeyhole size={12} />Local workspace</span><span className="avatar">AR</span></div></header>
      <main>
        <div className="page-heading"><div><div className="eyebrow"><span /> INTELLIGENT AUTONOMOUS CODING AGENT</div><h1>Code proposes.<br className="mobile-break" /> Evidence decides<span>.</span></h1><p>A coding loop you can inspect, from the first proposal to the final proof.</p></div><div className="heading-stamp"><ShieldCheck size={22} /><span>FIXED CONTRACT<br /><strong>Every attempt. Every time.</strong></span></div></div>

        {error && <div className="error-banner" role="alert"><Info size={17} /><span>{error}</span><button aria-label="Dismiss error" onClick={() => setError(null)}><X size={16} /></button></div>}

        <div className="workspace-grid">
          <div className="work-column">
            <section className="card task-card" aria-label="Task contract">
              <div className="section-label"><span className="step-number">01</span><h2>The task</h2><span className="locked-tag"><LockKeyhole size={11} />CONTRACT LOCKED</span></div>
              <label className="select-wrap task-select"><span className="sr-only">Choose a task</span><select value={taskId} onChange={event => chooseTask(event.target.value)} disabled={active || !contracts.length}>{contracts.length ? contracts.map(item => <option key={item.id} value={item.id}>{item.title}</option>) : <option value="append_three">Loading task contract…</option>}</select><ChevronDown size={16} /></label>
              <p className="task-instruction">{contract?.instruction ?? 'Loading the fixed task, reference and acceptance rules.'}</p>
              <div className="contract-tags"><span><Check size={12} />Preserve element order</span><span><Check size={12} />Handle empty lists</span><span><Check size={12} />Keep nested structure</span></div>
              <div className="reference-row"><span>REFERENCE</span><Refal text={contract?.reference ?? '…'} /></div>
              <div className="contract-footer"><span><LockKeyhole size={11} />Reference, tests and acceptance policy stay fixed</span><span title={contract?.fingerprint}>SHA256 {contract?.fingerprint.slice(0, 10) ?? '…'}</span></div>
            </section>

            <section className="card control-card" aria-label="Run controls">
              <div className="section-label"><span className="step-number">02</span><h2>Choose a proposal source</h2><span className="muted tiny">MAX. 2 ATTEMPTS</span></div>
              <div className="source-tabs" role="group" aria-label="Proposal source">
                <button className={provider === 'fixture' ? 'source-tab active' : 'source-tab'} onClick={() => setProvider('fixture')} disabled={active}><FlaskConical size={15} />Demo fixtures<span>FREE</span></button>
                <button className={provider === 'meta' ? 'source-tab active' : 'source-tab'} onClick={() => setProvider('meta')} disabled={active}><Zap size={15} />Meta Wonder</button>
                <button className={provider === 'manual' ? 'source-tab active' : 'source-tab'} onClick={() => setProvider('manual')} disabled={active}><Code2 size={15} />Your candidate</button>
              </div>
              {provider === 'fixture' && <div className="source-details"><div><span className="field-label">SCENARIO</span><label className="select-wrap"><span className="sr-only">Fixture scenario</span><select value={scenario} onChange={event => setScenario(event.target.value as Scenario)} disabled={active}>{Object.entries(scenarioLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select><ChevronDown size={14} /></label></div><p>Scripted proposals. Real verification.<br />No model calls or API credits used.</p></div>}
              {provider === 'meta' && <div className="live-details"><div className="model-mark">∞</div><div><strong>muse-wonder-1.0</strong><p>{!health?.meta_configured ? 'Set MODEL_API_KEY on the server to connect Meta.' : !health.live_call_limit ? 'Live calls are disabled. Enable a small call budget on the server when ready.' : `${health.live_calls_remaining} of ${health.live_call_limit} calls remain · ${tokenCount(health.completion_token_limit)} completion tokens per call`}</p></div><span className={`connection-tag ${liveAvailable ? 'ready' : ''}`}>{liveAvailable ? 'Ready' : 'Disabled'}</span></div>}
              {provider === 'manual' && <label className="candidate-input"><span className="field-label">CANDIDATE RHS EXPRESSION</span><textarea value={candidate} onChange={event => setCandidate(event.target.value)} spellCheck={false} maxLength={1024} disabled={active} aria-label="Candidate expression" /><small>Use {contract?.variables.map(variable => `e.${variable}`).join(', ')}, brackets and Append. The same acceptance rules apply.</small></label>}
              <div className="run-bar"><span><ShieldCheck size={14} />Compiler + tests + symbolic proof</span>{active ? <button className="stop-button" onClick={cancel} disabled={pending || run?.cancel_requested}><Square size={13} />{run?.cancel_requested ? 'Stopping…' : 'Stop run'}</button> : <button className="primary-button" onClick={start} disabled={!contract || (provider === 'meta' && !liveAvailable)}><Play size={14} fill="currentColor" />{provider === 'meta' ? 'Start Meta run' : 'Run verification'}<span aria-hidden="true">↗</span></button>}</div>
            </section>

            <section className="code-card" aria-label="Proposed change">
              <div className="editor-toolbar"><div><GitBranch size={16} /><h2>Proposed change</h2></div><div><span className="language-tag">REFAL</span><button aria-label="Copy candidate" onClick={copyCandidate} disabled={!attempt}>{copied ? <Check size={14} /> : <Copy size={14} />}</button></div></div>
              <div className="attempt-tabs">{run?.attempts.length ? run.attempts.map((item, index) => <button key={item.number} onClick={() => setSelectedAttempt(index)} className={selectedAttempt === index ? 'attempt-tab active' : 'attempt-tab'}>Attempt {item.number}<span className={`attempt-dot ${item.verification.decision}`} /></button>) : <span className="editor-placeholder-tab"><Code2 size={13} />candidate.ref</span>}<span className="attempt-origin">{run?.provider === 'meta' ? 'META WONDER' : run?.provider === 'manual' ? 'MANUAL' : 'FIXTURE'}</span></div>
              <div className="diff-editor"><div className="code-line"><span className="line-number">1</span><Refal text="Candidate {" /></div><div className="code-line"><span className="line-number">2</span><Refal text={`  ${contract?.pattern ?? '(e.A) (e.B) (e.C)'} =`} /></div><div className="code-line removed"><span className="line-number">−</span><Refal text={`    ${contract?.reference ?? ''};`} /></div><div className={`code-line added ${!attempt ? 'awaiting' : ''}`}><span className="line-number">{attempt ? '+' : '·'}</span>{attempt ? <Refal text={`    ${attempt.candidate};`} /> : <code className="awaiting-text">    {active ? 'Waiting for a checked candidate…' : 'Run a scenario to inspect the proposed change.'}</code>}</div><div className="code-line"><span className="line-number">4</span><Refal text="}" /></div></div>
              <div className="editor-footer"><span><span className="status-light online" />{attempt?.provenance ?? 'Only the candidate expression can change'}</span>{attempt && <span>{verification?.elapsed_ms} ms verification</span>}</div>
            </section>
            {attempt && <div className="proposal-note"><span><Braces size={15} /></span><p>{attempt.explanation}</p></div>}
          </div>

          <aside className="evidence-column">
            <section className={`card decision-card ${verification?.decision ?? ''}`} aria-label="Decision">
              <div className="section-label"><span className="step-number">03</span><h2>The decision</h2><Activity size={16} className="muted" /></div>
              <div className={`decision-symbol ${active ? 'working' : ''}`}>{active ? <LoaderCircle size={29} className="spin" /> : verification?.decision === 'accepted' ? <ShieldCheck size={34} /> : verification?.decision === 'rejected' ? <XCircle size={34} /> : verification?.decision === 'unresolved' ? <CircleDashed size={34} /> : <ShieldCheck size={34} />}</div>
              <h3>{active ? 'Checking the evidence' : verification?.decision === 'accepted' ? 'Change accepted' : verification?.decision === 'rejected' ? 'Change rejected' : verification?.decision === 'unresolved' ? 'Proof unresolved' : run?.status === 'failed' ? 'Run stopped' : 'Evidence comes first'}</h3>
              <p>{active ? run?.phase ?? 'Preparing the run…' : verification?.reason ?? run?.error ?? 'A proposal earns acceptance only when every required check passes.'}</p>
              {verification ? <Badge decision={verification.decision} /> : <span className="waiting-label">{active ? 'RUN IN PROGRESS' : 'READY WHEN YOU ARE'}</span>}
              <div className="gate-list">
                <GateRow title="Expression boundary" subtitle="AST-validated, fixed helpers" passed={verification?.boundary.passed} />
                <GateRow title="Strict compiler checks" subtitle="Syntax and semantic diagnostics" passed={verification?.compiler?.passed} />
                <GateRow title="Requested transformation" subtitle="Structural contract is satisfied" passed={verification?.transformation?.passed} />
                <GateRow title="Execution tests" subtitle={verification?.tests ? `${verification.tests.passed.toLocaleString()} / ${verification.tests.total.toLocaleString()} independent cases` : 'Independent Rust test oracle'} passed={verification?.tests ? verification.tests.passed === verification.tests.total : undefined} />
                <GateRow title="Symbolic equivalence" subtitle={verification?.proof ? `${verification.proof.verdict} · ${verification.proof.steps} driving steps` : 'All required branches must close'} passed={verification?.proof ? verification.proof.verdict === 'proved' && verification.proof.complete ? true : verification.proof.verdict === 'refuted' ? false : 'open' : undefined} />
              </div>
              {run && <a className="export-button" href={`/api/runs/${run.id}/export`} download><Download size={14} />Export evidence<ArrowUpRight size={14} /></a>}
            </section>

            <section className="metrics-card" aria-label="Resource use"><div><Clock3 size={14} /><span>Verification</span><strong>{verification ? `${verification.elapsed_ms} ms` : '—'}</strong></div><div><Zap size={14} /><span>API tokens</span><strong>{attempt?.usage.total_tokens != null ? tokenCount(attempt.usage.total_tokens) : attempt && run?.provider !== 'meta' ? '0M' : '—'}</strong></div><div><span className="cost-symbol">$</span><span>Estimated API cost</span><strong>{attempt?.usage.estimated_cost_usd != null ? `$${attempt.usage.estimated_cost_usd.toFixed(4)}` : attempt && run?.provider !== 'meta' ? '$0.00' : '—'}</strong></div><div><Layers3 size={14} /><span>Proof budget</span><strong>{verification?.proof ? verification.proof.budget.toLocaleString() : '5,000'}<small>steps</small></strong></div></section>
            <div className="scope-note"><Info size={14} /><p>Accepted within the declared Refal task and engine scope. Tests, symbolic results, and performance measurements remain distinct.</p></div>
          </aside>
        </div>

        <section className="card evidence-details" aria-label="Verification evidence">
          <div className="evidence-heading"><div className="section-label"><Terminal size={17} /><h2>Under the hood</h2><span className="muted tiny">THE ACTUAL EVIDENCE</span></div>{attempt && <Badge decision={verification!.decision} small />}</div>
          <div className="evidence-tabs" role="tablist" aria-label="Evidence type">{([{ id: 'tests', label: 'Execution tests', icon: FlaskConical }, { id: 'proof', label: 'Symbolic proof', icon: GitBranch }, { id: 'feedback', label: 'Revision feedback', icon: RefreshCcw }, { id: 'source', label: 'Verified source', icon: Code2 }] as const).map(tab => <button key={tab.id} id={`tab-${tab.id}`} role="tab" aria-selected={evidenceTab === tab.id} aria-controls={`panel-${tab.id}`} className={evidenceTab === tab.id ? 'active' : ''} onClick={() => setEvidenceTab(tab.id)}><tab.icon size={14} />{tab.label}{tab.id === 'tests' && verification?.tests && <span>{verification.tests.total}</span>}</button>)}</div>
          <div className="evidence-panel" id={`panel-${evidenceTab}`} role="tabpanel" aria-labelledby={`tab-${evidenceTab}`}>
            {!attempt ? <div className="empty-evidence"><div><Braces size={23} /></div><h3>A clear trail, not a black box.</h3><p>Run a scenario to see test cases, proof branches and the feedback returned for revision.</p><span><span>Propose</span><ArrowRight size={13} /><span>Verify</span><ArrowRight size={13} /><span>Revise</span></span></div> : <EvidenceContent attempt={attempt} tab={evidenceTab} />}
          </div>
        </section>

        {run && <section className="timeline" aria-label="Run timeline"><div><Activity size={15} /><h2>Run timeline</h2><code>{run.id.slice(0, 8)}</code><span className="timeline-status">{run.status}</span></div>{run.events.map((event, index) => <p key={`${index}-${event.at_ms}`}><span className="event-dot" /><time>+{((event.at_ms - run.created_at) / 1000).toFixed(2)}s</time>{event.message}</p>)}</section>}
        <footer className="page-footer"><span>IACA <span className="footer-dot">/</span> Probabilistic generation. Deterministic checks.</span><a href="https://github.com/Abhinav-Rust/REFAL-SUPERSYSTEM" target="_blank" rel="noreferrer">Built on Refal Supersystem<ArrowUpRight size={12} /></a></footer>
      </main>
    </div>

    {drawer && <div className="drawer-backdrop" onClick={() => setDrawer(null)}><section className="drawer" role="dialog" aria-modal="true" aria-labelledby="drawer-title" onClick={event => event.stopPropagation()}><div className="drawer-heading"><h2 id="drawer-title">{drawer === 'history' ? 'Run history' : 'A loop with a fixed contract'}</h2><button aria-label="Close panel" onClick={() => setDrawer(null)}><X size={20} /></button></div>{drawer === 'history' ? <><p className="muted">Inspect a previous run without making another model call.</p>{history.length ? history.map(item => <button className="history-item" key={item.id} onClick={() => openRun(item)} disabled={active && item.id !== run?.id}><div><strong>{contracts.find(task => task.id === item.task)?.title}</strong><Badge decision={item.decision} small /></div><p>{item.provider === 'meta' ? 'Live Meta Wonder' : item.provider === 'manual' ? 'Manual candidate' : 'Deterministic fixture'} · {item.attempts.length} attempt{item.attempts.length === 1 ? '' : 's'}</p><time>{new Date(item.created_at).toLocaleString()}</time></button>) : <div className="history-empty"><History size={30} /><p>Your completed runs will appear here.</p></div>}</> : <About contract={contract} health={health} />}</section></div>}
  </div>
}

function GateRow({ title, subtitle, passed }: { title: string; subtitle: string; passed: boolean | 'open' | undefined }) {
  return <div className={`gate-row ${passed === true ? 'pass' : passed === false ? 'fail' : passed === 'open' ? 'open' : ''}`}><span className="gate-icon">{passed === true ? <Check size={13} /> : passed === false ? <X size={13} /> : passed === 'open' ? <CircleDashed size={13} /> : <span />}</span><div><strong>{title}</strong><small>{subtitle}</small></div>{passed === undefined && <span className="gate-pending">—</span>}</div>
}

function EvidenceContent({ attempt, tab }: { attempt: Attempt; tab: string }) {
  const { verification: evidence } = attempt
  if (tab === 'tests') {
    if (!evidence.tests) return <div className="evidence-message"><XCircle size={21} /><h3>Execution was not reached</h3><p>{evidence.boundary.details.join(' ')}</p>{evidence.compiler?.details.map((detail, index) => <p key={index}>{detail}</p>)}</div>
    const tests = evidence.tests
    const reduction = tests.reference_steps ? ((1 - tests.candidate_steps / tests.reference_steps) * 100).toFixed(1) : '0'
    return <><div className="test-summary"><div><span className={`summary-dot ${tests.passed === tests.total ? 'good' : 'bad'}`} /><strong>{tests.passed.toLocaleString()} of {tests.total.toLocaleString()} passed</strong><span>Representative cases below</span></div><span className="oracle-label">INDEPENDENT RUST ORACLE</span></div><div className="table-scroll"><table><thead><tr><th>Result</th><th>Input lists</th><th>Expected</th><th>Candidate returned</th></tr></thead><tbody>{tests.cases.map(test => <tr key={test.name} className={test.passed ? '' : 'failed-row'}><td><span className={test.passed ? 'test-pass' : 'test-fail'}>{test.passed ? <Check size={13} /> : <X size={13} />}{test.passed ? 'Pass' : 'Fail'}</span></td><td><code>{test.input}</code></td><td><code>{test.expected}</code></td><td><code>{test.actual}</code></td></tr>)}</tbody></table></div><div className="test-footnote"><p>{tests.oracle}</p><span><ArrowDown size={13} />{reduction}% fewer interpreter reduction steps on this suite ({tests.reference_steps.toLocaleString()} → {tests.candidate_steps.toLocaleString()}). This is not a wall-clock benchmark.</span></div></>
  }
  if (tab === 'proof') {
    if (!evidence.proof) return <div className="evidence-message"><CircleDashed size={23} /><h3>No symbolic result</h3><p>The candidate must pass the expression boundary and compiler checks first.</p></div>
    const proof = evidence.proof
    return <div className="proof-layout"><div className="proof-tree"><div className="proof-root"><GitBranch size={18} /><span>Reference ≡ Candidate<small>For all inputs in the fixed domain</small></span></div><div className="proof-branches">{proof.leaves.map((leaf, index) => <div key={index} className={`proof-leaf ${leaf.kind}`}><span className="branch-junction" />{leaf.kind === 'stuck' ? <CircleDashed size={15} /> : leaf.kind === 'refuted' ? <XCircle size={15} /> : <CheckCircle2 size={15} />}<div><strong>{leaf.kind === 'folded' ? 'Induction hypothesis' : leaf.kind === 'reflexive' ? 'Reflexivity' : leaf.kind === 'generalized' ? 'Generalization' : leaf.kind === 'refuted' ? 'Conflicting results' : 'Budget exhausted / open branch'}</strong><small>{leaf.left !== null ? `${leaf.left} ≠ ${leaf.right}` : `Depth ${leaf.depth}${leaf.ancestor !== null ? ` · folds to ancestor ${leaf.ancestor}` : ''}`}</small></div></div>)}</div><p className="proof-scope">{proof.complete ? 'The engine completed this proof attempt.' : 'This proof attempt is incomplete. Passing tests do not establish the missing proof.'}</p></div><div className="proof-output"><span>ENGINE OUTPUT · {proof.steps} / {proof.budget} STEPS</span><pre>{proof.report}</pre></div></div>
  }
  if (tab === 'feedback') return <div className="feedback-view"><div><RefreshCcw size={18} /><p>This exact evidence is returned for the next revision, when another attempt is permitted. The candidate cannot edit the task contract.</p></div><pre>{JSON.stringify(JSON.parse(attempt.feedback), null, 2)}</pre></div>
  return <div className="source-view"><p><LockKeyhole size={13} />Complete program assembled by IACA. The model supplies only the Candidate RHS.</p><pre><Refal text={evidence.source ?? 'Source was not assembled: the candidate violated the expression boundary.'} /></pre></div>
}

function About({ contract, health }: { contract?: Contract; health: Health | null }) {
  return <div className="about-content"><div className="about-emblem"><ShieldCheck size={36} /></div><p>IACA combines Meta Wonder’s code generation with the existing Rust/Refal Supersystem. Each proposed expression has to satisfy a contract controlled by the application.</p><ol><li><strong>Propose.</strong> Meta generates a candidate expression. The reference, helpers and tests stay fixed.</li><li><strong>Verify.</strong> Validate the AST, run strict compiler checks, compare concrete executions with a separate Rust oracle, and attempt symbolic equivalence.</li><li><strong>Revise.</strong> Return the actual checker evidence for another proposal, within a fixed attempt and token budget.</li></ol><h3>Three honest outcomes</h3><p><strong>Accepted:</strong> every required gate passed. <strong>Rejected:</strong> a contract violation was found. <strong>Unresolved:</strong> required evidence is incomplete or unavailable.</p><h3>The scope</h3><p>{contract?.proof_scope}</p><p>The application uses equivalence checking, not the engine’s predicate-proof mode. Its supported expression grammar invokes only the fixed, terminating list helper.</p><h3>Credits stay under your control</h3><p>Fixtures and manual candidates never call Meta. Live calls are disabled by default. This server permits {health?.live_call_limit ?? 0} live calls, each capped at {tokenCount(health?.completion_token_limit ?? 2048)} completion tokens, with no automatic HTTP retries.</p><h3>Foundation and contribution</h3><p>The pre-existing, MIT-licensed Refal Supersystem provides parsing, execution and symbolic machinery. IACA adds the constrained proposal interface, independent tests, acceptance gates, Meta feedback loop, evidence records and this workbench.</p><a className="about-link" href="https://github.com/Abhinav-Rust/IACA" target="_blank" rel="noreferrer">View the project on GitHub<ArrowUpRight size={15} /></a></div>
}
