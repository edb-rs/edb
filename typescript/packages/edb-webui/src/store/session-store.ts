/**
 * @fileoverview Session management store
 * @description Manages debugging sessions with transaction hash storage
 */

import { create } from 'zustand';

export interface DebugSession {
  id: string;
  name: string;
  transactionHash: string;
  createdAt: Date;
  lastAccessedAt: Date;
  status: 'active' | 'loading' | 'error';
  errorMessage?: string;
}

interface SessionStore {
  sessions: DebugSession[];
  currentSessionId: string | null;

  // Actions
  createSession: (transactionHash: string, name?: string) => DebugSession;
  deleteSession: (sessionId: string) => void;
  setCurrentSession: (sessionId: string | null) => void;
  updateSession: (sessionId: string, updates: Partial<DebugSession>) => void;
  getCurrentSession: () => DebugSession | null;
  clearAllSessions: () => void;
}

export const useSessionStore = create<SessionStore>((set, get) => {
  // Load sessions from localStorage on initialization
  const loadSessions = (): DebugSession[] => {
    try {
      const stored = localStorage.getItem('edb-sessions');
      if (stored) {
        const parsed = JSON.parse(stored);
        return parsed.sessions || [];
      }
    } catch (error) {
      console.warn('Failed to load sessions from localStorage:', error);
    }
    return [];
  };

  // Save sessions to localStorage
  const saveSessions = (sessions: DebugSession[]) => {
    try {
      localStorage.setItem('edb-sessions', JSON.stringify({ sessions }));
    } catch (error) {
      console.warn('Failed to save sessions to localStorage:', error);
    }
  };

  const initialSessions = loadSessions();

  return {
      sessions: initialSessions,
      currentSessionId: null,

      createSession: (transactionHash: string, name?: string) => {
        const session: DebugSession = {
          id: crypto.randomUUID(),
          name: name || `Session ${transactionHash.slice(0, 8)}...`,
          transactionHash,
          createdAt: new Date(),
          lastAccessedAt: new Date(),
          status: 'loading',
        };

        set((state) => {
          const newSessions = [...state.sessions, session];
          saveSessions(newSessions);
          return {
            sessions: newSessions,
            currentSessionId: session.id,
          };
        });

        return session;
      },

      deleteSession: (sessionId: string) => {
        set((state) => {
          const newSessions = state.sessions.filter((s) => s.id !== sessionId);
          saveSessions(newSessions);
          return {
            sessions: newSessions,
            currentSessionId: state.currentSessionId === sessionId ? null : state.currentSessionId,
          };
        });
      },

      setCurrentSession: (sessionId: string | null) => {
        set((state) => {
          // Update last accessed time if switching to a session
          if (sessionId) {
            const updatedSessions = state.sessions.map((session) =>
              session.id === sessionId
                ? { ...session, lastAccessedAt: new Date() }
                : session
            );
            saveSessions(updatedSessions);
            return {
              currentSessionId: sessionId,
              sessions: updatedSessions,
            };
          }
          return { currentSessionId: sessionId };
        });
      },

      updateSession: (sessionId: string, updates: Partial<DebugSession>) => {
        set((state) => {
          const updatedSessions = state.sessions.map((session) =>
            session.id === sessionId ? { ...session, ...updates } : session
          );
          saveSessions(updatedSessions);
          return { sessions: updatedSessions };
        });
      },

      getCurrentSession: () => {
        const { sessions, currentSessionId } = get();
        return sessions.find((s) => s.id === currentSessionId) || null;
      },

      clearAllSessions: () => {
        saveSessions([]);
        set({ sessions: [], currentSessionId: null });
      },
    };
});