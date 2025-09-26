import React, { useState } from 'react';
import {
  Mosaic,
  MosaicWindow,
  MosaicNode,
  MosaicBranch,
  getLeaves
} from 'react-mosaic-component';

import { TracePanel } from './TracePanel';
import { CodePanel } from './CodePanel';
import { DisplayPanel } from './DisplayPanel';
import { TerminalPanel } from './TerminalPanel';
import { SnapshotNavigation } from './SnapshotNavigation';
import { useAdvancedEDBStore } from '../store/advanced-edb-store';
import { useTheme } from '../hooks/useTheme';

// Import Mosaic styles
import 'react-mosaic-component/react-mosaic-component.css';
import '../styles/mosaic-theme.css';

// Panel types
export type ViewId = 'trace' | 'code' | 'display' | 'terminal';

// Panel registry
const PANEL_MAP = {
  trace: {
    title: '🔍 Trace',
    component: TracePanel,
    description: 'Execution trace and call hierarchy'
  },
  code: {
    title: '📄 Code',
    component: CodePanel,
    description: 'Source code and bytecode viewer'
  },
  display: {
    title: '📊 Display',
    component: DisplayPanel,
    description: 'Variables, stack, memory, and storage'
  },
  terminal: {
    title: '💻 Terminal',
    component: TerminalPanel,
    description: 'Interactive debugger commands'
  }
};

// Default layout - similar to IDE layout
const DEFAULT_LAYOUT: MosaicNode<ViewId> = {
  direction: 'row',
  first: {
    direction: 'column',
    first: 'trace',
    second: 'code',
    splitPercentage: 40
  },
  second: {
    direction: 'column',
    first: 'display',
    second: 'terminal',
    splitPercentage: 70
  },
  splitPercentage: 50
};

// Alternative compact layout
const COMPACT_LAYOUT: MosaicNode<ViewId> = {
  direction: 'column',
  first: {
    direction: 'row',
    first: 'trace',
    second: 'code',
    splitPercentage: 50
  },
  second: 'terminal',
  splitPercentage: 75
};

interface MosaicDebuggerLayoutProps {
  className?: string;
}

