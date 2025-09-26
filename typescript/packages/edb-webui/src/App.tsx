import React, { useEffect } from 'react';
import { useAdvancedEDBStore } from './store/advanced-edb-store';
import { useTheme } from './hooks/useTheme';
import { MosaicDebuggerLayout } from './components/MosaicDebuggerLayout';

export function App() {
  const { theme, toggleTheme } = useTheme();
  const {
    isConnected,
    isConnecting,
    connectionError,
    serverUrl,
    connect,
    disconnect,
    setServerUrl,
    getSnapshotCount,
    getTraceData,
    isAnyLoading,
    getLoadingStatus
  } = useAdvancedEDBStore();

  // Auto-connect on startup
  useEffect(() => {
    if (!isConnected && !isConnecting && !connectionError) {
      connect();
    }
  }, [connect, isConnected, isConnecting, connectionError]);

  const handleConnect = async () => {
    await connect();
  };

  const handleDisconnect = () => {
    disconnect();
  };

  // Get data using new non-blocking approach
  const snapshotCount = getSnapshotCount();
  const traceData = getTraceData();
  const loadingStatus = getLoadingStatus();

  // If connected and have trace data, show debugger interface
  if (isConnected && traceData) {
    return <MosaicDebuggerLayout />;
  }

  // Otherwise show connection/setup interface
  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-600 dark:bg-gray-900 p-8 transition-colors duration-200">
      <div className="max-w-6xl mx-auto">
        {/* Header */}
        <div className="mb-8 flex items-center justify-between">
          <div>
            <h1 className="text-4xl font-bold text-gray-800 dark:text-white mb-2">
              EDB WebUI
            </h1>
            <p className="text-lg text-gray-600 dark:text-gray-300 dark:text-gray-300">
              Ethereum Debugger Web Interface
            </p>
          </div>

          {/* Theme Toggle */}
          <button
            onClick={toggleTheme}
            className="p-2 rounded-lg bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
            title={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
          >
            {theme === 'light' ? (
              <svg className="w-5 h-5 text-gray-600 dark:text-gray-300" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M10 2a1 1 0 011 1v1a1 1 0 11-2 0V3a1 1 0 011-1zm4 8a4 4 0 11-8 0 4 4 0 018 0zm-.464 4.95l.707.707a1 1 0 001.414-1.414l-.707-.707a1 1 0 00-1.414 1.414zm2.12-10.607a1 1 0 010 1.414l-.706.707a1 1 0 11-1.414-1.414l.707-.707a1 1 0 011.414 0zM17 11a1 1 0 100-2h-1a1 1 0 100 2h1zm-7 4a1 1 0 011 1v1a1 1 0 11-2 0v-1a1 1 0 011-1zM5.05 6.464A1 1 0 106.465 5.05l-.708-.707a1 1 0 00-1.414 1.414l.707.707zm1.414 8.486l-.707.707a1 1 0 01-1.414-1.414l.707-.707a1 1 0 011.414 1.414zM4 11a1 1 0 100-2H3a1 1 0 000 2h1z" clipRule="evenodd" />
              </svg>
            ) : (
              <svg className="w-5 h-5 text-yellow-400" fill="currentColor" viewBox="0 0 20 20">
                <path d="M17.293 13.293A8 8 0 016.707 2.707a8.001 8.001 0 1010.586 10.586z" />
              </svg>
            )}
          </button>
        </div>

        {/* Connection Panel */}
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6 border border-gray-100 dark:border-gray-700">
          <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-white">Connection Status</h2>

          <div className="flex items-center gap-2 mb-4">
            <div className={`w-3 h-3 rounded-full ${
              isConnected ? 'bg-green-500' :
              isConnecting ? 'bg-yellow-500' :
              'bg-red-500'
            }`} />
            <span className="text-sm text-gray-600 dark:text-gray-300 dark:text-gray-300">
              {isConnected ? 'Connected to EDB Engine' :
               isConnecting ? 'Connecting...' :
               'Disconnected'}
            </span>
          </div>

          <div className="flex gap-2 mb-4">
            {!isConnected ? (
              <button
                onClick={handleConnect}
                disabled={isConnecting}
                className="px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white rounded-md disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                {isConnecting ? 'Connecting...' : 'Connect'}
              </button>
            ) : (
              <button
                onClick={handleDisconnect}
                className="px-4 py-2 bg-red-500 hover:bg-red-600 text-white rounded-md transition-colors"
              >
                Disconnect
              </button>
            )}
          </div>

          {connectionError && (
            <div className="p-3 bg-red-100 dark:bg-red-900/20 border border-red-400 dark:border-red-800 text-red-700 dark:text-red-300 rounded">
              Error: {connectionError}
            </div>
          )}
        </div>

        {/* Debug Information Panel */}
        {isConnected && (
          <div className="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 mb-6 border border-gray-100 dark:border-gray-700">
            <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-white">Debug Session Info</h2>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
              <div>
                <h3 className="font-medium text-gray-700 dark:text-gray-300 dark:text-gray-300 mb-2">Snapshot Count</h3>
                <div className="text-lg">
                  {snapshotCount > 0 ? (
                    <span className="font-mono text-blue-600">{snapshotCount}</span>
                  ) : (
                    <span className="text-gray-500 dark:text-gray-400">
                      {loadingStatus.hasAnyLoading ? 'Loading...' : 'Not loaded'}
                    </span>
                  )}
                </div>
              </div>

              <div>
                <h3 className="font-medium text-gray-700 dark:text-gray-300 dark:text-gray-300 mb-2">Trace Status</h3>
                <div className="text-lg">
                  {loadingStatus.hasAnyLoading ? (
                    <span className="text-yellow-600 dark:text-yellow-400">Loading...</span>
                  ) : traceData ? (
                    <span className="text-green-600 dark:text-green-400">{traceData.inner.length} calls loaded</span>
                  ) : (
                    <span className="text-gray-500 dark:text-gray-400">Not loaded</span>
                  )}
                </div>
              </div>
            </div>

            {/* Cache Statistics */}
            {isConnected && (
              <div className="border-t border-gray-200 dark:border-gray-600 pt-4">
                <h3 className="font-medium text-gray-700 dark:text-gray-300 mb-3">Background Processing Status</h3>
                <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
                  <div className="text-center">
                    <div className="text-lg font-mono text-blue-600 dark:text-blue-400">
                      {loadingStatus.snapshotInfo ? '⟳' : '✓'}
                    </div>
                    <div className="text-xs text-gray-600 dark:text-gray-400">Snapshot Info</div>
                  </div>
                  <div className="text-center">
                    <div className="text-lg font-mono text-blue-600 dark:text-blue-400">
                      {loadingStatus.code ? '⟳' : '✓'}
                    </div>
                    <div className="text-xs text-gray-600 dark:text-gray-400">Code</div>
                  </div>
                  <div className="text-center">
                    <div className="text-lg font-mono text-blue-600 dark:text-blue-400">
                      {loadingStatus.storage ? '⟳' : '✓'}
                    </div>
                    <div className="text-xs text-gray-600 dark:text-gray-400">Storage</div>
                  </div>
                  <div className="text-center">
                    <div className="text-lg font-mono text-blue-600 dark:text-blue-400">
                      {loadingStatus.navigation ? '⟳' : '✓'}
                    </div>
                    <div className="text-xs text-gray-600 dark:text-gray-400">Navigation</div>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Trace Data Panel */}
        {isConnected && traceData && (
          <div className="bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 border border-gray-100 dark:border-gray-700">
            <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-white">Call Trace</h2>

            <div className="space-y-4">
              {traceData.inner.map((call, index) => (
                <div key={call.id} className="border border-gray-200 dark:border-gray-600 rounded-lg p-4 bg-gray-50 dark:bg-gray-600 dark:bg-gray-700">
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center gap-4">
                      <span className="text-sm font-mono bg-gray-100 px-2 py-1 rounded">
                        Call #{call.id}
                      </span>
                      <span className="text-sm text-gray-600 dark:text-gray-300">
                        Depth: {call.depth}
                      </span>
                      <span className="text-sm text-gray-600 dark:text-gray-300">
                        Snapshots: from {call.first_snapshot_id}
                      </span>
                    </div>
                  </div>

                  <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 text-sm">
                    <div>
                      <div className="mb-2">
                        <span className="font-medium text-gray-700 dark:text-gray-300">From:</span>
                        <div className="font-mono text-xs bg-gray-50 dark:bg-gray-600 p-2 rounded mt-1 break-all">
                          {call.caller}
                        </div>
                      </div>
                      <div className="mb-2">
                        <span className="font-medium text-gray-700 dark:text-gray-300">To:</span>
                        <div className="font-mono text-xs bg-gray-50 dark:bg-gray-600 p-2 rounded mt-1 break-all">
                          {call.target}
                        </div>
                      </div>
                    </div>

                    <div>
                      <div className="mb-2">
                        <span className="font-medium text-gray-700 dark:text-gray-300">Input:</span>
                        <div className="font-mono text-xs bg-gray-50 dark:bg-gray-600 p-2 rounded mt-1 break-all">
                          {call.input.length > 100
                            ? `${call.input.substring(0, 100)}...`
                            : call.input
                          }
                        </div>
                      </div>
                      <div>
                        <span className="font-medium text-gray-700 dark:text-gray-300">Result:</span>
                        <div className="text-xs bg-gray-50 dark:bg-gray-600 p-2 rounded mt-1">
                          {call.result?.Success ? (
                            <span className="text-green-600 dark:text-green-400">✓ Success</span>
                          ) : (
                            <span className="text-red-600 dark:text-red-400">✗ Failed</span>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>

                  {call.events && call.events.length > 0 && (
                    <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-600">
                      <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                        Events: {call.events.length}
                      </span>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Status Panel */}
        <div className="mt-6 bg-white dark:bg-gray-800 rounded-lg shadow-md p-6 border border-gray-100 dark:border-gray-700">
          <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-white">Status</h2>

          <div className="space-y-2 text-sm">
            <div>
              <span className="font-medium">Server:</span> {serverUrl}
            </div>
            <div>
              <span className="font-medium">Status:</span>{' '}
              <span className={
                isConnected ? 'text-green-600 dark:text-green-400' :
                isConnecting ? 'text-yellow-600 dark:text-yellow-400' :
                'text-red-600 dark:text-red-400'
              }>
                {isConnected ? 'Connected' :
                 isConnecting ? 'Connecting' :
                 'Disconnected'}
              </span>
            </div>
            {connectionError && (
              <div>
                <span className="font-medium text-red-600 dark:text-red-400">Error:</span> {connectionError}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}