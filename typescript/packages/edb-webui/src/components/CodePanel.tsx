import React, { useState, useRef } from 'react';
import Editor from '@monaco-editor/react';
import * as monaco from 'monaco-editor';

type ViewMode = 'source' | 'opcodes';

// TODO: Add proper Solidity syntax highlighting
// For now using JavaScript highlighting as fallback

export function CodePanel() {
  const [viewMode, setViewMode] = useState<ViewMode>('source');
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);



  // Setup Monaco editor
  const handleEditorDidMount = (editor: monaco.editor.IStandaloneCodeEditor, monaco: any) => {
    editorRef.current = editor;

    // Configure editor options
    editor.updateOptions({
      readOnly: true,
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      automaticLayout: true,
      lineNumbers: 'on',
      glyphMargin: true,
      folding: true,
    });
  };



  const renderSourceCode = () => {
    return (
      <div className="h-full">
        <Editor
          height="100%"
          language="javascript"
          value=""
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