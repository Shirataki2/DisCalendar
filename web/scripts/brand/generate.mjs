// ブランド画像 (アプリのアイコンと OGP 画像) を mark.svg / opengraph-image.html から書き出す。
// 実行: pnpm brand
//
// アイコンのサイズと置き場所は Next.js のファイル規約 (src/app/icon.png など) と
// src/app/manifest.ts に合わせてある。favicon.ico の生成にだけ ImageMagick (magick) を使う。
// 入っていない環境ではその 1 ファイルだけ飛ばすので、必要なら `brew install imagemagick` して撮り直す。
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "@playwright/test";

const here = path.dirname(fileURLToPath(import.meta.url));
const web = path.resolve(here, "../..");
const markUrl = pathToFileURL(path.join(here, "mark.svg")).href;

const browser = await chromium.launch();
const page = await browser.newPage();
const written = [];

/** マークを 1 枚の PNG にする。scale はアートボードに対するマークの大きさ (maskable 用に縮める) */
async function renderMark({ size, background, scale = 1 }) {
  await page.setViewportSize({ width: size, height: size });
  await page.setContent(
    `<style>
       html, body { margin: 0; height: 100%; }
       body { display: flex; align-items: center; justify-content: center;
              background: ${background ?? "transparent"}; }
       img { width: ${scale * 100}%; height: ${scale * 100}%; }
     </style>
     <img src="${markUrl}">`,
  );
  await page.waitForLoadState("networkidle");
  return page.screenshot({ omitBackground: !background });
}

async function writeMark(file, options) {
  writeFileSync(path.join(web, file), await renderMark(options));
  written.push(file);
}

// favicon / PWA 用は背景なし。ライトでもダークでも地に馴染ませる
await writeMark("src/app/icon.png", { size: 192 });
await writeMark("public/icons/icon-192.png", { size: 192 });
await writeMark("public/icons/icon-512.png", { size: 512 });

// iOS はアイコンの透明部分を黒で塗るので、Apple 用だけ白地にする
await writeMark("src/app/apple-icon.png", { size: 180, background: "#fff" });

// Android の adaptive icon 用。円や角丸で切り抜かれても欠けないよう 72% に縮めて白の余白を付ける
const maskable = { background: "#fff", scale: 0.72 };
await writeMark("public/icons/icon-maskable-192.png", {
  size: 192,
  ...maskable,
});
await writeMark("public/icons/icon-maskable-512.png", {
  size: 512,
  ...maskable,
});

// OGP (1200x630)。Uni Sans Heavy と日本語のタグラインをそのまま描く
await page.setViewportSize({ width: 1200, height: 630 });
await page.goto(pathToFileURL(path.join(here, "opengraph-image.html")).href);
await page.evaluate(() => document.fonts.ready);
await page.screenshot({ path: path.join(web, "src/app/opengraph-image.png") });
written.push("src/app/opengraph-image.png");

// favicon.ico は 16 / 32 / 48 をまとめた 1 ファイル。ICO は Chromium では作れないので magick に任せる
const work = mkdtempSync(path.join(tmpdir(), "discalendar-brand-"));
const source = path.join(work, "mark-256.png");
writeFileSync(source, await renderMark({ size: 256 }));
await browser.close();

try {
  execFileSync("magick", [
    source,
    "-define",
    "icon:auto-resize=48,32,16",
    path.join(web, "src/app/favicon.ico"),
  ]);
  written.push("src/app/favicon.ico");
} catch (error) {
  console.warn(
    `favicon.ico はスキップしました (ImageMagick の magick が必要): ${error.message}`,
  );
} finally {
  rmSync(work, { recursive: true, force: true });
}

for (const file of written) {
  console.log(`wrote ${file}`);
}
