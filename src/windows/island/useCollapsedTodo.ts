import { isUpdateTodo } from "../../features/todos/updateTodo";
import { useEffect, useMemo, useState } from "react";
import type { IslandSettings } from "../../features/appearance/types";
import type { TodoItem } from "../../features/todos/types";
import { useTranslation } from "../../i18n";
import { useTodoRotation } from "./useIslandAutomation";
import { getCollapsedIslandWidth } from "./layout";

function formatTimer(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/**
 * What the collapsed island shows for todos: the active (or rotating) todo
 * title, otherwise the pending count, and the width that text needs.
 */
export function useCollapsedTodo(
  todos: TodoItem[],
  activeTodoId: string | null,
  settings: IslandSettings,
  isRotationPaused: boolean,
) {
  const t = useTranslation();
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!activeTodoId) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [activeTodoId]);
  const pendingTodoIds = useMemo(
    () => todos.filter((todo) => !todo.completed && !isUpdateTodo(todo)).map((todo) => todo.id),
    [todos],
  );
  const displayedTodoId = useTodoRotation(
    pendingTodoIds,
    activeTodoId,
    settings.rotateTodos,
    settings.todoRotationSeconds,
    isRotationPaused || !!activeTodoId,
  );
  const collapsedTaskTitle = useMemo(() => {
    const activeTodo = todos.find(
      (todo) => todo.id === displayedTodoId && !todo.completed,
    );
    if (!activeTodo) return null;
    if (activeTodo.focusStartedAt) {
      const elapsed = Math.floor((now - activeTodo.focusStartedAt) / 1000);
      return `${activeTodo.title} · ${formatTimer(elapsed)}`;
    }
    return activeTodo.title;
  }, [displayedTodoId, todos, now]);
  // 计时中用固定占位串算宽度，避免秒数跳动导致岛屿宽度抖动
  const collapsedWidthSource = useMemo(() => {
    if (!collapsedTaskTitle) return collapsedTaskTitle;
    const activeTodo = todos.find((todo) => todo.id === displayedTodoId && !todo.completed);
    if (activeTodo?.focusStartedAt) {
      return `${activeTodo.title} · 00:00`;
    }
    return collapsedTaskTitle;
  }, [collapsedTaskTitle, displayedTodoId, todos]);
  const collapsedTodoCount = settings.showIdleTodoCount && !collapsedTaskTitle
    ? todos.filter((todo) => !todo.completed).length
    : null;
  const collapsedIslandWidth = useMemo(
    () =>
      getCollapsedIslandWidth(
        settings.showCollapsedText
          ? collapsedWidthSource ?? (collapsedTodoCount !== null
            ? t.island.pendingTodos(collapsedTodoCount)
            : settings.showTitle ? "fly-island" : "")
          : "",
      ),
    [collapsedWidthSource, collapsedTodoCount, settings.showCollapsedText, settings.showTitle, t],
  );

  return { collapsedTaskTitle, collapsedTodoCount, collapsedIslandWidth };
}
