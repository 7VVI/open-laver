// Tauri 事件订阅 hooks
import { useEffect, useRef } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export function useTauriEvent<T>(event: string, handler: (payload: T) => void) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let active = true;
    listen<T>(event, (e) => handlerRef.current(e.payload)).then((fn) => {
      if (active) unlisten = fn;
      else fn();
    });
    return () => {
      active = false;
      if (unlisten) unlisten();
    };
  }, [event]);
}

export const EV = {
  DELTA: "agent://delta",
  TOOL_START: "agent://tool-start",
  TOOL_RESULT: "agent://tool-result",
  ASSISTANT_MSG: "agent://assistant-message",
  TURN_STATE: "agent://turn-state",
  APPROVAL_REQUEST: "agent://approval-request",
  TODO_UPDATE: "agent://todo-update",
  TEAM_UPDATE: "agent://team-update",
  TEAM_MESSAGE: "agent://team-message",
  TASK_UPDATE: "agent://task-update",
  NOTICE: "agent://notice",
  BG_UPDATE: "agent://background-update",
  SESSION_TITLE: "session://title",
};
