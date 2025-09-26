import React, { useState, useEffect } from 'react';
import { useAdvancedEDBStore } from '../store/advanced-edb-store';

type DisplayMode = 'variables' | 'expressions' | 'stack' | 'memory' | 'calldata' | 'storage' | 'transient';

interface StorageEntry {
  slot: string;
  value: string;
  changed?: boolean;
}

interface StackEntry {
  depth: number;
  value: string;
  changed?: boolean;
}

interface WatchExpression {
  id: number;
  expression: string;
  value: string | null;
  error?: string;
}

export function DisplayPanel() {
  const {
    getSnapshotInfo,
    getStorage,
    currentSnapshotId,
    selectedAddress,
    getTraceData
  } = useAdvancedEDBStore();

  const [displayMode, setDisplayMode] = useState<DisplayMode>('variables');
  const [watchExpressions, setWatchExpressions] = useState<WatchExpression[]>([
    { id: 1, expression: 'balance', value: '1000000000000000000' },
    { id: 2, expression: 'msg.sender', value: '0x742d35cc6bf9f5b4e6cf7c7b6db4b7b7e8b8a8b8' }
  ]);
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());
  const [storageData, setStorageData] = useState<StorageEntry[]>([]);
  const [stackData, setStackData] = useState<StackEntry[]>([]);
  const [previousSnapshotId, setPreviousSnapshotId] = useState<number | null>(null);

  const currentSnapshot = currentSnapshotId !== null ? getSnapshotInfo(currentSnapshotId) : null;
  const traceData = getTraceData();

  // React to snapshot changes and fetch relevant data
  useEffect(() => {
    if (currentSnapshotId !== null && currentSnapshotId !== previousSnapshotId) {
      fetchSnapshotData(currentSnapshotId);
      setPreviousSnapshotId(currentSnapshotId);
    }
  }, [currentSnapshotId, previousSnapshotId]);

  const fetchSnapshotData = async (snapshotId: number) => {
    // Simulate fetching storage data for common slots
    const commonSlots = ['0x0', '0x1', '0x2', '0x3', '0x4', '0x5'];
    const newStorageData: StorageEntry[] = [];

    for (const slot of commonSlots) {
      const value = getStorage(snapshotId, slot);
      if (value !== null) {
        // Check if this value changed from the previous snapshot
        const prevValue = previousSnapshotId !== null ? getStorage(previousSnapshotId, slot) : null;
        newStorageData.push({
          slot,
          value,
          changed: prevValue !== null && prevValue !== value
        });
      }
    }

    setStorageData(newStorageData);

    // Simulate stack data (in real implementation, this would come from EDB)
    const mockStack: StackEntry[] = [];
    for (let i = 0; i < 8; i++) {
      const value = `0x${Math.floor(Math.random() * 0xffffffff).toString(16).padStart(8, '0')}${Math.floor(Math.random() * 0xffffffff).toString(16).padStart(8, '0')}`;
      mockStack.push({
        depth: i,
        value,
        changed: Math.random() > 0.7 // Randomly mark some as changed
      });
    }
    setStackData(mockStack);
  };

  const toggleExpanded = (itemId: string) => {
    const newExpanded = new Set(expandedItems);
    if (newExpanded.has(itemId)) {
      newExpanded.delete(itemId);
    } else {
      newExpanded.add(itemId);
    }
    setExpandedItems(newExpanded);
  };

  const formatValue = (value: any, isExpanded: boolean = false): string => {
    if (value === null || value === undefined) return 'null';
    if (typeof value === 'string') return `"${value}"`;
    if (typeof value === 'object') {
      if (isExpanded) {
        return JSON.stringify(value, null, 2);
      }
      return `{${Object.keys(value).length} keys}`;
    }
    return String(value);
  };

  const renderVariables = () => {
    if (currentSnapshotId === null) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          No snapshot selected
        </div>
      );
    }

    // Generate variables based on current snapshot and storage data
    const variables = [
      // State variables from storage
      ...storageData.slice(0, 3).map((storage, index) => ({
        name: `storage${index}`,
        type: 'uint256',
        value: storage.value,
        scope: 'state' as const,
        changed: storage.changed || false
      })),
      // Mock local variables that change per snapshot
      {
        name: 'msg.sender',
        type: 'address',
        value: traceData?.inner?.[0]?.caller || '0x742d35cc6bf9f5b4e6cf7c7b6db4b7b7e8b8a8b8',
        scope: 'local' as const,
        changed: currentSnapshotId % 5 === 0 // Mark as changed every 5th snapshot
      },
      {
        name: 'msg.value',
        type: 'uint256',
        value: (BigInt(currentSnapshotId || 0) * BigInt(1000000000000000000)).toString(),
        scope: 'local' as const,
        changed: previousSnapshotId !== null && currentSnapshotId !== previousSnapshotId
      },
      {
        name: 'block.number',
        type: 'uint256',
        value: (18500000 + (currentSnapshotId || 0)).toString(),
        scope: 'local' as const,
        changed: false
      }
    ];

    return (
      <div className="space-y-2">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Snapshot {currentSnapshotId} Variables
        </div>
        {variables.map((variable, index) => (
          <div
            key={index}
            className={`flex items-start p-2 rounded group transition-colors ${
              variable.changed
                ? 'bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-700'
                : 'hover:bg-gray-50 dark:hover:bg-gray-700'
            }`}
          >
            <div className="flex-1 min-w-0">
              <div className="flex items-center space-x-2">
                <span className={`px-2 py-1 text-xs rounded ${
                  variable.scope === 'state'
                    ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300'
                    : 'bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300'
                }`}>
                  {variable.scope}
                </span>
                <span className="font-mono text-sm font-medium text-gray-800 dark:text-white">
                  {variable.name}
                </span>
                <span className="text-xs text-gray-500 dark:text-gray-400">
                  {variable.type}
                </span>
                {variable.changed && (
                  <span className="text-xs text-yellow-600 dark:text-yellow-400 font-semibold">
                    CHANGED
                  </span>
                )}
              </div>
              <div className="mt-1 font-mono text-sm text-gray-600 dark:text-gray-300 break-all">
                {formatValue(variable.value)}
              </div>
            </div>
          </div>
        ))}
      </div>
    );
  };

  const renderExpressions = () => {
    return (
      <div className="space-y-2">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Watch Expressions
        </div>
        {watchExpressions.map((expr) => (
          <div
            key={expr.id}
            className="flex items-start p-2 hover:bg-gray-50 dark:hover:bg-gray-700 rounded group"
          >
            <div className="flex-1 min-w-0">
              <div className="font-mono text-sm font-medium text-gray-800 dark:text-white">
                ${expr.expression}
              </div>
              <div className="mt-1 font-mono text-sm">
                {expr.error ? (
                  <span className="text-red-600 dark:text-red-400">{expr.error}</span>
                ) : (
                  <span className="text-green-600 dark:text-green-400">
                    {formatValue(expr.value)}
                  </span>
                )}
              </div>
            </div>
            <button className="opacity-0 group-hover:opacity-100 text-red-500 hover:text-red-700 text-sm">
              ×
            </button>
          </div>
        ))}

        <div className="border-t border-gray-200 dark:border-gray-600 pt-2 mt-4">
          <button className="text-sm text-blue-600 dark:text-blue-400 hover:underline">
            + Add expression
          </button>
        </div>
      </div>
    );
  };

  const renderStack = () => {
    if (currentSnapshotId === null) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          No snapshot selected
        </div>
      );
    }

    return (
      <div className="space-y-1">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Stack ({stackData.length} items)
        </div>
        {stackData.map((item, index) => (
          <div
            key={index}
            className={`flex items-center p-2 rounded font-mono text-sm ${
              item.changed
                ? 'bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-700'
                : 'hover:bg-gray-50 dark:hover:bg-gray-700'
            }`}
          >
            <div className="w-8 text-gray-500 dark:text-gray-400 text-right mr-3">
              {item.depth}
            </div>
            <div className="flex-1 text-gray-800 dark:text-white">
              {item.value}
            </div>
            {item.changed && (
              <div className="text-yellow-600 dark:text-yellow-400 text-xs">
                CHANGED
              </div>
            )}
          </div>
        ))}
        {stackData.length === 0 && (
          <div className="text-center py-4 text-gray-500 dark:text-gray-400 text-sm">
            No stack data available
          </div>
        )}
      </div>
    );
  };

  const renderMemory = () => {
    if (currentSnapshotId === null) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          No snapshot selected
        </div>
      );
    }

    // TODO: Get memory data from EDB engine via getMemory(snapshotId)
    const mockMemory = currentSnapshotId !== null
      ? `0x${currentSnapshotId.toString(16).padStart(64, '0')}000000000000000000000000000000000000000000000000000000000000000d48656c6c6f2c20576f726c6421000000000000000000000000000000000000`
      : '';

    const formatMemoryLine = (data: string, offset: number) => {
      const hex = data.slice(0, 32);
      const ascii = hex.match(/.{2}/g)?.map(h => {
        const code = parseInt(h, 16);
        return (code >= 32 && code <= 126) ? String.fromCharCode(code) : '.';
      }).join('') || '';

      return { hex, ascii };
    };

    const lines = [];
    for (let i = 0; i < mockMemory.length; i += 32) {
      const line = formatMemoryLine(mockMemory.slice(i, i + 32), i / 2);
      lines.push({ offset: i / 2, ...line });
    }

    return (
      <div className="space-y-1">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Memory ({mockMemory.length / 2} bytes)
        </div>
        {lines.length > 0 ? (
          lines.map((line, index) => (
            <div
              key={index}
              className="flex items-center p-1 hover:bg-gray-50 dark:hover:bg-gray-700 rounded font-mono text-xs"
            >
              <div className="w-16 text-gray-500 dark:text-gray-400 text-right mr-3">
                0x{line.offset.toString(16).padStart(4, '0')}
              </div>
              <div className="flex-1 text-gray-800 dark:text-white">
                {line.hex}
              </div>
              <div className="w-20 text-gray-600 dark:text-gray-300 ml-3">
                {line.ascii}
              </div>
            </div>
          ))
        ) : (
          <div className="text-center py-4 text-gray-500 dark:text-gray-400 text-sm">
            No memory data available
          </div>
        )}
      </div>
    );
  };

  const renderCalldata = () => {
    if (currentSnapshotId === null) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          No snapshot selected
        </div>
      );
    }

    // TODO: Get calldata from EDB engine - should come from trace data
    const currentSnapshot = currentSnapshotId !== null ? getSnapshotInfo(currentSnapshotId) : null;
    const traceData = getTraceData();

    // Find the relevant call for this snapshot
    const callForSnapshot = traceData?.inner?.find(call =>
      call.first_snapshot_id <= currentSnapshotId
    );

    const calldata = callForSnapshot?.input || '';

    if (!calldata) {
      return (
        <div className="text-center py-4 text-gray-500 dark:text-gray-400 text-sm">
          No calldata available for this snapshot
        </div>
      );
    }

    const formatCalldataLine = (data: string, offset: number) => {
      const hex = data.slice(0, 32);
      const ascii = hex.match(/.{2}/g)?.map(h => {
        const code = parseInt(h, 16);
        return (code >= 32 && code <= 126) ? String.fromCharCode(code) : '.';
      }).join('') || '';

      return { hex, ascii };
    };

    const lines = [];
    const cleanCalldata = calldata.startsWith('0x') ? calldata.slice(2) : calldata;
    for (let i = 0; i < cleanCalldata.length; i += 32) {
      const line = formatCalldataLine(cleanCalldata.slice(i, i + 32), i / 2);
      lines.push({ offset: i / 2, ...line });
    }

    return (
      <div className="space-y-1">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Calldata ({cleanCalldata.length / 2} bytes)
        </div>
        <div className="mb-2 p-2 bg-gray-50 dark:bg-gray-700 rounded text-xs">
          <div className="text-gray-600 dark:text-gray-400 mb-1">Function Signature:</div>
          <div className="font-mono text-blue-600 dark:text-blue-400">
            {cleanCalldata.slice(0, 8)}
          </div>
        </div>
        {lines.map((line, index) => (
          <div
            key={index}
            className="flex items-center p-1 hover:bg-gray-50 dark:hover:bg-gray-700 rounded font-mono text-xs"
          >
            <div className="w-16 text-gray-500 dark:text-gray-400 text-right mr-3">
              0x{line.offset.toString(16).padStart(4, '0')}
            </div>
            <div className="flex-1 text-gray-800 dark:text-white">
              {line.hex}
            </div>
            <div className="w-20 text-gray-600 dark:text-gray-300 ml-3">
              {line.ascii}
            </div>
          </div>
        ))}
      </div>
    );
  };

  const renderTransient = () => {
    if (currentSnapshotId === null) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          No snapshot selected
        </div>
      );
    }

    // TODO: Get transient storage data from EDB engine
    // For now, show placeholder as transient storage is a newer EIP feature
    return (
      <div className="space-y-2">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Transient Storage (EIP-1153)
        </div>
        <div className="text-center py-8 text-gray-500 dark:text-gray-400 text-sm">
          <div className="mb-2">⚡ Transient storage tracking</div>
          <div>Available in supported EVM versions</div>
        </div>
      </div>
    );
  };

  const renderStorage = () => {
    if (currentSnapshotId === null) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          No snapshot selected
        </div>
      );
    }

    return (
      <div className="space-y-2">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Storage ({storageData.length} slots)
        </div>
        {storageData.map((item, index) => (
          <div
            key={index}
            className={`p-2 rounded font-mono text-sm ${
              item.changed
                ? 'bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-700'
                : 'hover:bg-gray-50 dark:hover:bg-gray-700'
            }`}
          >
            <div className="flex items-center justify-between mb-1">
              <span className="text-gray-500 dark:text-gray-400">
                Slot {item.slot}
              </span>
              {item.changed && (
                <span className="text-yellow-600 dark:text-yellow-400 text-xs">
                  SSTORE
                </span>
              )}
            </div>
            <div className="text-gray-800 dark:text-white break-all">
              {item.value}
            </div>
          </div>
        ))}
        {storageData.length === 0 && (
          <div className="text-center py-4 text-gray-500 dark:text-gray-400 text-sm">
            No storage data available
          </div>
        )}
      </div>
    );
  };

  const renderContent = () => {
    switch (displayMode) {
      case 'variables': return renderVariables();
      case 'expressions': return renderExpressions();
      case 'stack': return renderStack();
      case 'memory': return renderMemory();
      case 'storage': return renderStorage();
      case 'calldata':
        return renderCalldata();
      case 'transient':
        return renderTransient();
      default: return null;
    }
  };

  const modes = [
    { id: 'variables', name: 'Variables', icon: '📊' },
    { id: 'expressions', name: 'Watch', icon: '👁️' },
    { id: 'stack', name: 'Stack', icon: '🏗️' },
    { id: 'memory', name: 'Memory', icon: '💾' },
    { id: 'calldata', name: 'CallData', icon: '📞' },
    { id: 'storage', name: 'Storage', icon: '🗄️' },
    { id: 'transient', name: 'Transient', icon: '⚡' }
  ];

  return (
    <div className="h-full bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
      {/* Header with Mode Selector */}
      <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600">
        <div className="flex items-center justify-between mb-2">
          <h3 className="font-semibold text-gray-800 dark:text-white">Display</h3>
        </div>

        {/* Mode Tabs */}
        <div className="flex space-x-1 overflow-x-auto">
          {modes.map((mode) => (
            <button
              key={mode.id}
              onClick={() => setDisplayMode(mode.id as DisplayMode)}
              className={`px-3 py-1 text-xs rounded whitespace-nowrap flex items-center space-x-1 transition-colors ${
                displayMode === mode.id
                  ? 'bg-blue-500 text-white'
                  : 'text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
              }`}
            >
              <span>{mode.icon}</span>
              <span>{mode.name}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Content */}
      <div className="h-full overflow-y-auto overflow-x-auto p-4">
        {renderContent()}
      </div>

      {/* Footer */}
      <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-t border-gray-200 dark:border-gray-600">
        <div className="text-xs text-gray-600 dark:text-gray-300 space-x-4">
          <span>Mode: {modes.find(m => m.id === displayMode)?.name}</span>
          <span>s/S: Cycle modes</span>
        </div>
      </div>
    </div>
  );
}