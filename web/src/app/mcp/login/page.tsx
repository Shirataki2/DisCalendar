"use client";

import { useState } from "react";

export default function McpLoginPage() {
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  async function login() {
    setPending(true);
    try {
      const response = await fetch("/mcp/login/start", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ oauth_query: window.location.search.slice(1) }),
      });
      const result = await response.json();
      if (!response.ok || !result.url) throw new Error();
      window.location.assign(result.url);
    } catch {
      setError("ログインを開始できませんでした。接続をやり直してください。");
      setPending(false);
    }
  }
  return (
    <main className="mx-auto max-w-xl space-y-6 p-8">
      <h1 className="text-2xl font-bold">MCP 接続のためのログイン</h1>
      <p>
        Discordでログインした後、接続先のクライアントと許可するサーバー・操作を確認します。
      </p>
      <button
        type="button"
        disabled={pending}
        onClick={login}
        className="rounded bg-indigo-600 px-6 py-3 text-white disabled:opacity-50"
      >
        Discordでログイン
      </button>
      {error && <p role="alert">{error}</p>}
    </main>
  );
}
