-- 不具合対応の補助 (#426): Claude API への送信の記録と、GitHub Issue を起票したスタッフ・日時。
--
-- Claude API に送ったときの記録は、いつ・誰が・どのチケットを・どのモデルに・何件の発言を送ったかと
-- トークン数・結果だけを持つ。**本文や生成結果 (要約・返信案・下書き) は保存しない** (プライバシーポリシーの記載と揃える)。
--
-- IF NOT EXISTS にしてあるのは、ロールバック (support-bot/rollback/) で `_sqlx_migrations` の記録だけを消して
-- テーブル・カラムは残すため。もう一度この版へ進めたとき、このマイグレーションが当たり直っても記録は消えない
CREATE TABLE IF NOT EXISTS support_ai_requests (
    id BIGSERIAL PRIMARY KEY,
    thread_id BIGINT NOT NULL REFERENCES support_threads (thread_id) ON DELETE CASCADE,
    -- assist = 要約・分類・返信案 / issue_draft = Issue の下書き
    kind TEXT NOT NULL CHECK (kind IN ('assist', 'issue_draft')),
    -- 操作したスタッフ
    requested_by BIGINT NOT NULL,
    model TEXT NOT NULL,
    -- 送った発言の件数 (削除済み・保存期間切れの発言と、上限で省いた発言は含まない)
    message_count INTEGER NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    -- ok = 結果を受け取った / refusal = モデルが応答を断った / error = API エラー・応答の形が違うなど
    outcome TEXT NOT NULL CHECK (outcome IN ('ok', 'refusal', 'error')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS support_ai_requests_thread ON support_ai_requests (thread_id, created_at);

-- 起票した GitHub Issue の番号 (github_issue) は Phase 1 で用意済み。誰がいつ起票したかを足す
ALTER TABLE support_threads
    ADD COLUMN IF NOT EXISTS github_issue_created_by BIGINT,
    ADD COLUMN IF NOT EXISTS github_issue_created_at TIMESTAMPTZ;
