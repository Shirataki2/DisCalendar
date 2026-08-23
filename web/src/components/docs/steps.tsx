import { Children, type ReactElement, type ReactNode } from "react";

interface StepProps {
  title: string;
  children: ReactNode;
  /** <Steps> が並び順から振るので MDX 側では指定しない */
  number?: number;
}

/** 番号付きの手順 1 つ分。<Steps> の中に並べる */
export function Step({ title, children, number }: StepProps) {
  return (
    <li className="relative pl-12">
      <span
        aria-hidden
        className="absolute top-0 left-0 flex size-9 items-center justify-center rounded-full bg-indigo-500 text-sm font-bold text-white"
      >
        {number}
      </span>
      <h3 className="mt-1.5 mb-2 text-lg font-semibold">{title}</h3>
      <div className="text-sm leading-7 text-neutral-200 [&_p]:my-2">
        {children}
      </div>
    </li>
  );
}

/** 「基本的な使い方」の導入ステップ (旧 Introduction.vue のタイムライン相当)。子の <Step> に番号を振る */
export function Steps({ children }: { children: ReactNode }) {
  const steps = Children.toArray(children).filter(
    (child): child is ReactElement<StepProps> =>
      typeof child === "object" && child !== null && "props" in child,
  );
  return (
    <ol className="my-6 flex list-none flex-col gap-8 pl-0">
      {steps.map((step, index) => (
        <Step key={step.props.title} {...step.props} number={index + 1} />
      ))}
    </ol>
  );
}
