"use client";

import {
  type FormEvent,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";

/** 通常のフォームPOSTも残しつつ、処理中と失敗を同じ画面で案内する。 */
export function McpForm({
  action,
  children,
}: {
  action: string;
  children: ReactNode;
}) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const submitting = useRef(false);
  const feedback = useRef<HTMLParagraphElement>(null);
  useEffect(() => {
    if (!pending && !error) return;
    feedback.current?.focus();
    feedback.current?.scrollIntoView({ block: "nearest" });
  }, [pending, error]);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submitting.current) return;
    const body = new FormData(
      event.currentTarget,
      (event.nativeEvent as SubmitEvent).submitter,
    );
    submitting.current = true;
    setPending(true);
    setError("");
    try {
      const response = await fetch(action, {
        method: "POST",
        headers: { Accept: "application/json" },
        body,
      });
      if (!response.ok) throw new Error();
      const result = await response.json();
      window.location.assign(result.url);
    } catch {
      setError(
        "処理を完了できませんでした。接続管理で現在の状態を確認してください。同意に失敗した場合はクライアントから接続をやり直してください。",
      );
      submitting.current = false;
      setPending(false);
    }
  }
  return (
    <form
      action={action}
      method="post"
      onSubmit={submit}
      aria-busy={pending}
      className="space-y-4"
    >
      <fieldset disabled={pending} className="min-w-0 space-y-6">
        {children}
      </fieldset>
      {(pending || error) && (
        <p
          ref={feedback}
          tabIndex={-1}
          role={pending ? "status" : "alert"}
          className={
            pending
              ? "text-sm text-muted-foreground"
              : "rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm"
          }
        >
          {pending ? "処理中です。このままお待ちください。" : error}
        </p>
      )}
    </form>
  );
}
