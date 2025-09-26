/**
 * @fileoverview Debug session layout
 * @description Layout wrapper for debugging sessions with navigation
 */

import React, { useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useSessionStore } from '../store/session-store';
import { useAdvancedEDBStore } from '../store/advanced-edb-store';
import { MosaicDebuggerLayout } from './MosaicDebuggerLayout';

export function DebugLayout() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();

  const {
    sessions,
    setCurrentSession,
    updateSession,
    getCurrentSession,
  } = useSessionStore();

  const {
    isConnected,
    isConnecting,
    connectionError,
    connect,
    disconnect,
    getTraceData,
    getSnapshotCount,
  } = useAdvancedEDBStore();

  const currentSession = getCurrentSession();

  // Set current session when component mounts
  useEffect(() => {
    if (sessionId) {
      setCurrentSession(sessionId);
    }
  }, [sessionId, setCurrentSession]);

  // Redirect to dashboard if session not found
  useEffect(() => {
    if (sessionId && !sessions.find(s => s.id === sessionId)) {
      navigate('/');
      return;
    }
  }, [sessionId, sessions, navigate]);

  // Auto-connect and load trace data when session is set
  useEffect(() => {
    if (currentSession && !isConnected && !isConnecting) {
      // Update session status to loading
      updateSession(currentSession.id, { status: 'loading' });

      // Connect to EDB
      connect().then(() => {
        updateSession(currentSession.id, {
          status: 'active',
          errorMessage: undefined,
        });
      }).catch((error) => {
        updateSession(currentSession.id, {
          status: 'error',
          errorMessage: error.message,
        });
      });
    }
  }, [currentSession, isConnected, isConnecting, connect, updateSession]);

  // Handle back to dashboard
  const handleBackToDashboard = () => {
    disconnect();
    setCurrentSession(null);
    navigate('/');
  };

  // Show loading if we're connecting or don't have trace data yet
  const traceData = getTraceData();
  const snapshotCount = getSnapshotCount();

  if (!currentSession) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-gray-900 flex items-center justify-center">
        <div className="text-center">
          <h1 className="text-2xl font-bold text-gray-800 dark:text-white mb-4">
            Session not found
          </h1>
          <button
            onClick={() => navigate('/')}
            className="px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white rounded-md"
          >
            Back to Dashboard
          </button>
        </div>
      </div>
    );
  }

  if (!isConnected || !traceData) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-gray-900 flex items-center justify-center">
        <div className="text-center max-w-md">
          <div className="mb-6">
            <h1 className="text-2xl font-bold text-gray-800 dark:text-white mb-2">
              {currentSession.name}
            </h1>
            <p className="text-gray-600 dark:text-gray-400 font-mono text-sm">
              {currentSession.transactionHash}
            </p>
          </div>

          {isConnecting && (
            <div className="mb-6">
              <div className="inline-flex items-center gap-2">
                <div className="w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
                <span className="text-gray-700 dark:text-gray-300">Connecting to EDB...</span>
              </div>
            </div>
          )}

          {connectionError && (
            <div className="mb-6 p-4 bg-red-100 dark:bg-red-900/20 border border-red-400 dark:border-red-800 text-red-700 dark:text-red-300 rounded">
              <h3 className="font-medium mb-2">Connection Error</h3>
              <p className="text-sm">{connectionError}</p>
            </div>
          )}

          {isConnected && !traceData && (
            <div className="mb-6">
              <div className="inline-flex items-center gap-2">
                <div className="w-4 h-4 border-2 border-green-500 border-t-transparent rounded-full animate-spin"></div>
                <span className="text-gray-700 dark:text-gray-300">Loading trace data...</span>
              </div>
            </div>
          )}

          <button
            onClick={handleBackToDashboard}
            className="px-4 py-2 bg-gray-500 hover:bg-gray-600 text-white rounded-md"
          >
            ← Back to Dashboard
          </button>
        </div>
      </div>
    );
  }

  // Show debugger interface with session context
  return (
    <div className="h-screen flex flex-col">
      {/* Session header */}
      <div className="bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 px-4 py-2 flex items-center justify-between">
        <div className="flex items-center gap-4">
          <button
            onClick={handleBackToDashboard}
            className="text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 flex items-center gap-1 text-sm"
          >
            ← Dashboard
          </button>
          <div>
            <h1 className="font-medium text-gray-800 dark:text-white">
              {currentSession.name}
            </h1>
            <p className="text-xs text-gray-600 dark:text-gray-400 font-mono">
              {currentSession.transactionHash}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
          <span>Snapshots: {snapshotCount}</span>
          <span>•</span>
          <span>Calls: {traceData.inner.length}</span>
        </div>
      </div>

      {/* Debugger interface */}
      <div className="flex-1">
        <MosaicDebuggerLayout />
      </div>
    </div>
  );
}