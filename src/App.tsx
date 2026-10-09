import { useEffect, useState } from "react";
import { getAppInfo } from "./lib/desktop";
import "./App.css";

const scope = [
  {
    name: "Providers",
    items: "Command Code GOAT · DeepSeek · OpenRouter",
    detail: "Bring your own API access and model preferences.",
  },
  {
    name: "Agents",
    items: "Pi · Grok Build",
    detail: "Preview configuration changes before applying them.",
  },
  {
    name: "Skills",
    items: "One shared collection",
    detail: "Track sources, review updates, and sync to your agents.",
  },
  {
    name: "MCP servers",
    items: "Shared server definitions",
    detail: "Manage connections with each agent's supported options.",
  },
];

export default function App() {
  const [runtime, setRuntime] = useState("Checking desktop runtime…");

  useEffect(() => {
    // React may remount effects in development. Ignore results after cleanup so
    // an old request cannot update a component that is no longer mounted.
    let active = true;
    getAppInfo().then(
      (info) => {
        if (active)
          setRuntime(
            info
              ? `${info.name} ${info.version} · Desktop runtime ready`
              : "Browser preview · Desktop runtime unavailable",
          );
      },
      () => {
        if (active)
          setRuntime("Desktop metadata unavailable. Restart the app to retry.");
      },
    );
    return () => {
      active = false;
    };
  }, []);

  return (
    <main className="workspace">
      <header className="topbar">
        <span className="brand">
          vibemate<span className="brand-dot">.</span>
        </span>
        <span className="stage">EARLY DEVELOPMENT</span>
      </header>
      <section className="intro" aria-labelledby="intro-title">
        <p className="eyebrow">YOUR AGENTS, TOGETHER</p>
        <h1 id="intro-title">
          A home for your
          <br />
          agent configuration.
        </h1>
        <p className="summary">
          Providers, models, skills, and MCP servers — managed in one desktop
          workspace.
        </p>
      </section>
      <section className="scope" aria-labelledby="scope-title">
        <div className="section-heading">
          <h2 id="scope-title">First release scope</h2>
          <span>04 areas</span>
        </div>
        <div className="scope-grid">
          {scope.map((area, index) => (
            <article className="scope-card" key={area.name}>
              <div className="card-top">
                <span className="number">0{index + 1}</span>
                <span className="planned">Planned</span>
              </div>
              <h3>{area.name}</h3>
              <p className="items">{area.items}</p>
              <p className="detail">{area.detail}</p>
            </article>
          ))}
        </div>
      </section>
      <footer>
        <p>Foundation ready. Integrations will be added next.</p>
        <p className="runtime" role="status">
          {runtime}
        </p>
      </footer>
    </main>
  );
}
