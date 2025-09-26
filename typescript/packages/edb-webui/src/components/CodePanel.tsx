import React, { useState, useRef, useEffect } from 'react';
import Editor from '@monaco-editor/react';
import * as monaco from 'monaco-editor';
import { useAdvancedEDBStore } from '../store/advanced-edb-store';

type ViewMode = 'source' | 'opcodes';

// TODO: Add proper Solidity syntax highlighting
// For now using JavaScript highlighting as fallback

export function CodePanel() {
  const {
    getCode,
    getSourceFile,
    getTraceData,
    getSnapshotCount,
    getSnapshotInfo,
    currentSnapshotId,
  } = useAdvancedEDBStore();

  const [viewMode, setViewMode] = useState<ViewMode>('source');
  const [sourceContent, setSourceContent] = useState<string>('');
  const [currentFilePath, setCurrentFilePath] = useState<string | null>(null);
  const [isCaching, setIsCaching] = useState<boolean>(false);
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const hasPreloadedAll = useRef<boolean>(false);
  const [currentDecorations, setCurrentDecorations] = useState<string[]>([]);

  // Get current snapshot's code data
  const currentCode = currentSnapshotId !== null ? getCode(currentSnapshotId) : null;
  const currentSnapshot = currentSnapshotId !== null ? getSnapshotInfo(currentSnapshotId) : null;
  const traceData = getTraceData();
  const snapshotCount = getSnapshotCount();



  // Cache all source files on initial load
  useEffect(() => {
    if (snapshotCount === 0 || hasPreloadedAll.current || isCaching) return;

    setIsCaching(true);
    hasPreloadedAll.current = true;

    // Preload all snapshots
    const preloadAllSnapshots = async () => {
      for (let i = 0; i < snapshotCount; i++) {
        getCode(i);
        // Small delay to avoid overwhelming the server
        await new Promise(resolve => setTimeout(resolve, 20));
      }
      setIsCaching(false);
    };

    preloadAllSnapshots();
  }, [snapshotCount, getCode, isCaching]);

  // Load source code when snapshot changes
  useEffect(() => {
    if (currentSnapshotId !== null) {
      // Trigger code loading for the current snapshot
      getCode(currentSnapshotId);
    }
  }, [currentSnapshotId, getCode]);

  // Update source content when code data changes
  useEffect(() => {
    if (viewMode === 'source' && currentCode?.Source?.sources) {
      // Determine which file to show based on current snapshot
      let targetFilePath: string | null = null;
      let fileContent = '';

      // If we have a Hook snapshot, show the file being executed
      if (currentSnapshot?.detail.Hook?.path) {
        const hookPath = currentSnapshot.detail.Hook.path;
        if (currentCode.Source.sources[hookPath]) {
          targetFilePath = hookPath;
          fileContent = currentCode.Source.sources[hookPath];
        }
      }

      // If no Hook path or file not found, show the first available source file
      if (!targetFilePath) {
        const sourceFiles = Object.entries(currentCode.Source.sources);
        if (sourceFiles.length > 0) {
          const [filePath, content] = sourceFiles[0];
          targetFilePath = filePath;
          fileContent = content;
        }
      }

      if (targetFilePath) {
        setCurrentFilePath(targetFilePath);
        setSourceContent(fileContent);
      } else {
        setCurrentFilePath(null);
        setSourceContent('// No source code available for this snapshot');
      }
    } else if (viewMode === 'opcodes' && currentCode?.Opcode?.codes) {
      // Format opcodes with PC and instruction
      const opcodes = Object.entries(currentCode.Opcode.codes)
        .sort(([a], [b]) => parseInt(a) - parseInt(b))
        .map(([pc, instruction]) => `${pc.padStart(6, ' ')}: ${instruction}`)
        .join('\n');
      setCurrentFilePath(null);
      setSourceContent(opcodes);
    } else {
      setCurrentFilePath(null);
      setSourceContent(viewMode === 'source'
        ? '// Loading source code...'
        : '// Loading opcodes...');
    }
  }, [currentCode, currentSnapshot, viewMode]);

  // Calculate current debugging line from Hook offset
  const getCurrentLine = React.useCallback((): number | null => {
    // Only show highlighting in source mode
    if (viewMode !== 'source') {
      return null;
    }

    // Must have current snapshot with Hook data
    if (!currentSnapshot?.detail.Hook || !currentFilePath) {
      return null;
    }

    const hookDetail = currentSnapshot.detail.Hook;

    // CRITICAL: Only show current line if we're viewing the EXACT file that's being executed
    // AND this is the file associated with the current snapshot
    if (hookDetail.path !== currentFilePath) {
      return null;
    }

    // Get source content and convert character offset to line number
    if (typeof hookDetail.offset === 'number') {
      // Convert character offset to line number using the same algorithm as TUI
      // TUI: s[..offset + 1].lines().count()
      const textUpToOffset = sourceContent.substring(0, hookDetail.offset + 1);
      return textUpToOffset.split('\n').length; // 1-based line numbering
    }

    return null;
  }, [currentSnapshot, currentFilePath, sourceContent, viewMode]);

  // Update line highlighting when snapshot changes
  useEffect(() => {
    if (!editorRef.current) return;

    const editor = editorRef.current;
    const currentLine = getCurrentLine();

    // Always clear previous decorations first
    const clearedDecorations = editor.deltaDecorations(currentDecorations, []);

    const decorationsToAdd = [];

    // Add current line highlighting ONLY if we have a valid current line for this snapshot
    if (currentLine && currentLine > 0) {
      decorationsToAdd.push({
        range: new monaco.Range(currentLine, 1, currentLine, 1),
        options: {
          isWholeLine: true,
          className: 'current-debug-line',
          glyphMarginClassName: 'current-debug-line-glyph'
        }
      });

      // Scroll to current line and center it only when highlighting
      editor.revealLineInCenter(currentLine);
    }

    // Apply new decorations (empty array if no current line)
    const finalDecorations = editor.deltaDecorations([], decorationsToAdd);
    setCurrentDecorations(finalDecorations);

  }, [currentSnapshot, currentSnapshotId, currentFilePath, viewMode, getCurrentLine]);

  // Setup Monaco editor
  const handleEditorDidMount = (editor: monaco.editor.IStandaloneCodeEditor, monaco: any) => {
    editorRef.current = editor;

    // Configure editor options
    editor.updateOptions({
      readOnly: false, // Allow users to navigate freely
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      automaticLayout: true,
      lineNumbers: 'on',
      glyphMargin: true,
      folding: true,
    });
  };



  const renderSourceCode = () => {
    const language = viewMode === 'source' ? 'javascript' : 'text';

    return (
      <div className="h-full">
        <Editor
          height="100%"
          language={language}
          value={sourceContent}
          onMount={handleEditorDidMount}
          theme="vs-dark"
          options={{
            readOnly: true,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            automaticLayout: true,
            lineNumbers: 'on',
            glyphMargin: true,
            folding: true,
            wordWrap: 'on',
            fontSize: 14,
            fontFamily: 'Monaco, Menlo, "Ubuntu Mono", monospace',
          }}
        />
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
            {currentSnapshotId !== null && (
              <span className="text-xs text-gray-600 dark:text-gray-400">
                Snapshot {currentSnapshotId}
                {currentFilePath && (
                  <span className="ml-2 text-gray-500">• {currentFilePath.split('/').pop()}</span>
                )}
                {isCaching && (
                  <span className="ml-2 text-blue-500">• Caching all snapshots...</span>
                )}
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

          </div>
        </div>
      </div>


      {/* Code Content */}
      <div className="h-full overflow-y-auto overflow-x-auto">
        {renderSourceCode()}
      </div>

      {/* Footer */}
      <div className="bg-gray-50 dark:bg-gray-700 px-4 py-2 border-t border-gray-200 dark:border-gray-600">
        <div className="text-xs text-gray-600 dark:text-gray-300 space-x-4">
          <span>Mode: {viewMode}</span>
        </div>
      </div>
    </div>
  );
}