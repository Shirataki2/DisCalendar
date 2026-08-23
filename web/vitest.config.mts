import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// ユニットテスト (Vitest) の設定。対象は src/ の純粋ロジック (*.test.ts)。
// E2E (e2e/、Playwright) は別コマンド (pnpm e2e) で動かすのでここには含めない
export default defineConfig({
  resolve: {
    alias: {
      // tsconfig.json の paths ("@/*" → "./src/*") と同じ
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
