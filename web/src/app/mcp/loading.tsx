import { Message } from "@/components/language-provider";
export default function McpLoading() {
  return (
    <p
      role="status"
      className="mx-auto w-full max-w-3xl px-4 py-8 text-muted-foreground sm:px-8"
    >
      <Message message="MCP の情報を読み込んでいます…" />
    </p>
  );
}
