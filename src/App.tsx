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
type TweakInfo = { id: string; name: string; description: string; applied: boolean };

function TweakRow({ id, name, description, applied }: TweakInfo) {
  const [on, setOn] = useState(applied);
  const [error, setError] = useState("");

  async function toggle() {
    try {
      if (on) await invoke("revert_tweak", { id });
      else await invoke("apply_tweak", { id });
      setOn(!on);
      setError("");
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="tweak-row">
      <div>
        <div className="tweak-name">{name}</div>
        <div className="tweak-desc">{error || description}</div>
      </div>
      <button className={on ? "toggle on" : "toggle"} onClick={toggle}>
        <span className="knob" />
      </button>
    </div>
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
        {page === "dashboard" ? <p>This page is empty for now.</p> : <TweakList page={page} />}
      </main>
    </div>
  );
}

export default App;