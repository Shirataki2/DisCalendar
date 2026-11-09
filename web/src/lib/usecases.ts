// 用途別の紹介ページ (/usecases/<slug>) の一覧 (#381)。
// 使い方 (/docs) が「操作の手順」なのに対して、こちらは「自分たちの使い方に合うか」を伝える。
// 文章は日本語の原文で書き、英訳は lib/i18n/messages.ts に足す。機能の説明は /docs の内容と食い違わないようにする。
// app/usecases/[slug]/page.tsx がこの一覧を元に静的生成し、sitemap.ts・LP・フッタもここから辿る

import type { StaticImageData } from "next/image";
import calendarShot from "@/assets/lp/calendar.png";
import dialogShot from "@/assets/lp/dialog.png";
import settingsShot from "@/assets/lp/settings.png";
import { docPath } from "@/lib/docs";
import type { InviteSource } from "@/lib/site";

export interface UsecaseSolution {
  title: string;
  body: string;
  /** くわしい操作手順の使い方ページ (lib/docs.ts の slug) */
  docSlug: string;
}

export interface UsecasePage {
  /** URL の末尾 (/usecases/<slug>) */
  slug: string;
  /** 導入ボタンの導入元 (`/invite?from=usecase-<slug>`)。api の KNOWN_SOURCES にも足す */
  inviteSource: Extract<InviteSource, `usecase-${string}`>;
  /** LP・フッタ・見出しに出す短い名前 */
  name: string;
  /**
   * <title> と OGP のタイトル。用途を含む言葉 (「TRPG 日程調整 Discord」など) で探す人に届くよう、
   * 用途と Discord・カレンダーを入れる。画面には出さないので翻訳しない
   */
  metaTitle: string;
  /** <meta name="description">。画面には出さないので翻訳しない */
  description: string;
  /** ページ先頭の見出し */
  headline: string;
  /** 見出しの下の文章と、LP のカードの説明 */
  lead: string;
  /** よくある困りごと */
  problems: readonly string[];
  /** DisCalendar での解決 (その用途で効く機能) */
  solutions: readonly UsecaseSolution[];
  /** 解決の節に添えるスクリーンショット (web/src/assets/ の流用) */
  image: { src: StaticImageData; alt: string };
}

