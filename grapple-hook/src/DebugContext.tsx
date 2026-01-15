import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { useToasts } from "./toasts";

export type DebugMode = "debug" | false;

export interface DebugContextT {
  mode: DebugMode,
  setMode: (mode: DebugMode) => void
}

export const DebugContext = createContext<DebugContextT>({ mode: false, setMode: () => {} });

const KONAMI = [
  'ArrowUp', 'ArrowUp',
  'ArrowDown', 'ArrowDown',
  'ArrowLeft', 'ArrowRight', 'ArrowLeft', 'ArrowRight',
  'b', 'a', 'Enter'
];

function keyHistoryMatchesMagic(history: string[], magic: string[]): boolean {
  if (history.length >= magic.length) {
    const historySlice = history.slice(-magic.length);

    for (let i = 0; i < magic.length; i++) {
      if (magic[i] != historySlice[i])
        return false;
    }
    
    return true;
  }

  return false;
}

export default function DebugContextProvider({ children }: { children: React.ReactElement }) {
  const max_history = 24;

  const [ mode, setMode ] = useState<DebugMode>(false);
  const [ keyHistory, setKeyHistory ] = useState<string[]>([]);

  const { addInfo: addInfoToast } = useToasts();

  const contextV = {
    mode, setMode
  };

  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    setKeyHistory(last => {
      let hist = last;
      if (hist.length >= max_history) {
        hist = hist.slice(hist.length - max_history + 1);
      }
      return [ ...hist, event.key ]
    });
  }, []);

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    }
  }, [handleKeyDown]);

  useEffect(() => {
    if (keyHistoryMatchesMagic(keyHistory, KONAMI)) {
      if (!mode) {
        addInfoToast("Debug Mode Enabled");
        setMode("debug");
      } else {
        addInfoToast("Debug Mode Disabled");
        setMode(false);
      }
    }
  }, [keyHistory]);

  return <DebugContext.Provider value={contextV}>
    { children }
  </DebugContext.Provider>
}

export function useDebugCtx() {
  const { mode, setMode } = useContext(DebugContext);
  return { mode, setMode }
}