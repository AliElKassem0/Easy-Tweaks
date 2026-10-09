import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Page = "dashboard" | "tweaks" | "input" | "debloat";

const pages: { id: Page; label: string }[] = [
  { id: "dashboard", label: "Dashboard" },
  { id: "tweaks", label: "Tweaks" },
  { id: "input", label: "Input" },
  { id: "debloat", label: "Debloat" },
];

// Same shape as TweakInfo in lib.rs
type TweakInfo = {
  id: string;
  name: string;
  description: string;
  applied: boolean; // Easy Tweaks turned it on
  already_set: boolean; // Windows already has it, but not from us
};

function TweakRow(props: TweakInfo) {
  const [tweak, setTweak] = useState(props);
  const [error, setError] = useState("");

  async function toggle() {
    try {
      // Rust answers with the tweak's new state, so we show what it sees
      const command = tweak.applied ? "revert_tweak" : "apply_tweak";
      setTweak(await invoke<TweakInfo>(command, { id: tweak.id }));
      setError("");
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="tweak-row">
      <div>
        <div className="tweak-name">{tweak.name}</div>
        <div className="tweak-desc">{error || tweak.description}</div>
        {tweak.already_set && !error && <div className="tweak-note">Already set on this PC</div>}
      </div>
      <button className={tweak.applied ? "toggle on" : "toggle"} onClick={toggle}>
        <span className="knob" />
      </button>
    </div>
  );
}

// Same shapes as LiveStats / SystemInfo in stats.rs
type LiveStats = { cpu_usage: number; ram_used: number; ram_total: number };
type SystemInfo = { cpu_name: string; cpu_threads: number; os: string };

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

  useEffect(() => {
    invoke<SystemInfo>("system_info").then(setInfo);

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
      </div>
      {info && (
        <p className="sysinfo">
          {info.cpu_name} · {info.cpu_threads} threads · {info.os}
        </p>
      )}
    </>
  );
}

// Ask Rust which tweaks belong on this page
function TweakList({ page }: { page: Page }) {
  const [tweaks, setTweaks] = useState<TweakInfo[]>([]);

  useEffect(() => {
    invoke<TweakInfo[]>("list_tweaks", { page }).then(setTweaks);
  }, [page]);

  if (tweaks.length === 0) return <p>This page is empty for now.</p>;
  return (
    <>
      {tweaks.map((t) => (
        <TweakRow key={t.id} {...t} />
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
        {page === "dashboard" ? <Dashboard /> : <TweakList page={page} />}
      </main>
    </div>
  );
}

export default App;