import { useState } from "react";

/**
 * null でない最後の値を返す。
 * ダイアログやポップオーバーを閉じるとき、閉じるアニメーションの間も直前の内容を描画し続けるために使う
 */
export function useLastValue<T>(value: T | null): T | null {
  const [last, setLast] = useState<T | null>(value);
  if (value !== null && value !== last) {
    // 描画中の setState は「前回の描画の情報を保持する」ための正規のパターン
    setLast(value);
    return value;
  }
  return value ?? last;
}