export function MosaicDebuggerLayout({ className = '' }: MosaicDebuggerLayoutProps) {
  const [currentNode, setCurrentNode] = useState<MosaicNode<ViewId> | null>(DEFAULT_LAYOUT);
  const [windowWidth, setWindowWidth] = useState(window.innerWidth);

  const {
    disconnect,
    currentSnapshotId,
    setCurrentSnapshot,
    navigateToNextCall,
    navigateToPrevCall,
    getSnapshotCount
  } = useAdvancedEDBStore();
  const { theme, toggleTheme } = useTheme();

  React.useEffect(() => {
    const handleResize = () => setWindowWidth(window.innerWidth);
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // Auto-switch to compact layout on smaller screens
  React.useEffect(() => {
    if (windowWidth < 1000 && currentNode === DEFAULT_LAYOUT) {
      setCurrentNode(COMPACT_LAYOUT);
    } else if (windowWidth >= 1000 && currentNode === COMPACT_LAYOUT) {
      setCurrentNode(DEFAULT_LAYOUT);
    }
  }, [windowWidth, currentNode]);

  // Keyboard shortcuts for debugger navigation
  React.useEffect(() => {
    const handleKeyPress = (e: KeyboardEvent) => {
      // Only handle shortcuts when not typing in inputs
      if ((e.target as HTMLElement)?.tagName === 'INPUT') return;

      switch (e.key) {
        case 's':
          if (e.ctrlKey || e.metaKey) return; // Allow browser shortcuts
          e.preventDefault();
          // Step forward
          if (currentSnapshotId !== null && currentSnapshotId < getSnapshotCount() - 1) {
            setCurrentSnapshot(currentSnapshotId + 1);
          }
          break;
        case 'a':
          if (e.ctrlKey || e.metaKey) return; // Allow browser shortcuts
          e.preventDefault();
          // Step backward
          if (currentSnapshotId !== null && currentSnapshotId > 0) {
            setCurrentSnapshot(currentSnapshotId - 1);
          }
          break;
        case 'n':
          if (e.ctrlKey || e.metaKey) return; // Allow browser shortcuts
          e.preventDefault();
          navigateToNextCall();
          break;
        case 'p':
          if (e.ctrlKey || e.metaKey) return; // Allow browser shortcuts
          e.preventDefault();
          navigateToPrevCall();
          break;
        case 'ArrowRight':
          if (e.shiftKey) {
            e.preventDefault();
            if (currentSnapshotId !== null && currentSnapshotId < getSnapshotCount() - 1) {
              setCurrentSnapshot(currentSnapshotId + 1);
            }
          }
          break;
        case 'ArrowLeft':
          if (e.shiftKey) {
            e.preventDefault();
            if (currentSnapshotId !== null && currentSnapshotId > 0) {
              setCurrentSnapshot(currentSnapshotId - 1);
            }
          }
          break;
      }
    };

    window.addEventListener('keydown', handleKeyPress);
    return () => window.removeEventListener('keydown', handleKeyPress);
  }, [currentSnapshotId, getSnapshotCount, setCurrentSnapshot, navigateToNextCall, navigateToPrevCall]);

  const onChange = (newNode: MosaicNode<ViewId> | null) => {
    setCurrentNode(newNode);
  };

  const onRelease = (newNode: MosaicNode<ViewId> | null) => {
    console.log('Mosaic layout released:', newNode);
  };

  const renderTile = (id: ViewId, path: MosaicBranch[]) => {
    const panel = PANEL_MAP[id];
    const Component = panel.component;

    return (
      <MosaicWindow<ViewId>
        path={path}
        createNode={() => 'trace'}
        title={panel.title}
        toolbarControls={[
          // Custom controls can go here
        ]}
        additionalControls={[
          // Additional controls like settings, etc.
        ]}
      >
        <div className="h-full overflow-hidden bg-white dark:bg-gray-800">
          <Component />
        </div>
      </MosaicWindow>
    );
  };

  const renderStatusBar = () => (
    <div className="bg-gray-800 text-white px-4 py-2 flex items-center justify-between text-sm">
      <div className="flex items-center space-x-4">
        <span className="text-green-400">EDB Debugger</span>
        <span className="text-gray-400">|</span>
        <SnapshotNavigation />
      </div>

      <div className="flex items-center space-x-4">
        <div className="flex items-center space-x-4 text-xs">
          <span>Drag tabs to rearrange</span>
          <span className="text-gray-400">|</span>
          <span>s/a: Step</span>
          <span>n/p: Call</span>
          <span>Shift+←/→: Navigate</span>
        </div>

        <div className="flex items-center space-x-2">
          {/* Layout Presets */}
          <select
            value="current"
            onChange={(e) => {
              if (e.target.value === 'default') {
                setCurrentNode(DEFAULT_LAYOUT);
              } else if (e.target.value === 'compact') {
                setCurrentNode(COMPACT_LAYOUT);
              }
            }}
            className="px-2 py-1 text-xs bg-gray-700 text-white rounded border border-gray-600"
          >
            <option value="current">Current Layout</option>
            <option value="default">Default Layout</option>
            <option value="compact">Compact Layout</option>
          </select>

          {/* Theme Toggle */}
          <button
            onClick={toggleTheme}
            className="p-1 rounded hover:bg-gray-700 transition-colors"
            title={`Switch to ${theme === 'light' ? 'dark' : 'light'} mode`}
          >
            {theme === 'light' ? (
              <svg className="w-4 h-4 text-gray-300" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M10 2a1 1 0 011 1v1a1 1 0 11-2 0V3a1 1 0 011-1zm4 8a4 4 0 11-8 0 4 4 0 018 0zm-.464 4.95l.707.707a1 1 0 001.414-1.414l-.707-.707a1 1 0 00-1.414 1.414zm2.12-10.607a1 1 0 010 1.414l-.706.707a1 1 0 11-1.414-1.414l.707-.707a1 1 0 011.414 0zM17 11a1 1 0 100-2h-1a1 1 0 100 2h1zm-7 4a1 1 0 011 1v1a1 1 0 11-2 0v-1a1 1 0 011-1zM5.05 6.464A1 1 0 106.465 5.05l-.708-.707a1 1 0 00-1.414 1.414l.707.707zm1.414 8.486l-.707.707a1 1 0 01-1.414-1.414l.707-.707a1 1 0 011.414 1.414zM4 11a1 1 0 100-2H3a1 1 0 000 2h1z" clipRule="evenodd" />
              </svg>
            ) : (
              <svg className="w-4 h-4 text-yellow-400" fill="currentColor" viewBox="0 0 20 20">
                <path d="M17.293 13.293A8 8 0 016.707 2.707a8.001 8.001 0 1010.586 10.586z" />
              </svg>
            )}
          </button>

          {/* Panel Info */}
          <span className="text-xs text-gray-300">
            Panels: {currentNode ? getLeaves(currentNode).length : 0}
          </span>

        </div>
      </div>
    </div>
  );

  return (
    <div className={`h-screen flex flex-col ${className}`}>
      {renderStatusBar()}

      <div className="flex-1 relative">
        {currentNode ? (
          <Mosaic<ViewId>
            renderTile={renderTile}
            value={currentNode}
            onChange={onChange}
            onRelease={onRelease}
            className={theme === 'dark' ? 'mosaic-dark-theme' : 'mosaic-light-theme'}
          />
        ) : (
          <div className="h-full flex items-center justify-center bg-gray-100 dark:bg-gray-900">
            <div className="text-center">
              <div className="text-gray-500 dark:text-gray-400 mb-4">
                <svg className="w-16 h-16 mx-auto" fill="currentColor" viewBox="0 0 20 20">
                  <path fillRule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 011 1v2a1 1 0 01-1 1H4a1 1 0 01-1-1V4zm0 4a1 1 0 011-1h12a1 1 0 011 1v2a1 1 0 01-1 1H4a1 1 0 01-1-1V8zm0 4a1 1 0 011-1h12a1 1 0 011 1v2a1 1 0 01-1 1H4a1 1 0 01-1-1v-2z" clipRule="evenodd" />
                </svg>
              </div>
              <h3 className="text-lg font-medium text-gray-800 dark:text-white mb-2">
                No Panels Open
              </h3>
              <p className="text-gray-600 dark:text-gray-300 mb-4">
                Add panels using the "Add Panel" dropdown in the status bar.
              </p>
              <button
                onClick={() => setCurrentNode(DEFAULT_LAYOUT)}
                className="px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white rounded transition-colors"
              >
                Restore Default Layout
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}