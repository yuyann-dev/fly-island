import { useMemo } from "react";
import type { TodoArchive, TodoItem } from "./types";
import "./ArchiveDetail.css";

const SEGMENT_COLORS = [
  "rgba(64,125,147,0.85)",
  "rgba(116,83,214,0.8)",
  "rgba(192,80,101,0.8)",
  "rgba(200,150,60,0.8)",
  "rgba(80,160,120,0.8)",
  "rgba(150,100,180,0.8)",
];

function formatSec(sec: number): string {
  const m = Math.floor(sec / 60);
  if (m < 60) return `${m}分`;
  const h = Math.floor(m / 60);
  return `${h}时${m % 60}分`;
}

function DonutChart({ todos }: { todos: TodoItem[] }) {
  const focused = todos.filter((t) => (t.focusTotalSec ?? 0) > 0);
  const total = focused.reduce((s, t) => s + (t.focusTotalSec ?? 0), 0);
  const r = 52;
  const c = 2 * Math.PI * r;
  const segments = useMemo(() => {
    let offset = 0;
    return focused.map((t, i) => {
      const frac = (t.focusTotalSec ?? 0) / total;
      const seg = {
        color: SEGMENT_COLORS[i % SEGMENT_COLORS.length],
        dash: `${c * frac} ${c}`,
        offset: -offset * c,
        title: t.title,
        sec: t.focusTotalSec ?? 0,
        pct: Math.round(frac * 100),
      };
      offset += frac;
      return seg;
    });
  }, [focused, total, c]);

  if (total === 0) {
    return (
      <div className="archive-detail__donut-empty">
        <svg width="140" height="140" viewBox="0 0 140 140">
          <circle cx="70" cy="70" r={r} fill="none" stroke="rgba(31,27,22,0.08)" strokeWidth="14" />
        </svg>
        <span>无专注记录</span>
      </div>
    );
  }

  return (
    <div className="archive-detail__donut">
      <svg width="140" height="140" viewBox="0 0 140 140">
        <circle cx="70" cy="70" r={r} fill="none" stroke="rgba(31,27,22,0.06)" strokeWidth="14" />
        {segments.map((seg, i) => (
          <circle
            key={i}
            cx="70" cy="70" r={r} fill="none"
            stroke={seg.color} strokeWidth="14"
            strokeDasharray={seg.dash}
            strokeDashoffset={seg.offset}
            transform="rotate(-90 70 70)"
            style={{
              transition: "stroke-dasharray 700ms ease, stroke-dashoffset 700ms ease",
              animation: `donut-in 700ms ease ${i * 80}ms both`,
            }}
          >
            <title>{`${seg.title} ${formatSec(seg.sec)} (${seg.pct}%)`}</title>
          </circle>
        ))}
      </svg>
      <div className="archive-detail__donut-center">
        <strong>{formatSec(total)}</strong>
        <span>总专注</span>
      </div>
    </div>
  );
}

export function ArchiveDetail({
  archive,
  onBack,
}: {
  archive: TodoArchive;
  onBack: () => void;
}) {
  return (
    <div className="archive-detail" role="tabpanel">
      <div className="archive-detail__left">
        <DonutChart todos={archive.todos} />
      </div>
      <div className="archive-detail__right">
        <h3>{archive.date}</h3>
        <ul className="archive-detail__tasks">
          {archive.todos.map((todo, i) => (
            <li key={todo.id} className={todo.completed ? "is-done" : ""}>
              <span className="archive-detail__dot" style={{ background: SEGMENT_COLORS[i % SEGMENT_COLORS.length] }} />
              <span className="archive-detail__task-title">{todo.title}</span>
              {(todo.focusTotalSec ?? 0) > 0 && (
                <span className="archive-detail__task-time">{formatSec(todo.focusTotalSec ?? 0)}</span>
              )}
            </li>
          ))}
        </ul>
        {archive.dailyNote && (
          <p className="archive-detail__note">{archive.dailyNote}</p>
        )}
        <button className="archive-detail__back" type="button" onClick={onBack}>
          返回书架
        </button>
      </div>
    </div>
  );
}
