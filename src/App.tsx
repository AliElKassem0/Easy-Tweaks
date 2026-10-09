import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Page = "dashboard" | "input" | "visual" | "windows" | "debloat";

const pages: { id: Page; label: string }[] = [
  { id: "dashboard", label: "Dashboard" },
  { id: "input", label: "Input" },
  { id: "visual", label: "Visual" },
  { id: "windows", label: "Windows" },
  { id: "debloat", label: "Debloat" },
];

// Same shape as TweakInfo in lib.rs
type TweakInfo = {
  id: string;
  section: string;
  name: string;
  description: string;
  applied: boolean; // Easy Tweaks turned it on
  already_set: boolean; // Windows already has it, but not from us
};

// Only displays a tweak. The state lives in TweakList, which tells the row what to show.
function TweakRow({ tweak, error, disabled, onToggle }: {
  tweak: TweakInfo;
  error?: string;
  disabled: boolean;
  onToggle: () => void;
}) {
  return (
    <div className="tweak-row">
      <div>
        <div className="tweak-name">{tweak.name}</div>
        <div className="tweak-desc">{error || tweak.description}</div>
        {tweak.already_set && !error && <div className="tweak-note">Already set on this PC</div>}
      </div>
      <button
        className={tweak.applied ? "toggle on" : "toggle"}
        onClick={onToggle}
        disabled={disabled}
        role="switch"
        aria-checked={tweak.applied}
        aria-label={tweak.name}
      >
        <span className="knob" />
      </button>
    </div>
  );
}

// Same shapes as LiveStats / SystemInfo in stats.rs
type LiveStats = { cpu_usage: number; ram_used: number; ram_total: number };
type SystemInfo = { cpu_name: string; cpu_threads: number; os: string };
type TweakSummary = { applied: number; total: number }; // tweak_summary in lib.rs

const GB = 1024 ** 3; // what Task Manager calls a GB

// A bar that fills up to `percent`. role="meter" lets screen readers announce the value.
function Meter({ label, percent }: { label: string; percent: number }) {
  return (
    <div className="meter" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(percent)}>
      <div className="meter-fill" style={{ width: `${percent}%` }} />
    </div>
  );
}

function Dashboard() {
  const [live, setLive] = useState<LiveStats | null>(null);
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [summary, setSummary] = useState<TweakSummary | null>(null);

  useEffect(() => {
    invoke<SystemInfo>("system_info").then(setInfo);
    invoke<TweakSummary>("tweak_summary").then(setSummary);

    // Ask Rust for fresh numbers every second
    const update = () => invoke<LiveStats>("live_stats").then(setLive);
    update();
    const timer = setInterval(update, 1000);
    // Runs when you leave the Dashboard: stop polling, or it would keep going in the background
    return () => clearInterval(timer);
  }, []);

  if (!live) return <p>Loading…</p>;

  const ramPercent = (live.ram_used / live.ram_total) * 100;
  return (
    <>
      <div className="stats">
        <div className="stat-tile">
          <div className="stat-label">CPU usage</div>
          <div className="stat-value">{Math.round(live.cpu_usage)}%</div>
          <div className="stat-sub">all cores, last second</div>
          <Meter label="CPU usage" percent={live.cpu_usage} />
        </div>
        <div className="stat-tile">
          <div className="stat-label">Memory in use</div>
          <div className="stat-value">{(live.ram_used / GB).toFixed(1)} GB</div>
          <div className="stat-sub">
            of {(live.ram_total / GB).toFixed(0)} GB · {Math.round(ramPercent)}%
          </div>
          <Meter label="Memory in use" percent={ramPercent} />
        </div>
        {summary && (
          <div className="stat-tile">
            <div className="stat-label">Active tweaks</div>
            <div className="stat-value">{summary.applied}</div>
            <div className="stat-sub">of {summary.total}, applied by Easy Tweaks</div>
            <Meter label="Active tweaks" percent={(summary.applied / summary.total) * 100} />
          </div>
        )}
      </div>
      {info && (
        <p className="sysinfo">
          {info.cpu_name} · {info.cpu_threads} threads · {info.os}
        </p>
      )}
    </>
  );
}

// Holds all tweaks of one page: sections, the "X of N active" count and Apply all / Revert all
function TweakList({ page }: { page: Page }) {
  const [tweaks, setTweaks] = useState<TweakInfo[]>([]);
  const [errors, setErrors] = useState<Record<string, string>>({}); // tweak id -> error
  const [busy, setBusy] = useState(false); // true while Apply all / Revert all runs

  useEffect(() => {
    invoke<TweakInfo[]>("list_tweaks", { page }).then(setTweaks);
  }, [page]);

  // Apply or revert one tweak, then put Rust's answer into the list
  async function toggle(t: TweakInfo) {
    try {
      const updated = await invoke<TweakInfo>(t.applied ? "revert_tweak" : "apply_tweak", { id: t.id });
      setTweaks((list) => list.map((x) => (x.id === updated.id ? updated : x)));
      setErrors(({ [t.id]: _, ...rest }) => rest); // clear this tweak's old error
    } catch (e) {
      setErrors((errs) => ({ ...errs, [t.id]: String(e) }));
    }
  }

  // One after another (not all at once), so each row updates as it finishes.
  // A failing tweak shows its error in its own row and the rest keep going.
  async function toggleAll(apply: boolean) {
    setBusy(true);
    for (const t of tweaks.filter((t) => t.applied !== apply)) {
      await toggle(t);
    }
    setBusy(false);
  }

  if (tweaks.length === 0) return <p>Loading…</p>;

  // Group rows by section, keeping the order from lib.rs
  const sections = [...new Set(tweaks.map((t) => t.section))];
  const active = tweaks.filter((t) => t.applied).length;

  return (
    <>
      <div className="page-bar">
        <span className="page-count">
          {active} of {tweaks.length} active
        </span>
        <button className="btn" onClick={() => toggleAll(true)} disabled={busy || active === tweaks.length}>
          Apply all
        </button>
        <button className="btn" onClick={() => toggleAll(false)} disabled={busy || active === 0}>
          Revert all
        </button>
      </div>
      {sections.map((section) => (
        <section key={section}>
          <h2 className="section-title">{section}</h2>
          {tweaks
            .filter((t) => t.section === section)
            .map((t) => (
              <TweakRow key={t.id} tweak={t} error={errors[t.id]} disabled={busy} onToggle={() => toggle(t)} />
            ))}
        </section>
      ))}
    </>
  );
}

function App() {
  const [page, setPage] = useState<Page>("dashboard");

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="logo">Easy Tweaks</div>
        {pages.map((p) => (
          <button
            key={p.id}
            className={page === p.id ? "nav-item active" : "nav-item"}
            onClick={() => setPage(p.id)}
          >
            {p.label}
          </button>
        ))}
      </aside>

      <main className="content">
        <h1>{pages.find((p) => p.id === page)?.label}</h1>
        {page === "dashboard" ? <Dashboard /> : <TweakList key={page} page={page} />}
      </main>
    </div>
  );
}

export default App;