import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useState } from "react";
import { Check, GripVertical, Play, Square, Trash2 } from "lucide-react";
import { useTranslation } from "../../i18n";
import { getTodoTitleLineCount } from "../../utils/text";
import { isUpdateTodo } from "./updateTodo";
import { UpdateTodoAction } from "./UpdateTodoAction";
import type { TodoItem } from "./types";
import "./TodoListItem.css";

function formatFocus(sec: number): string {
  if (sec < 60) return `${sec}秒`;
  const m = Math.floor(sec / 60);
  if (m < 60) return `${m}分`;
  const h = Math.floor(m / 60);
  return `${h}时${m % 60}分`;
}

function formatClock(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/** 专注进行中的实时正计时：累计时长 + 当前会话已走秒数，每秒刷新。 */
function LiveFocusClock({ todo }: { todo: TodoItem }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  const base = todo.focusTotalSec ?? 0;
  const current = todo.focusStartedAt
    ? Math.max(0, Math.floor((now - todo.focusStartedAt) / 1000))
    : 0;
  return (
    <span className="todo-focus-clock" title="专注计时中">
      {formatClock(base + current)}
    </span>
  );
}

export function TodoListItem({
  todo,
  isTodayMode,
  isActive,
  canReorder,
  isDragging,
  isDragOver,
  isEditing,
  titleDraft,
  onTitleDraftChange,
  onCommitEdit,
  onCancelEdit,
  onStartEdit,
  onToggle,
  onStart,
  onDelete,
  onDragStart,
  onDragMove,
  onDragEnd,
  onDragCancel,
}: {
  todo: TodoItem;
  isTodayMode: boolean;
  isActive: boolean;
  canReorder: boolean;
  isDragging: boolean;
  isDragOver: boolean;
  isEditing: boolean;
  titleDraft: string;
  onTitleDraftChange: (value: string) => void;
  onCommitEdit: () => void;
  onCancelEdit: () => void;
  onStartEdit: (todo: TodoItem) => void;
  onToggle: (id: string) => void;
  onStart: (id: string) => void;
  onDelete: (id: string) => void;
  onDragStart: (event: ReactPointerEvent<HTMLButtonElement>, todoId: string) => void;
  onDragMove: (event: ReactPointerEvent<HTMLButtonElement>) => void;
  onDragEnd: (event: ReactPointerEvent<HTMLButtonElement>) => void;
  onDragCancel: () => void;
}) {
  const t = useTranslation();
  const titleLineCount = getTodoTitleLineCount(todo.title);

  return (
    <div
      className={[
        "todo-item",
        todo.completed ? "todo-item--done" : "",
        isActive ? "todo-item--active" : "",
        canReorder ? "todo-item--reorderable" : "",
        isDragging ? "todo-item--dragging" : "",
        isDragOver ? "todo-item--drag-over" : "",
        !isTodayMode ? "todo-item--readonly" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      role="listitem"
      data-todo-id={todo.id}
      style={
        {
          "--todo-title-min-height": `${titleLineCount * 19}px`,
        } as CSSProperties
      }
    >
      <button
        className="todo-check"
        type="button"
        aria-pressed={todo.completed}
        disabled={!isTodayMode}
        title={todo.completed ? t.todo.markIncomplete : t.todo.complete}
        aria-label={t.todo.actionOnTask(
          todo.completed ? t.todo.markIncomplete : t.todo.complete,
          todo.title,
        )}
        onClick={() => onToggle(todo.id)}
      >
        {todo.completed && <Check size={14} strokeWidth={2.5} />}
      </button>
      {isTodayMode && isEditing ? (
        <input
          className="todo-title-input"
          value={titleDraft}
          aria-label={t.todo.editTitle}
          autoFocus
          onChange={(event) => onTitleDraftChange(event.currentTarget.value)}
          onBlur={onCommitEdit}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              onCommitEdit();
            }

            if (event.key === "Escape") {
              onCancelEdit();
            }
          }}
        />
      ) : isTodayMode ? (
        <button
          className="todo-title todo-title--editable"
          type="button"
          title={t.todo.editTitle}
          onClick={() => onStartEdit(todo)}
        >
          {todo.title}
        </button>
      ) : (
        <span className="todo-title">{todo.title}</span>
      )}
      {isTodayMode && (
        <>
          {isUpdateTodo(todo) ? <UpdateTodoAction /> : <span className="todo-action-cell">
            {isActive && todo.focusStartedAt ? (
              <LiveFocusClock todo={todo} />
            ) : (todo.focusTotalSec ?? 0) > 0 && (
              <span className="todo-focus-badge" title={t.todo.focusTotal(todo.focusTotalSec ?? 0)}>
                {formatFocus(todo.focusTotalSec ?? 0)}
              </span>
            )}
            <button
              className={["todo-start", isActive ? "todo-start--active" : ""]
                .filter(Boolean)
                .join(" ")}
              type="button"
              title={isActive ? t.todo.stop : t.todo.start}
              aria-label={t.todo.actionOnTask(
                isActive ? t.todo.stop : t.todo.start,
                todo.title,
              )}
              disabled={todo.completed}
              onClick={() => onStart(todo.id)}
            >
              {isActive ? (
                <Square size={12} strokeWidth={2.6} fill="currentColor" />
              ) : (
                <Play size={13} strokeWidth={2.4} />
              )}
              <span>{isActive ? t.todo.stop : t.todo.start}</span>
            </button>
          </span>}
          <button
            className="todo-delete"
            type="button"
            title={t.todo.delete}
            aria-label={t.todo.deleteTask(todo.title)}
            onClick={() => onDelete(todo.id)}
          >
            <Trash2 size={14} strokeWidth={2.2} />
          </button>
          {canReorder && (
            <button
              className="todo-drag-handle"
              type="button"
              title={t.todo.reorder}
              aria-label={t.todo.reorderTask(todo.title)}
              disabled={isEditing}
              onPointerDown={(event) => onDragStart(event, todo.id)}
              onPointerMove={onDragMove}
              onPointerUp={onDragEnd}
              onPointerCancel={onDragCancel}
            >
              <GripVertical size={15} strokeWidth={2.4} />
            </button>
          )}
        </>
      )}
    </div>
  );
}
