import React, { useState, useEffect } from 'react';
import { useAdvancedEDBStore } from '../store/advanced-edb-store';

interface TraceEntry {
  id: number;
  bytecode: string;
  caller: string;
  target: string;
  input: string;
  depth: number;
  call_type: any;
  result: any;
  events: Array<any>;
  first_snapshot_id: number;
  parent_id: number | null;
  expanded?: boolean;
}

export function TracePanel() {
  const { getTraceData, currentSnapshotId, setCurrentSnapshot } = useAdvancedEDBStore();
  const [expandedEntries, setExpandedEntries] = useState<Set<number>>(new Set());
  const [selectedEntry, setSelectedEntry] = useState<number | null>(null);

  const traceData = getTraceData();

  useEffect(() => {
    if (currentSnapshotId !== null && traceData?.inner) {
      // Find the entry that best matches this snapshot
      // We want the entry with the highest first_snapshot_id that's still <= currentSnapshotId
      const currentEntry = traceData.inner
        .filter(entry => entry.first_snapshot_id <= currentSnapshotId)
        .reduce((best, current) =>
          current.first_snapshot_id > best.first_snapshot_id ? current : best
        );

      if (currentEntry) {
        setSelectedEntry(currentEntry.id);
      }
    }
  }, [currentSnapshotId, traceData]);

  const toggleExpanded = (entryId: number) => {
    const newExpanded = new Set(expandedEntries);
    if (newExpanded.has(entryId)) {
      newExpanded.delete(entryId);
    } else {
      newExpanded.add(entryId);
    }
    setExpandedEntries(newExpanded);
  };

  const handleEntryClick = (entry: TraceEntry) => {
    setSelectedEntry(entry.id);
    setCurrentSnapshot(entry.first_snapshot_id);
  };

  const getCallTypeIcon = (callType: any) => {
    if (callType?.Call) return '📞';
    if (callType?.DelegateCall) return '🔄';
    if (callType?.StaticCall) return '🔍';
    if (callType?.Create) return '🏗️';
    if (callType?.Create2) return '🏭';
    return '❓';
  };

  const getResultIcon = (result: any) => {
    if (result?.Success) return '✅';
    if (result?.Revert) return '🔴';
    if (result?.OutOfGas) return '⛽';
    return '❓';
  };

  const formatAddress = (address: string) => {
    return `${address.slice(0, 6)}...${address.slice(-4)}`;
  };

  if (!traceData?.inner) {
    return (
      <div className="h-full flex items-center justify-center bg-gray-50 dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700">
        <div className="text-center">
          <div className="text-gray-500 dark:text-gray-400 mb-2">📊</div>
          <div className="text-sm text-gray-600 dark:text-gray-300">No trace data loaded</div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
      {/* Header */}
      <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600">
        <div className="flex items-center justify-between">
          <h3 className="font-semibold text-gray-800 dark:text-white">Trace</h3>
          <div className="text-sm text-gray-600 dark:text-gray-300">
            {traceData.inner.length} calls
          </div>
        </div>
      </div>

      {/* Trace Content */}
      <div className="h-full overflow-y-auto overflow-x-auto">
        <div className="p-2 space-y-1">
          {traceData.inner.map((entry, index) => {
            const isExpanded = expandedEntries.has(entry.id);
            const isSelected = selectedEntry === entry.id;

            return (
              <div key={entry.id} className="group">
                {/* Main Call Entry */}
                <div
                  className={`flex items-center p-2 rounded cursor-pointer transition-colors text-sm font-mono ${
                    isSelected
                      ? 'bg-blue-100 dark:bg-blue-900/30 border border-blue-300 dark:border-blue-700'
                      : 'hover:bg-gray-50 dark:hover:bg-gray-700'
                  }`}
                  onClick={() => handleEntryClick(entry)}
                  style={{ paddingLeft: `${8 + entry.depth * 16}px` }}
                >
                  {/* Expand/Collapse */}
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      toggleExpanded(entry.id);
                    }}
                    className="w-4 h-4 mr-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                  >
                    {entry.events?.length > 0 ? (isExpanded ? '▼' : '▶') : '·'}
                  </button>

                  {/* Call Type Icon */}
                  <span className="mr-2" title={JSON.stringify(entry.call_type)}>
                    {getCallTypeIcon(entry.call_type)}
                  </span>

                  {/* Call Info */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center space-x-2">
                      <span className="text-gray-500 dark:text-gray-400">#{entry.id}</span>
                      <span className="text-blue-600 dark:text-blue-400">
                        {formatAddress(entry.caller)}
                      </span>
                      <span className="text-gray-400">→</span>
                      <span className="text-green-600 dark:text-green-400">
                        {formatAddress(entry.target)}
                      </span>
                      <div className="flex items-center space-x-1 ml-auto">
                        <span className={`text-xs px-2 py-1 rounded ${
                          currentSnapshotId !== null && entry.first_snapshot_id <= currentSnapshotId
                            ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300'
                            : 'text-gray-500 dark:text-gray-400'
                        }`}>
                          @{entry.first_snapshot_id}
                        </span>
                        <span title={JSON.stringify(entry.result)}>
                          {getResultIcon(entry.result)}
                        </span>
                      </div>
                    </div>

                    {/* Input Data (truncated) */}
                    {entry.input && entry.input.length > 2 && (
                      <div className="text-xs text-gray-600 dark:text-gray-400 mt-1 truncate">
                        Input: {entry.input.length > 20 ? `${entry.input.slice(0, 20)}...` : entry.input}
                      </div>
                    )}
                  </div>
                </div>

                {/* Events (when expanded) */}
                {isExpanded && entry.events?.length > 0 && (
                  <div className="ml-8 mt-1 space-y-1">
                    {entry.events.map((event, eventIndex) => (
                      <div
                        key={eventIndex}
                        className="text-xs text-gray-600 dark:text-gray-400 p-2 bg-gray-50 dark:bg-gray-700 rounded font-mono"
                        style={{ paddingLeft: `${16 + entry.depth * 16}px` }}
                      >
                        <div className="flex items-center space-x-2">
                          <span className="text-yellow-500">📝</span>
                          <span>Event #{eventIndex}</span>
                          <span className="text-xs text-gray-500 dark:text-gray-400 ml-auto">
                            {JSON.stringify(event).slice(0, 50)}...
                          </span>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Footer with Navigation Hints */}
      <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-t border-gray-200 dark:border-gray-600">
        <div className="text-xs text-gray-600 dark:text-gray-300 space-x-4">
          <span>Click to navigate</span>
          <span>▶/▼ to expand events</span>
        </div>
      </div>
    </div>
  );
}