"use client";

import { Button } from "@/components/ui/button";

export default function McpError({ reset }: { reset: () => void }) {
  return (
    <main className="mx-auto w-full max-w-3xl space-y-4 px-4 py-8 sm:px-8">
      <h1 className="text-2xl font-bold">MCP の情報を読み込めませんでした</h1>
      <p role="alert" className="text-sm text-muted-foreground">
        時間をおいて再試行してください。同意画面を開けない場合はクライアントから接続をやり直してください。
      </p>
      <Button onClick={reset} variant="outline">
        再読み込み
      </Button>
    </main>
  );
}