export const USECASE_PAGES: readonly UsecasePage[] = [
  {
    slug: "trpg",
    inviteSource: "usecase-trpg",
    name: "TRPG・マーダーミステリー",
    metaTitle: "TRPG・マダミスのセッション日程調整を Discord で",
    description:
      "TRPG やマーダーミステリーのセッション日程を Discord サーバーで決めて共有するなら DisCalendar。候補日への ○△× の回答集め、決まった日時の事前通知、Discord のイベント作成まで、ブラウザのカレンダーから行えます。",
    headline: "TRPG・マーダーミステリーのセッション日程を Discord で",
    lead: "候補日の回答集めから、決まったセッションの通知まで。卓のメンバーがいる Discord サーバーの中で完結します。",
    problems: [
      "参加者に都合のよい日を聞いても、チャンネルの流れで返事が埋もれて集計が大変",
      "決まったセッションの日時を忘れる人がいて、GM が毎回リマインドを送っている",
      "キャンペーンの次回や、サーバー内で並行している卓の予定がどこにも一覧になっていない",
    ],
    solutions: [
      {
        title: "候補日を出して ○△× で回答を集める",
        body: "日程調整で候補を最大 5 件出すと、メンバーは「参加できる / 未定 / 参加できない」で回答できます。Bot が通知チャンネルに投稿する投票メッセージのボタンからも回答でき、○ が最も多い候補には印が付きます。選んだ候補はそのまま予定にできます。",
        docSlug: "polls",
      },
      {
        title: "セッション前に Bot が自動でリマインド",
        body: "「1 日前」「1 時間前」のような事前通知を予定ごとに最大 10 件まで設定でき、開始時刻にも通知が届きます。通知でプレイヤーのロールや参加者を指定してメンションできるので、手作業の声かけがいりません。",
        docSlug: "calendar",
      },
      {
        title: "Discord のイベントとしても作成",
        body: "予定を作るときにチェックを入れると、Discord サーバーのスケジュールイベントも一緒に作られます。メンバーは Discord 上で「興味あり」を押して参加を表明できます。",
        docSlug: "calendar",
      },
      {
        title: "卓ごとに色分けして、ひとつのカレンダーで見る",
        body: "サーバーの予定はメンバー全員で共有されます。卓やシナリオごとに色を変えておけば、月表示でも見分けがつきます。説明欄にシナリオの概要を、「場所 / URL」にオンラインセッションの URL を書いておくと、通知にも載ります。",
        docSlug: "calendar",
      },
    ],
    image: {
      src: dialogShot,
      alt: "予定の編集ダイアログ。「基本」「繰り返し・通知」「添付・共有」のタブと、タイトル・日時・色・場所・説明の入力欄がある",
    },
  },
  {
    slug: "clan",
    inviteSource: "usecase-clan",
    name: "ゲームのクラン・チーム",
    metaTitle: "ゲームのクラン・チームの練習日を Discord で共有",
    description:
      "ゲームのクランやチームの練習日・大会の予定を Discord サーバーで共有するなら DisCalendar。毎週の練習を繰り返し予定で登録し、開始前にロールをメンションして通知。今日・今週の予定のまとめも Bot が自動で投稿します。",
    headline: "ゲームのクラン・チームの練習日を Discord で共有",
    lead: "毎週の練習から大会・スクリムの日程まで。決まった予定を Bot がチャンネルに知らせるので、集合の声かけを減らせます。",
    problems: [
      "毎週の練習日や大会・スクリムの日程が、メンバーにきちんと伝わっていない",
      "練習の前に、毎回だれかが「今日は 21 時から」と呼びかけている",
      "予定をだれでも変えられてしまうのは困るが、メンバーには見てほしい",
    ],
    solutions: [
      {
        title: "毎週の練習は繰り返し予定で一度だけ登録",
        body: "毎日・毎週・隔週・毎月・毎年から繰り返しを選べます。週は「火曜と木曜」のように複数の曜日を、毎月は「第 2・第 4 土曜」のような指定もできます。終了日や回数を決めておくこともできます。",
        docSlug: "calendar",
      },
      {
        title: "開始前にロールをメンションして通知",
        body: "事前通知を予定ごとに最大 10 件まで設定でき、開始時刻にも通知が届きます。通知のメンション先にチームのロールを選べば、該当するメンバーにだけ呼びかけられます。",
        docSlug: "calendar",
      },
      {
        title: "今日・今週の予定を Bot が自動で投稿",
        body: "サーバー設定で、その日の予定を毎日、7 日間の予定を毎週、通知先チャンネルにまとめて投稿できます。投稿する時刻と曜日は選べます。",
        docSlug: "initialize",
      },
      {
        title: "予定を変えられる人を管理者やロールに限定",
        body: "restricted モードを有効にすると、管理系の権限を持つメンバーか、編集を許可したロールのメンバーだけが予定を追加・編集・削除できます。閲覧はメンバー全員ができます。",
        docSlug: "edit",
      },
    ],
    image: {
      src: settingsShot,
      alt: "サーバー設定ダイアログ。通知・投稿の言語、編集制限と編集ロール、Discord への通知、今日・今週のまとめ投稿、配信用のフィード URL、重ね表示する外部カレンダーの登録、Webhook の設定、削除した予定の一覧がある",
    },
  },
];

/** どの用途のページにも添える、共通の機能 */
export const USECASE_COMMON_SOLUTIONS: readonly UsecaseSolution[] = [
  {
    title: "普段のカレンダーアプリでも見られる",
    body: "サーバーごとのフィード URL (iCal 形式) を Google カレンダーや Apple カレンダーに登録すると、DisCalendar の予定が自動で表示されます。",
    docSlug: "subscribe",
  },
  {
    title: "スマホのブラウザや Discord からも操作できる",
    body: "予定の追加・編集はスマホのブラウザからもでき、Discord の /create や /list コマンドでも同じカレンダーを扱えます。",
    docSlug: "commands",
  },
];

/** ページ先頭のスクリーンショット */
export const USECASE_HERO_IMAGE = {
  src: calendarShot,
  alt: "DisCalendar のカレンダー画面。月表示に色分けされた予定が並んでいる",
} as const;

export function findUsecasePage(slug: string): UsecasePage | undefined {
  return USECASE_PAGES.find((page) => page.slug === slug);
}

export function usecasePath(slug: string): `/usecases/${string}` {
  return `/usecases/${slug}`;
}

export function solutionDocPath(solution: UsecaseSolution): string {
  return docPath(solution.docSlug);
}
