import React, { useState, useEffect } from 'react';
import { useAdvancedEDBStore } from '../store/advanced-edb-store';

type ViewMode = 'source' | 'opcodes';

interface CodeLine {
  lineNumber: number;
  content: string;
  isCurrentLine?: boolean;
  hasBreakpoint?: boolean;
}

export function CodePanel() {
  const {
    getCode,
    getSnapshotInfo,
    currentSnapshotId,
    selectedAddress,
    setSelectedAddress
  } = useAdvancedEDBStore();

  const [viewMode, setViewMode] = useState<ViewMode>('source');
  const [showFiles, setShowFiles] = useState(false);
  const [breakpoints, setBreakpoints] = useState<Set<number>>(new Set());

  const currentSnapshot = currentSnapshotId !== null ? getSnapshotInfo(currentSnapshotId) : null;
  const currentCode = selectedAddress ? getCode(selectedAddress) : null;

  // Mock source files for demonstration
  const availableFiles = [
    { address: '0x123...abc', name: 'Token.sol', type: 'source' },
    { address: '0x456...def', name: 'Exchange.sol', type: 'source' },
    { address: '0x789...ghi', name: 'Bytecode', type: 'bytecode' }
  ];

  const toggleBreakpoint = (lineNumber: number) => {
    const newBreakpoints = new Set(breakpoints);
    if (newBreakpoints.has(lineNumber)) {
      newBreakpoints.delete(lineNumber);
    } else {
      newBreakpoints.add(lineNumber);
    }
    setBreakpoints(newBreakpoints);
  };

  const parseCodeLines = (code: string | null, isOpcodes: boolean): CodeLine[] => {
    if (!code) return [];

    const lines = code.split('\n');
    return lines.map((line, index) => ({
      lineNumber: index + 1,
      content: line,
      isCurrentLine: false, // TODO: Implement current execution line tracking
      hasBreakpoint: breakpoints.has(index + 1)
    }));
  };

  const renderSourceCode = () => {
    if (!currentCode) {
      return (
        <div className="flex items-center justify-center h-64">
          <div className="text-center">
            <div className="text-gray-500 dark:text-gray-400 mb-2">📄</div>
            <div className="text-sm text-gray-600 dark:text-gray-300">
              {selectedAddress ? 'Loading source code...' : 'No address selected'}
            </div>
          </div>
        </div>
      );
    }

    // For now, show bytecode if no source is available
    const isOpcodes = !currentCode.sourceMap || viewMode === 'opcodes';
    const codeLines = parseCodeLines(
      isOpcodes ? currentCode.bytecode : 'No source code available',
      isOpcodes
    );

    return (
      <div className="font-mono text-sm">
        {codeLines.map((line) => (
          <div
            key={line.lineNumber}
            className={`flex items-center group hover:bg-gray-50 dark:hover:bg-gray-700 ${
              line.isCurrentLine ? 'bg-yellow-100 dark:bg-yellow-900/30' : ''
            }`}
          >
            {/* Line Number */}
            <div className="w-12 text-right text-gray-400 dark:text-gray-500 px-2 select-none">
              {line.lineNumber}
            </div>

            {/* Breakpoint Area */}
            <div
              className="w-6 text-center cursor-pointer hover:bg-red-100 dark:hover:bg-red-900/30"
              onClick={() => toggleBreakpoint(line.lineNumber)}
            >
              {line.hasBreakpoint ? (
                <span className="text-red-500">●</span>
              ) : (
                <span className="text-transparent group-hover:text-red-300">○</span>
              )}
            </div>

            {/* Current Line Indicator */}
            <div className="w-4">
              {line.isCurrentLine && (
                <span className="text-yellow-500">▶</span>
              )}
            </div>

            {/* Code Content */}
            <div className="flex-1 py-1 px-2 overflow-x-auto">
              <pre className="whitespace-pre-wrap text-gray-800 dark:text-gray-200">
                {line.content || ' '}
              </pre>
            </div>
          </div>
        ))}
      </div>
    );
  };

  const renderFileSelector = () => {
    return (
      <div className="absolute top-12 left-4 right-4 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-10 max-h-64 overflow-y-auto">
        <div className="p-2">
          <div className="text-sm font-medium text-gray-800 dark:text-white mb-2">Available Files</div>
          {availableFiles.map((file, index) => (
            <div
              key={index}
              className={`flex items-center p-2 rounded cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-700 ${
                selectedAddress === file.address ? 'bg-blue-50 dark:bg-blue-900/30' : ''
              }`}
              onClick={() => {
                setSelectedAddress(file.address);
                setShowFiles(false);
              }}
            >
              <div className="mr-3">
                {file.type === 'source' ? '📄' : '⚙️'}
              </div>
              <div className="flex-1">
                <div className="text-sm font-medium text-gray-800 dark:text-white">
                  {file.name}
                </div>
                <div className="text-xs text-gray-600 dark:text-gray-400">
                  {file.address}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    );
  };

  return (
    <div className="h-full bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden relative">
      {/* Header */}
      <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <h3 className="font-semibold text-gray-800 dark:text-white">Code</h3>
            {selectedAddress && (
              <span className="text-xs text-gray-600 dark:text-gray-400 font-mono">
                {selectedAddress.slice(0, 10)}...
              </span>
            )}
          </div>

          <div className="flex items-center space-x-2">
            {/* View Mode Toggle */}
            <div className="flex bg-gray-200 dark:bg-gray-600 rounded-md">
              <button
                onClick={() => setViewMode('source')}
                className={`px-3 py-1 text-xs rounded-l-md transition-colors ${
                  viewMode === 'source'
                    ? 'bg-blue-500 text-white'
                    : 'text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-500'
                }`}
              >
                Source
              </button>
              <button
                onClick={() => setViewMode('opcodes')}
                className={`px-3 py-1 text-xs rounded-r-md transition-colors ${
                  viewMode === 'opcodes'
                    ? 'bg-blue-500 text-white'
                    : 'text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-500'
                }`}
              >
                Opcodes
              </button>
            </div>

            {/* File Selector */}
            <button
              onClick={() => setShowFiles(!showFiles)}
              className="px-3 py-1 text-xs bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded-md hover:bg-gray-300 dark:hover:bg-gray-500 transition-colors"
              title="Select file (F)"
            >
              📁 Files
            </button>
          </div>
        </div>
      </div>

      {/* File Selector Overlay */}
      {showFiles && renderFileSelector()}

      {/* Code Content */}
      <div className="h-full overflow-y-auto overflow-x-auto">
        {renderSourceCode()}
      </div>

      {/* Footer */}
      <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-t border-gray-200 dark:border-gray-600">
        <div className="text-xs text-gray-600 dark:text-gray-300 space-x-4">
          <span>Click line number for breakpoint</span>
          <span>F: Files</span>
          <span>Mode: {viewMode}</span>
        </div>
      </div>
    </div>
  );
}