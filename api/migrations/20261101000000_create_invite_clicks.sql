-- 招待リンク (web の `/invite`) の導入元ごとのクリック数 (#374)。
-- どこから Bot を導入してもらえたか (LP・使い方・Bot 掲載サイト・SNS など) を比べるため、
-- 導入元 × 日 (JST) ごとの件数だけを積み上げる。
-- IP アドレス・cookie・ユーザーエージェント・Discord の ID など、個人を特定できる情報は持たない。
-- 導入元は api が決めた値 (models::invite_clicks::KNOWN_SOURCES) だけで、それ以外は 'other' にまとめる。
-- 新規テーブルなので bot は読み書きしない
CREATE TABLE invite_clicks (
    -- クリックした日 (JST の日付。api / bot の慣習どおり日付の区切りは JST)
    day DATE NOT NULL,
    -- 導入元 (例: 'lp-hero' / 'docs-start' / 'topgg' / 'direct' / 'other')
    source TEXT NOT NULL,
    clicks BIGINT NOT NULL DEFAULT 0 CHECK (clicks >= 0),
    PRIMARY KEY (day, source)
);
