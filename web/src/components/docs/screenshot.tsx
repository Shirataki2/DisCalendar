import Image, { type StaticImageData } from "next/image";

interface Props {
  src: StaticImageData;
  alt: string;
  /** 画像の下に出す補足 (任意) */
  caption?: string;
  /** ダイアログなど縦長の画像は幅を抑えて出す */
  size?: "full" | "narrow";
}

/** docs 本文に貼るスクリーンショット (旧 VImage 相当)。LP と同じ枠で出す */
export function Screenshot({ src, alt, caption, size = "full" }: Props) {
  return (
    <figure className={`my-6 ${size === "narrow" ? "mx-auto max-w-md" : ""}`}>
      <div className="overflow-hidden rounded-xl border border-white/10 bg-surface shadow-xl shadow-black/40">
        <Image
          src={src}
          alt={alt}
          sizes={
            size === "narrow"
              ? "(min-width: 640px) 448px, 100vw"
              : "(min-width: 1024px) 720px, 100vw"
          }
          className="h-auto w-full"
        />
      </div>
      {caption && (
        <figcaption className="mt-2 text-center text-xs text-neutral-400">
          {caption}
        </figcaption>
      )}
    </figure>
  );
}
