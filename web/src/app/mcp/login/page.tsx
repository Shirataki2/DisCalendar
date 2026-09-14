"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";

export default function McpLoginPage() {
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  async function login() {
    setError("");
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
    <main className="mx-auto w-full max-w-xl flex-1 space-y-6 px-4 py-12 sm:px-8">
      <h1 className="text-2xl font-bold">MCP 接続のためのログイン</h1>
      <p className="text-sm leading-relaxed text-muted-foreground">
        {pending ? "ログイン画面へ移動中…" : "Discordでログイン"}
        した後、接続先のクライアントと許可するサーバー・操作を確認します。
      </p>
      <Button type="button" disabled={pending} onClick={login} size="lg">
        {pending ? "ログイン画面へ移動中…" : "Discordでログイン"}
      </Button>
      {pending && <p role="status">ログイン画面へ移動しています。</p>}
      {error && <p role="alert">{error}</p>}
    </main>
  );
}
