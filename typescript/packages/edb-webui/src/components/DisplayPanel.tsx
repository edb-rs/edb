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
    // Get real snapshot info from EDB engine
    const snapshotInfo = getSnapshotInfo(snapshotId);
    const prevSnapshotInfo = previousSnapshotId !== null ? getSnapshotInfo(previousSnapshotId) : null;

    if (!snapshotInfo) {
      // If snapshot info is not yet loaded, clear current data
      setStorageData([]);
      setStackData([]);
      return;
    }

    // Handle both Opcode and Hook snapshot types
    let newStorageData: StorageEntry[] = [];
    let newStackData: StackEntry[] = [];

    if (snapshotInfo.detail.Opcode) {
      const opcodeDetail = snapshotInfo.detail.Opcode;
      const prevOpcodeDetail = prevSnapshotInfo?.detail.Opcode;

      // Extract real stack data from EDB
      newStackData = opcodeDetail.stack.map((value, index) => {
        const prevValue = prevOpcodeDetail?.stack[index];
        return {
          depth: index,
          value: value,
          changed: prevValue !== undefined && prevValue !== value
        };
      });

      // Extract storage data - we still need to query individual slots
      // as storage is not included in opcode snapshots directly
      const commonSlots = ['0x0', '0x1', '0x2', '0x3', '0x4', '0x5'];
      for (const slot of commonSlots) {
        const value = getStorage(snapshotId, slot);
        if (value !== null && value !== '0x0000000000000000000000000000000000000000000000000000000000000000') {
          const prevValue = previousSnapshotId !== null ? getStorage(previousSnapshotId, slot) : null;
          newStorageData.push({
            slot,
            value,
            changed: prevValue !== null && prevValue !== value
          });
        }
      }

    } else if (snapshotInfo.detail.Hook) {
      // Hook snapshots don't have raw stack/memory, but we can still get storage
      const commonSlots = ['0x0', '0x1', '0x2', '0x3', '0x4', '0x5'];
      for (const slot of commonSlots) {
        const value = getStorage(snapshotId, slot);
        if (value !== null && value !== '0x0000000000000000000000000000000000000000000000000000000000000000') {
          const prevValue = previousSnapshotId !== null ? getStorage(previousSnapshotId, slot) : null;
          newStorageData.push({
            slot,
            value,
            changed: prevValue !== null && prevValue !== value
          });
        }
      }

      // For hook snapshots, stack data isn't available at the opcode level
      newStackData = [];
    }

    setStorageData(newStorageData);
    setStackData(newStackData);
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

    // Handle EdbSolValue structured objects
    if (typeof value === 'object' && value.type && value.value !== undefined) {
      switch (value.type) {
        case 'Address':
          return value.value;
        case 'Uint':
          return `${value.value}`;
        case 'Int':
          return `${value.value}`;
        case 'Bool':
          return value.value ? 'true' : 'false';
        case 'Bytes':
          return `0x${value.value.map((b: number) => b.toString(16).padStart(2, '0')).join('')}`;
        case 'FixedBytes':
          return `0x${value.value.map((b: number) => b.toString(16).padStart(2, '0')).join('')}`;
        case 'String':
          return `"${value.value}"`;
        case 'Array':
          if (isExpanded) {
            return `[\n${value.value.map((v: any) => '  ' + formatValue(v, false)).join(',\n')}\n]`;
          }
          return `[${value.value.length} items]`;
        case 'FixedArray':
          if (isExpanded) {
            return `[\n${value.value.map((v: any) => '  ' + formatValue(v, false)).join(',\n')}\n]`;
          }
          return `[${value.value.length} items]`;
        case 'Tuple':
          if (isExpanded) {
            return `(\n${value.value.map((v: any) => '  ' + formatValue(v, false)).join(',\n')}\n)`;
          }
          return `(${value.value.length} fields)`;
        default:
          return JSON.stringify(value.value);
      }
    }

    // Handle plain values
    if (typeof value === 'string') return `"${value}"`;
    if (typeof value === 'object') {
      if (isExpanded) {
        return JSON.stringify(value, null, 2);
      }
      return `{${Object.keys(value).length} keys}`;
    }
    return String(value);
  };

  const getValueType = (value: any): string => {
    if (value === null || value === undefined) return 'null';

    // Handle EdbSolValue structured objects
    if (typeof value === 'object' && value.type && value.value !== undefined) {
      switch (value.type) {
        case 'Address':
          return 'address';
        case 'Uint':
          return `uint${value.bits || 256}`;
        case 'Int':
          return `int${value.bits || 256}`;
        case 'Bool':
          return 'bool';
        case 'Bytes':
          return 'bytes';
        case 'FixedBytes':
          return `bytes${value.size || 32}`;
        case 'String':
          return 'string';
        case 'Array':
          return 'array';
        case 'FixedArray':
          return 'array';
        case 'Tuple':
          return 'tuple';
        default:
          return value.type.toLowerCase();
      }
    }

    // Fallback to JavaScript type
    return typeof value;
  };

  const renderVariables = () => {
    if (currentSnapshotId === null) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          No snapshot selected
        </div>
      );
    }

    // Get real snapshot info from EDB engine
    const snapshotInfo = getSnapshotInfo(currentSnapshotId);
    const prevSnapshotInfo = previousSnapshotId !== null ? getSnapshotInfo(previousSnapshotId) : null;

    if (!snapshotInfo) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          <div className="mb-2">Loading snapshot data...</div>
          <div className="text-xs">Snapshot {currentSnapshotId}</div>
        </div>
      );
    }

    const variables = [];

    // Add state variables from storage data
    storageData.forEach((storage, index) => {
      variables.push({
        name: `storage[${storage.slot}]`,
        type: 'bytes32',
        value: storage.value,
        scope: 'state' as const,
        changed: storage.changed || false
      });
    });

    // Add Hook-specific variables (locals and state variables)
    if (snapshotInfo.detail.Hook) {
      const hookDetail = snapshotInfo.detail.Hook;
      const prevHookDetail = prevSnapshotInfo?.detail.Hook;

      // Add local variables
      Object.entries(hookDetail.locals).forEach(([name, value]) => {
        const prevValue = prevHookDetail?.locals[name];
        variables.push({
          name,
          type: getValueType(value),
          value: value,
          scope: 'local' as const,
          changed: prevValue !== undefined && JSON.stringify(prevValue) !== JSON.stringify(value)
        });
      });

      // Add state variables from Hook snapshot
      Object.entries(hookDetail.state_variables).forEach(([name, value]) => {
        const prevValue = prevHookDetail?.state_variables[name];
        variables.push({
          name,
          type: getValueType(value),
          value: value,
          scope: 'state' as const,
          changed: prevValue !== undefined && JSON.stringify(prevValue) !== JSON.stringify(value)
        });
      });
    }

    // Add some built-in EVM variables from trace data if available
    const traceData = getTraceData();
    if (traceData?.inner?.length > 0) {
      const currentCall = traceData.inner.find(call => call.first_snapshot_id <= currentSnapshotId);
      if (currentCall) {
        variables.push({
          name: 'msg.sender',
          type: 'address',
          value: currentCall.caller,
          scope: 'builtin' as const,
          changed: false
        });

        variables.push({
          name: 'msg.target',
          type: 'address',
          value: currentCall.target,
          scope: 'builtin' as const,
          changed: false
        });
      }
    }

    if (variables.length === 0) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          <div className="mb-2">No variables available</div>
          <div className="text-xs">Snapshot {currentSnapshotId} - {snapshotInfo.detail.Opcode ? 'Opcode' : 'Hook'} type</div>
        </div>
      );
    }

    return (
      <div className="space-y-2">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Snapshot {currentSnapshotId} Variables ({snapshotInfo.detail.Opcode ? 'Opcode' : 'Hook'})
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
                    : variable.scope === 'local'
                    ? 'bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300'
                    : 'bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-300'
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

    // Get real memory data from EDB snapshot info
    const snapshotInfo = getSnapshotInfo(currentSnapshotId);

    if (!snapshotInfo) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          Loading snapshot data...
        </div>
      );
    }

    if (!snapshotInfo.detail.Opcode) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          <div className="mb-2">Memory not available</div>
          <div className="text-xs">Hook snapshots don't include raw memory data</div>
        </div>
      );
    }

    const memoryBytes = snapshotInfo.detail.Opcode.memory;

    if (!memoryBytes || memoryBytes.length === 0) {
      return (
        <div className="text-center py-4 text-gray-500 dark:text-gray-400 text-sm">
          No memory data available
        </div>
      );
    }

    const formatMemoryLine = (bytes: number[], offset: number) => {
      // Take 16 bytes at a time for display
      const lineBytes = bytes.slice(0, 16);
      const hex = lineBytes.map(b => b.toString(16).padStart(2, '0')).join('');
      const ascii = lineBytes.map(b =>
        (b >= 32 && b <= 126) ? String.fromCharCode(b) : '.'
      ).join('');

      return { hex, ascii };
    };

    const lines = [];
    for (let i = 0; i < memoryBytes.length; i += 16) {
      const lineBytes = memoryBytes.slice(i, i + 16);
      const line = formatMemoryLine(lineBytes, i);
      lines.push({ offset: i, ...line });
    }

    return (
      <div className="space-y-1">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Memory ({memoryBytes.length} bytes)
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

  const renderCalldata = () => {
    if (currentSnapshotId === null) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          No snapshot selected
        </div>
      );
    }

    // Get real calldata from EDB snapshot info
    const snapshotInfo = getSnapshotInfo(currentSnapshotId);

    if (!snapshotInfo) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          Loading snapshot data...
        </div>
      );
    }

    let calldataBytes: number[] = [];

    if (snapshotInfo.detail.Opcode) {
      // Use real calldata from opcode snapshot
      calldataBytes = snapshotInfo.detail.Opcode.calldata;
    } else {
      // For hook snapshots, fall back to trace data
      const traceData = getTraceData();
      const callForSnapshot = traceData?.inner?.find(call =>
        call.first_snapshot_id <= currentSnapshotId
      );

      if (callForSnapshot?.input) {
        const cleanCalldata = callForSnapshot.input.startsWith('0x')
          ? callForSnapshot.input.slice(2)
          : callForSnapshot.input;

        // Convert hex string to byte array
        calldataBytes = [];
        for (let i = 0; i < cleanCalldata.length; i += 2) {
          calldataBytes.push(parseInt(cleanCalldata.slice(i, i + 2), 16));
        }
      }
    }

    if (!calldataBytes || calldataBytes.length === 0) {
      return (
        <div className="text-center py-4 text-gray-500 dark:text-gray-400 text-sm">
          No calldata available for this snapshot
        </div>
      );
    }

    const formatCalldataLine = (bytes: number[], offset: number) => {
      // Take 16 bytes at a time for display
      const lineBytes = bytes.slice(0, 16);
      const hex = lineBytes.map(b => b.toString(16).padStart(2, '0')).join('');
      const ascii = lineBytes.map(b =>
        (b >= 32 && b <= 126) ? String.fromCharCode(b) : '.'
      ).join('');

      return { hex, ascii };
    };

    const lines = [];
    for (let i = 0; i < calldataBytes.length; i += 16) {
      const lineBytes = calldataBytes.slice(i, i + 16);
      const line = formatCalldataLine(lineBytes, i);
      lines.push({ offset: i, ...line });
    }

    // Extract function signature (first 4 bytes)
    const functionSig = calldataBytes.length >= 4
      ? calldataBytes.slice(0, 4).map(b => b.toString(16).padStart(2, '0')).join('')
      : '';

    return (
      <div className="space-y-1">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Calldata ({calldataBytes.length} bytes)
        </div>
        {functionSig && (
          <div className="mb-2 p-2 bg-gray-50 dark:bg-gray-700 rounded text-xs">
            <div className="text-gray-600 dark:text-gray-400 mb-1">Function Signature:</div>
            <div className="font-mono text-blue-600 dark:text-blue-400">
              0x{functionSig}
            </div>
          </div>
        )}
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

    // Get real transient storage data from EDB snapshot info
    const snapshotInfo = getSnapshotInfo(currentSnapshotId);

    if (!snapshotInfo) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400">
          Loading snapshot data...
        </div>
      );
    }

    if (!snapshotInfo.detail.Opcode) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400 text-sm">
          <div className="mb-2">⚡ Transient storage not available</div>
          <div>Hook snapshots don't include transient storage data</div>
        </div>
      );
    }

    const transientStorage = snapshotInfo.detail.Opcode.transient_storage;

    if (!transientStorage || Object.keys(transientStorage).length === 0) {
      return (
        <div className="text-center py-8 text-gray-500 dark:text-gray-400 text-sm">
          <div className="mb-2">⚡ Transient Storage (EIP-1153)</div>
          <div>No transient storage entries for this snapshot</div>
        </div>
      );
    }

    const prevSnapshotInfo = previousSnapshotId !== null ? getSnapshotInfo(previousSnapshotId) : null;
    const prevTransientStorage = prevSnapshotInfo?.detail.Opcode?.transient_storage || {};

    const transientEntries = Object.entries(transientStorage).map(([slot, value]) => {
      const prevValue = prevTransientStorage[slot];
      return {
        slot,
        value,
        changed: prevValue !== undefined && prevValue !== value
      };
    });

    return (
      <div className="space-y-2">
        <div className="text-sm text-gray-600 dark:text-gray-300 mb-3">
          Transient Storage ({transientEntries.length} slots)
        </div>
        {transientEntries.map((item, index) => (
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
                  TSTORE
                </span>
              )}
            </div>
            <div className="text-gray-800 dark:text-white break-all">
              {item.value}
            </div>
          </div>
        ))}
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