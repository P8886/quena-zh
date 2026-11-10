import { useRef } from "react";
import { actions } from "../actions";

/** Minimum sizes in pixels keep both areas usable while dragging. */
export function Splitter({ onDrag, vertical, min = [120, 120], label }: {
  onDrag: (fraction: number) => void;
  vertical?: boolean;
  min?: [number, number];
  label?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const down = (e: React.PointerEvent) => {
    e.preventDefault();
    const rect = ref.current!.parentElement!.getBoundingClientRect();
    const move = (ev: PointerEvent) => {
      const size = vertical ? rect.height : rect.width;
      const f = vertical ? (ev.clientY - rect.top) / size : (ev.clientX - rect.left) / size;
      const lo = Math.min(0.5, min[0] / size);
      const hi = Math.max(0.5, 1 - min[1] / size);
      onDrag(Math.min(hi, Math.max(lo, f)));
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      actions.saveLayout();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
  return <div ref={ref} className={vertical ? "splitter-h" : "splitter-v"} role="separator" aria-orientation={vertical ? "horizontal" : "vertical"} aria-label={label} title={label} onPointerDown={down} />;
}
