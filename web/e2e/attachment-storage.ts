import { createHash, createHmac } from "node:crypto";
import { createServer, type IncomingMessage } from "node:http";

// R2互換のテスト用HTTPサーバー。署名・条件付きPUT/Copy・Rangeを実際に検証する。
export const ATTACHMENT_MOCK_PORT = Number(
  process.env.E2E_ATTACHMENT_PORT ?? 8192,
);
export const ATTACHMENT_MOCK_URL = `http://127.0.0.1:${ATTACHMENT_MOCK_PORT}`;
const secret = "e2e-attachment-secret";
const objects = new Map<
  string,
  { bytes: Buffer; type: string; etag: string }
>();
const hash = (value: string | Buffer) =>
  createHash("sha256").update(value).digest("hex");
const hmac = (key: string | Buffer, value: string) =>
  createHmac("sha256", key).update(value).digest();
const encode = (value: string) =>
  encodeURIComponent(value).replace(
    /[!'()*]/g,
    (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`,
  );

function validSignature(req: IncomingMessage, url: URL) {
  const signature = url.searchParams.get("X-Amz-Signature");
  if (!signature) return !!req.headers.authorization; // SDKのAPI側リクエスト。ダミー資格情報だけを使用。
  const credential = url.searchParams.get("X-Amz-Credential")?.split("/") ?? [];
  const signed = url.searchParams.get("X-Amz-SignedHeaders") ?? "";
  const date = url.searchParams.get("X-Amz-Date") ?? "";
  const expiry = Number(url.searchParams.get("X-Amz-Expires"));
  const instant = Date.parse(
    date.replace(
      /^(\d{4})(\d\d)(\d\d)T(\d\d)(\d\d)(\d\d)Z$/,
      "$1-$2-$3T$4:$5:$6Z",
    ),
  );
  if (!Number.isFinite(instant) || Date.now() > instant + expiry * 1000)
    return false;
  const query = [...url.searchParams]
    .filter(([key]) => key !== "X-Amz-Signature")
    .map(([key, value]) => [encode(key), encode(value)])
    .sort(([a, av], [b, bv]) =>
      a === b ? av.localeCompare(bv) : a < b ? -1 : 1,
    )
    .map(([key, value]) => `${key}=${value}`)
    .join("&");
  const headers = signed
    .split(";")
    .map((name) => `${name}:${String(req.headers[name] ?? "").trim()}\n`)
    .join("");
  const canonical = [
    req.method,
    url.pathname,
    query,
    headers,
    signed,
    "UNSIGNED-PAYLOAD",
  ].join("\n");
  const scope = credential.slice(1).join("/");
  const toSign = ["AWS4-HMAC-SHA256", date, scope, hash(canonical)].join("\n");
  const key = hmac(
    hmac(
      hmac(hmac(`AWS4${secret}`, credential[1]), credential[2]),
      credential[3],
    ),
    "aws4_request",
  );
  return hmac(key, toSign).toString("hex") === signature;
}

export async function startAttachmentStorage() {
  objects.clear();
  let failDelete = false;
  let failCopy = false;
  let replaceAfterHead = false;
  let holdCopy = false;
  let copyStarted = false;
  let releaseCopy: (() => void) | undefined;
  const server = createServer(async (req, res) => {
    const url = new URL(req.url ?? "/", ATTACHMENT_MOCK_URL);
    res.setHeader("Access-Control-Allow-Origin", req.headers.origin ?? "*");
    res.setHeader(
      "Access-Control-Allow-Methods",
      "GET, HEAD, PUT, DELETE, OPTIONS",
    );
    res.setHeader(
      "Access-Control-Allow-Headers",
      "content-type,content-length,if-none-match",
    );
    if (req.method === "OPTIONS") {
      res.writeHead(204).end();
      return;
    }
    if (url.pathname === "/test-control") {
      if (url.searchParams.has("status")) {
        res.end(JSON.stringify({ copyStarted }));
        return;
      }
      releaseCopy?.();
      releaseCopy = undefined;
      holdCopy = url.searchParams.get("hold") === "true";
      copyStarted = false;
      replaceAfterHead = url.searchParams.get("replace") === "true";
      failDelete = url.searchParams.get("delete") === "fail";
      failCopy = url.searchParams.get("copy") === "fail";
      res.end("ok");
      return;
    }
    if (!validSignature(req, url)) {
      res.writeHead(403).end();
      return;
    }
    const key = decodeURIComponent(url.pathname);
    if (req.method === "PUT") {
      const copy = req.headers["x-amz-copy-source"];
      if (copy) {
        if (holdCopy) {
          copyStarted = true;
          await new Promise<void>((resolve) => {
            releaseCopy = resolve;
          });
        }
        if (failCopy) {
          res.writeHead(503).end();
          return;
        }
        const source = objects.get(`/${String(copy).replace(/^\//, "")}`);
        if (!source) {
          res.writeHead(404).end();
          return;
        }
        if (req.headers["x-amz-copy-source-if-match"] !== source.etag) {
          res.writeHead(412).end();
          return;
        }
        objects.set(key, source);
        res.setHeader("content-type", "application/xml");
        res.end(
          `<CopyObjectResult><ETag>${source.etag}</ETag><LastModified>2026-09-17T00:00:00Z</LastModified></CopyObjectResult>`,
        );
        return;
      }
      if (req.headers["if-none-match"] === "*" && objects.has(key)) {
        res.writeHead(412).end();
        return;
      }
      const chunks: Buffer[] = [];
      for await (const chunk of req) chunks.push(Buffer.from(chunk));
      const bytes = Buffer.concat(chunks);
      const etag = `"${hash(bytes)}"`;
      objects.set(key, {
        bytes,
        etag,
        type: String(req.headers["content-type"]),
      });
      res.setHeader("ETag", etag);
      res.end();
      return;
    }
    if (req.method === "DELETE") {
      if (failDelete) {
        res.writeHead(503).end();
        return;
      }
      objects.delete(key);
      res.writeHead(204).end();
      return;
    }
    const object = objects.get(key);
    if (!object) {
      res.writeHead(404).end();
      return;
    }
    if (req.headers["if-match"] && req.headers["if-match"] !== object.etag) {
      res.writeHead(412).end();
      return;
    }
    res.setHeader("ETag", object.etag);
    res.setHeader("Content-Type", object.type);
    res.setHeader("Cache-Control", "no-store");
    const disposition = url.searchParams.get("response-content-disposition");
    if (disposition) res.setHeader("Content-Disposition", disposition);
    const bytes = req.headers.range
      ? object.bytes.subarray(0, 16)
      : object.bytes;
    res.setHeader("Content-Length", bytes.length);
    if (req.method === "HEAD") {
      res.end();
      if (replaceAfterHead) objects.set(key, { ...object, etag: '"replaced"' });
    } else res.writeHead(req.headers.range ? 206 : 200).end(bytes);
  });
  await new Promise<void>((resolve) =>
    server.listen(ATTACHMENT_MOCK_PORT, "127.0.0.1", resolve),
  );
  return server;
}
