import React, { useEffect } from 'react';
import { useEDBStore } from './store/edb-store';

export function App() {
  const {
    isConnected,
    isConnecting,
    connectionError,
    serverUrl,
    traceData,
    snapshotCount,
    isLoadingTrace,
    connect,
    disconnect,
    setServerUrl,
    callRpcMethod
  } = useEDBStore();

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

  const handleTestRpcMethod = async (method: string) => {
    try {
      const result = await callRpcMethod(method);
      console.log(`${method} result:`, result);
      alert(`${method} executed successfully! Check console for details.`);
    } catch (error) {
      alert(`Failed to call ${method}: ${error}`);
    }
  };

  return (
    <div className="min-h-screen bg-gray-50 p-8">
      <div className="max-w-6xl mx-auto">
        {/* Header */}
        <div className="mb-8">
          <h1 className="text-4xl font-bold text-gray-800 mb-2">
            EDB WebUI
          </h1>
          <p className="text-lg text-gray-600">
            Ethereum Debugger Web Interface
          </p>
        </div>

        {/* Connection Panel */}
        <div className="bg-white rounded-lg shadow-md p-6 mb-6">
          <h2 className="text-xl font-semibold mb-4">Connection Status</h2>

          <div className="flex items-center gap-2 mb-4">
            <div className={`w-3 h-3 rounded-full ${
              isConnected ? 'bg-green-500' :
              isConnecting ? 'bg-yellow-500' :
              'bg-red-500'
            }`} />
            <span className="text-sm text-gray-600">
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
                className="px-4 py-2 bg-blue-500 text-white rounded-md hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isConnecting ? 'Connecting...' : 'Connect'}
              </button>
            ) : (
              <button
                onClick={handleDisconnect}
                className="px-4 py-2 bg-red-500 text-white rounded-md hover:bg-red-600"
              >
                Disconnect
              </button>
            )}
          </div>

          {connectionError && (
            <div className="p-3 bg-red-100 border border-red-400 text-red-700 rounded">
              Error: {connectionError}
            </div>
          )}
        </div>

        {/* Debug Information Panel */}
        {isConnected && (
          <div className="bg-white rounded-lg shadow-md p-6 mb-6">
            <h2 className="text-xl font-semibold mb-4">Debug Session Info</h2>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
              <div>
                <h3 className="font-medium text-gray-700 mb-2">Snapshot Count</h3>
                <div className="text-lg">
                  {snapshotCount !== null ? (
                    <span className="font-mono text-blue-600">{snapshotCount}</span>
                  ) : (
                    <span className="text-gray-500">Loading...</span>
                  )}
                </div>
              </div>

              <div>
                <h3 className="font-medium text-gray-700 mb-2">Trace Status</h3>
                <div className="text-lg">
                  {isLoadingTrace ? (
                    <span className="text-yellow-600">Loading...</span>
                  ) : traceData ? (
                    <span className="text-green-600">{traceData.inner.length} calls loaded</span>
                  ) : (
                    <span className="text-gray-500">Not loaded</span>
                  )}
                </div>
              </div>
            </div>

            {/* Available RPC Methods */}
            <div className="border-t pt-4">
              <h3 className="font-medium text-gray-700 mb-3">Available RPC Methods</h3>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2">
                {[
                  'edb_getTrace',
                  'edb_getSnapshotCount',
                  'edb_getSnapshotInfo',
                  'edb_getCode',
                  'edb_getContractABI',
                  'edb_getCallableABI',
                  'edb_getStorage',
                  'edb_getStorageDiff',
                  'edb_getNextCall',
                  'edb_getPrevCall',
                  'edb_evalOnSnapshot'
                ].map((method) => (
                  <button
                    key={method}
                    onClick={() => handleTestRpcMethod(method)}
                    className="px-3 py-2 text-sm bg-gray-100 hover:bg-gray-200 rounded border text-left font-mono"
                  >
                    {method}
                  </button>
                ))}
              </div>
            </div>
          </div>
        )}

        {/* Trace Data Panel */}
        {isConnected && traceData && (
          <div className="bg-white rounded-lg shadow-md p-6">
            <h2 className="text-xl font-semibold mb-4">Call Trace</h2>

            <div className="space-y-4">
              {traceData.inner.map((call, index) => (
                <div key={call.id} className="border border-gray-200 rounded-lg p-4">
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center gap-4">
                      <span className="text-sm font-mono bg-gray-100 px-2 py-1 rounded">
                        Call #{call.id}
                      </span>
                      <span className="text-sm text-gray-600">
                        Depth: {call.depth}
                      </span>
                      <span className="text-sm text-gray-600">
                        Snapshots: from {call.first_snapshot_id}
                      </span>
                    </div>
                  </div>

                  <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 text-sm">
                    <div>
                      <div className="mb-2">
                        <span className="font-medium text-gray-700">From:</span>
                        <div className="font-mono text-xs bg-gray-50 p-2 rounded mt-1 break-all">
                          {call.caller}
                        </div>
                      </div>
                      <div className="mb-2">
                        <span className="font-medium text-gray-700">To:</span>
                        <div className="font-mono text-xs bg-gray-50 p-2 rounded mt-1 break-all">
                          {call.target}
                        </div>
                      </div>
                    </div>

                    <div>
                      <div className="mb-2">
                        <span className="font-medium text-gray-700">Input:</span>
                        <div className="font-mono text-xs bg-gray-50 p-2 rounded mt-1 break-all">
                          {call.input.length > 100
                            ? `${call.input.substring(0, 100)}...`
                            : call.input
                          }
                        </div>
                      </div>
                      <div>
                        <span className="font-medium text-gray-700">Result:</span>
                        <div className="text-xs bg-gray-50 p-2 rounded mt-1">
                          {call.result?.Success ? (
                            <span className="text-green-600">✓ Success</span>
                          ) : (
                            <span className="text-red-600">✗ Failed</span>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>

                  {call.events && call.events.length > 0 && (
                    <div className="mt-3 pt-3 border-t border-gray-100">
                      <span className="text-sm font-medium text-gray-700">
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
        <div className="mt-6 bg-white rounded-lg shadow-md p-6">
          <h2 className="text-xl font-semibold mb-4">Status</h2>

          <div className="space-y-2 text-sm">
            <div>
              <span className="font-medium">Server:</span> {serverUrl}
            </div>
            <div>
              <span className="font-medium">Status:</span>{' '}
              <span className={
                isConnected ? 'text-green-600' :
                isConnecting ? 'text-yellow-600' :
                'text-red-600'
              }>
                {isConnected ? 'Connected' :
                 isConnecting ? 'Connecting' :
                 'Disconnected'}
              </span>
            </div>
            {connectionError && (
              <div>
                <span className="font-medium text-red-600">Error:</span> {connectionError}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}