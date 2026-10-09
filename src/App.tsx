import { useEffect, useRef, useState } from "react";
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
  effect: "now" | "explorer" | "signout"; // when the change shows up (Effect in lib.rs)
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

// Progress of one run (a single toggle, or Apply all / Revert all), shown in the Toast
type Report = {
  run: number; // new number per run, so each run gets a fresh toast (and countdown)
  name?: string; // set for a single toggle: the toast shows the tweak's name
  apply: boolean; // apply (true) or revert (false)
  total: number; // how many tweaks this run changes
  done: number; // finished so far, ok or failed
  failed: number;
  explorer: number; // succeeded, shows up after an Explorer restart
  signout: number; // succeeded, shows up after a PC restart
  finished: boolean;
};

// Adds one finished tweak (null = it failed) to the report
function counted(r: Report, updated: TweakInfo | null): Report {
  return {
    ...r,
    done: r.done + 1,
    failed: r.failed + (updated ? 0 : 1),
    explorer: r.explorer + (updated?.effect === "explorer" ? 1 : 0),
    signout: r.signout + (updated?.effect === "signout" ? 1 : 0),
  };
}

const plural = (n: number) => `${n} tweak${n === 1 ? "" : "s"}`;

// The card in the bottom-right corner. While running (Apply all / Revert all): spinner + progress bar.
// When finished: check mark + summary, then it closes itself
// (the shrinking bar is a CSS animation; onAnimationEnd closes the toast,
// and hovering pauses it, so you get time to read).
function Toast({ report, onClose }: { report: Report; onClose: () => void }) {
  const { name, apply, total, done, failed, explorer, signout, finished } = report;
  const ok = done - failed;
  const verb = apply ? "applied" : "reverted";

  // The Restart Explorer button is only offered when it's all that's left to do:
  // if some tweak needs a PC restart anyway, the button wouldn't finish the job.
  const [restart, setRestart] = useState<"idle" | "busy" | "done">("idle");
  const [restartError, setRestartError] = useState("");
  const offerExplorer = finished && failed === 0 && signout === 0 && explorer > 0;

  async function restartExplorer() {
    setRestart("busy");
    setRestartError("");
    try {
      await invoke("restart_explorer");
      setRestart("done");
    } catch (e) {
      setRestartError(String(e));
      setRestart("idle");
    }
  }

  let title: string;
  let sub: string;
  if (!finished) {
    title = `${apply ? "Applying" : "Reverting"} ${plural(total)}…`;
    sub = `${done} of ${total} done`;
  } else if (failed > 0) {
    title = name ? `Couldn't ${apply ? "apply" : "revert"} ${name}` : `${ok} of ${total} ${verb}`;
    sub = `See the message in ${failed === 1 ? "its row" : "their rows"}`;
  } else {
    title = name ? `${name} ${verb}` : `${plural(ok)} ${verb}`;
    sub =
      signout > 0 ? (apply ? "Please restart your PC to see the full changes" : "Please restart your PC to fully undo the changes")
      : explorer === 0 || restart === "done" ? "All changes are active now"
      : "Restart Explorer to see the changes now (open folders will close), or restart your PC";
  }
  if (restartError) sub = restartError;

  const icon = !finished ? "spinner" : failed > 0 || restartError ? "warn" : "ok";
  return (
    <div className={`toast ${icon}`} role="status" aria-live="polite">
      <div className="toast-icon" aria-hidden="true">
        {icon === "spinner" && (
          <svg viewBox="0 0 24 24"><circle className="spin" cx="12" cy="12" r="9" /></svg>
        )}
        {icon === "ok" && (
          <svg viewBox="0 0 24 24"><path className="draw" d="M6 12.5l4 4 8-9" /></svg>
        )}
        {icon === "warn" && (
          <svg viewBox="0 0 24 24"><path className="draw" d="M12 6.5v7M12 17.5v.01" /></svg>
        )}
      </div>
      <div className="toast-text">
        <div className="toast-title">{title}</div>
        <div className="toast-sub">{sub}</div>
        {offerExplorer && restart !== "done" && (
          <button className="toast-action" onClick={restartExplorer} disabled={restart === "busy"}>
            {restart === "busy" ? "Restarting Explorer…" : "Restart Explorer"}
          </button>
        )}
      </div>
      <button className="toast-close" onClick={onClose} aria-label="Close">×</button>
      {/* key: a new element whenever the run finishes or the restart state changes,
          so the countdown starts fresh. Paused while Explorer restarts; 10 s instead
          of 5 s while the button waits for a click. */}
      {finished ? (
        <div
          key={`timer-${restart}`}
          className={restart === "busy" ? "toast-bar timer paused" : "toast-bar timer"}
          style={{ animationDuration: offerExplorer && restart === "idle" ? "10s" : "5s" }}
          onAnimationEnd={onClose}
        />
      ) : (
        <div key="progress" className="toast-bar" style={{ width: `${(done / total) * 100}%` }} />
      )}
    </div>
  );
}

// Holds all tweaks of one page: sections, the "X of N active" count and Apply all / Revert all
function TweakList({ page }: { page: Page }) {
  const [tweaks, setTweaks] = useState<TweakInfo[]>([]);
  const [errors, setErrors] = useState<Record<string, string>>({}); // tweak id -> error
  const [busy, setBusy] = useState(false); // true while Apply all / Revert all runs
  const [report, setReport] = useState<Report | null>(null); // null = no toast
  const runs = useRef(0); // counts runs; a ref because changing it shouldn't re-render

  useEffect(() => {
    invoke<TweakInfo[]>("list_tweaks", { page }).then(setTweaks);
  }, [page]);

  // Apply or revert one tweak, then put Rust's answer into the list.
  // Returns the new state, or null if it failed.
  async function toggle(t: TweakInfo): Promise<TweakInfo | null> {
    try {
      const updated = await invoke<TweakInfo>(t.applied ? "revert_tweak" : "apply_tweak", { id: t.id });
      setTweaks((list) => list.map((x) => (x.id === updated.id ? updated : x)));
      setErrors(({ [t.id]: _, ...rest }) => rest); // clear this tweak's old error
      return updated;
    } catch (e) {
      setErrors((errs) => ({ ...errs, [t.id]: String(e) }));
      return null;
    }
  }

  // A single row's toggle: same as toggle(), plus a toast with the result
  async function toggleOne(t: TweakInfo) {
    const updated = await toggle(t);
    const start: Report = { run: ++runs.current, name: t.name, apply: !t.applied, total: 1, done: 0, failed: 0, explorer: 0, signout: 0, finished: true };
    setReport(counted(start, updated));
  }

  // One after another (not all at once), so each row updates as it finishes.
  // A failing tweak shows its error in its own row and the rest keep going.
  // The toast follows along: `r` is updated after every tweak.
  async function toggleAll(apply: boolean) {
    const todo = tweaks.filter((t) => t.applied !== apply);
    let r: Report = { run: ++runs.current, apply, total: todo.length, done: 0, failed: 0, explorer: 0, signout: 0, finished: false };
    setReport(r);
    setBusy(true);
    for (const t of todo) {
      r = counted(r, await toggle(t));
      setReport(r);
    }
    setReport({ ...r, finished: true });
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
              <TweakRow key={t.id} tweak={t} error={errors[t.id]} disabled={busy} onToggle={() => toggleOne(t)} />
            ))}
        </section>
      ))}
      {report && <Toast key={report.run} report={report} onClose={() => setReport(null)} />}
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