import React, { useCallback, useContext, useState } from "react";
import update from "immutability-helper";

export interface Toast {
  variant: string,
  message: React.ReactNode,
  title: string
};

export const ToastContext = React.createContext<{ toasts: Toast[], add: (variant: string, e: React.ReactNode, title?: string) => void, addError: (e: React.ReactNode, title?: string) => void, addWarning: (e: React.ReactNode, title?: string) => void, addInfo: (e: React.ReactNode, title?: string) => void, removeToast: (idx: number) => void }>({
  toasts: [],
  add: () => {},
  addError: () => {},
  addWarning: () => {},
  addInfo: () => {},
  removeToast: () => {}
});

export default function ToastProvider({ children }: { children: React.ReactElement }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const remove = (i: number) => setToasts(update(toasts, { $splice: [[i, 1]] }));
  const add = (variant: string, e: React.ReactNode, title?: string) => setToasts(update(toasts, { $push: [{ title: title || "",  message: e, variant }]  }))

  const contextValue = {
    toasts,
    add: useCallback((variant: string, e: React.ReactNode, title?: string) => add(variant, e, title), []),
    addError: useCallback((e: any, title?: string) => add("danger", e instanceof Error ? e.message : e, title), []),
    addWarning: useCallback((e: any, title?: string) => add("warning", e instanceof Error ? e.message : e, title), []),
    addInfo: useCallback((e: React.ReactNode, title?: string) => add("primary", e, title), []),
    removeToast: useCallback((i: number) => remove(i), [])
  };

  return (
    <ToastContext.Provider value={contextValue as any}>
      {children}
    </ToastContext.Provider>
  );
}

export function useToasts() {
  const { toasts, add, addError, addWarning, addInfo, removeToast } = useContext(ToastContext);
  return { toasts, add, addError, addWarning, addInfo, removeToast };
}