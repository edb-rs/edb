import React, { useState, useEffect } from 'react';
import { TracePanel } from './TracePanel';
import { CodePanel } from './CodePanel';
import { DisplayPanel } from './DisplayPanel';
import { TerminalPanel } from './TerminalPanel';
import { ResizablePanel } from './ResizablePanel';
import { PanelManagerProvider, usePanelManager, PanelControls } from './PanelManager';
import { useAdvancedEDBStore } from '../store/advanced-edb-store';
import { useTheme } from '../hooks/useTheme';

type PanelType = 'trace' | 'code' | 'display' | 'terminal';
type LayoutMode = 'full' | 'compact' | 'mobile';

interface LayoutConfig {
  mode: LayoutMode;
  activePanel?: PanelType;
  showTrace: boolean;
  showCode: boolean;
  showDisplay: boolean;
  showTerminal: boolean;
}

function DebuggerLayoutInner() {
  const [windowWidth, setWindowWidth] = useState(window.innerWidth);
  const [activePanel, setActivePanel] = useState<PanelType>('trace');
  const [focusedPanel, setFocusedPanel] = useState<PanelType>('trace');

  const { disconnect } = useAdvancedEDBStore();
  const { theme, toggleTheme } = useTheme();
  const {
    panels,
    togglePanel,
    showPanel,
    hidePanel,
    setPanelSize,
    getVisiblePanels,
    isPanelVisible
  } = usePanelManager();

  useEffect(() => {
    const handleResize = () => setWindowWidth(window.innerWidth);
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  const getLayoutConfig = (): LayoutConfig => {
    if (windowWidth >= 1200) {
      // Full layout (≥120 columns equivalent)
      return {
        mode: 'full',
        showTrace: isPanelVisible('trace'),
        showCode: isPanelVisible('code'),
        showDisplay: isPanelVisible('display'),
        showTerminal: isPanelVisible('terminal')
      };
    } else if (windowWidth >= 800) {
      // Compact layout (80-119 columns equivalent)
      return {
        mode: 'compact',
        showTrace: isPanelVisible('trace'),
        showCode: isPanelVisible('code'),
        showDisplay: isPanelVisible('display'),
        showTerminal: isPanelVisible('terminal')
      };
    } else {
      // Mobile layout (<80 columns equivalent)
      return {
        mode: 'mobile',
        activePanel,
        showTrace: activePanel === 'trace' && isPanelVisible('trace'),
        showCode: activePanel === 'code' && isPanelVisible('code'),
        showDisplay: activePanel === 'display' && isPanelVisible('display'),
        showTerminal: activePanel === 'terminal' && isPanelVisible('terminal')
      };
    }
  };

  const layoutConfig = getLayoutConfig();

  const handleKeyPress = (e: KeyboardEvent) => {
    // Panel switching shortcuts
    switch (e.key) {
      case 'F1':
        e.preventDefault();
        setActivePanel('trace');
        setFocusedPanel('trace');
        break;
      case 'F2':
        e.preventDefault();
        setActivePanel('code');
        setFocusedPanel('code');
        break;
      case 'F3':
        e.preventDefault();
        setActivePanel('display');
        setFocusedPanel('display');
        break;
      case 'F4':
        e.preventDefault();
        setActivePanel('terminal');
        setFocusedPanel('terminal');
        break;
      case 'Tab':
        if (e.shiftKey) {
          // Shift+Tab - previous panel
          const panels: PanelType[] = ['trace', 'code', 'display', 'terminal'];
          const currentIndex = panels.indexOf(focusedPanel);
          const prevIndex = currentIndex > 0 ? currentIndex - 1 : panels.length - 1;
          setFocusedPanel(panels[prevIndex]);
          e.preventDefault();
        } else {
          // Tab - next panel
          const panels: PanelType[] = ['trace', 'code', 'display', 'terminal'];
          const currentIndex = panels.indexOf(focusedPanel);
          const nextIndex = currentIndex < panels.length - 1 ? currentIndex + 1 : 0;
          setFocusedPanel(panels[nextIndex]);
          e.preventDefault();
        }
        break;
    }
  };

  useEffect(() => {
    window.addEventListener('keydown', handleKeyPress);
    return () => window.removeEventListener('keydown', handleKeyPress);
  }, [focusedPanel]);

  const renderStatusBar = () => (
    <div className="bg-gray-800 text-white px-4 py-2 flex items-center justify-between text-sm">
      <div className="flex items-center space-x-4">
        <span className="text-green-400">EDB Debugger</span>
        <span className="text-gray-400">|</span>
        <span>Layout: {layoutConfig.mode}</span>
        <span className="text-gray-400">|</span>
        <span>Focus: {focusedPanel}</span>
      </div>

      <div className="flex items-center space-x-4">
        <div className="flex items-center space-x-4 text-xs">
          <span>F1: Trace</span>
          <span>F2: Code</span>
          <span>F3: Display</span>
          <span>F4: Terminal</span>
          <span className="text-gray-400">|</span>
          <span>Tab: Switch</span>
        </div>

        <div className="flex items-center space-x-2">
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

        </div>
      </div>
    </div>
  );

  const renderMobileLayout = () => (
    <div className="h-screen flex flex-col">
      {renderStatusBar()}
      <PanelControls />

      {/* Mobile Panel Selector */}
      <div className="bg-gray-100 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600">
        <div className="flex space-x-1">
          {[
            { id: 'trace', name: 'Trace', icon: '🔍' },
            { id: 'code', name: 'Code', icon: '📄' },
            { id: 'display', name: 'Display', icon: '📊' },
            { id: 'terminal', name: 'Terminal', icon: '💻' }
          ].filter((panel) => isPanelVisible(panel.id as PanelType)).map((panel) => (
            <button
              key={panel.id}
              onClick={() => setActivePanel(panel.id as PanelType)}
              className={`flex items-center space-x-1 px-3 py-2 text-sm rounded transition-colors ${
                activePanel === panel.id
                  ? 'bg-blue-500 text-white'
                  : 'text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
              }`}
            >
              <span>{panel.icon}</span>
              <span>{panel.name}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Single Panel Content */}
      <div className="flex-1 p-4">
        {layoutConfig.showTrace && activePanel === 'trace' && (
          <div className="h-full">
            <TracePanel />
          </div>
        )}
        {layoutConfig.showCode && activePanel === 'code' && (
          <div className="h-full">
            <CodePanel />
          </div>
        )}
        {layoutConfig.showDisplay && activePanel === 'display' && (
          <div className="h-full">
            <DisplayPanel />
          </div>
        )}
        {layoutConfig.showTerminal && activePanel === 'terminal' && (
          <div className="h-full">
            <TerminalPanel />
          </div>
        )}
      </div>
    </div>
  );

  const renderCompactLayout = () => (
    <div className="h-screen flex flex-col">
      {renderStatusBar()}
      <PanelControls />

      <div className="flex-1 flex flex-col p-4 space-y-2">
        {/* Main Panel Area - Show active panels */}
        <div className="flex-1 flex flex-col space-y-2">
          {/* Show trace, code, or display based on what's visible */}
          {layoutConfig.showTrace && activePanel === 'trace' && (
            <ResizablePanel
              defaultSize={panels.trace.size}
              minSize={200}
              direction="vertical"
              isVisible={true}
              onToggleVisibility={() => hidePanel('trace')}
              title="Trace"
              className="flex-1"
            >
              <TracePanel />
            </ResizablePanel>
          )}
          {layoutConfig.showCode && activePanel === 'code' && (
            <ResizablePanel
              defaultSize={panels.code.size}
              minSize={200}
              direction="vertical"
              isVisible={true}
              onToggleVisibility={() => hidePanel('code')}
              title="Code"
              className="flex-1"
            >
              <CodePanel />
            </ResizablePanel>
          )}
          {layoutConfig.showDisplay && activePanel === 'display' && (
            <ResizablePanel
              defaultSize={panels.display.size}
              minSize={200}
              direction="vertical"
              isVisible={true}
              onToggleVisibility={() => hidePanel('display')}
              title="Display"
              className="flex-1"
            >
              <DisplayPanel />
            </ResizablePanel>
          )}
        </div>

        {/* Terminal Panel */}
        {layoutConfig.showTerminal && (
          <ResizablePanel
            defaultSize={panels.terminal.size}
            minSize={150}
            maxSize={400}
            direction="vertical"
            resizeHandle="top"
            isVisible={layoutConfig.showTerminal}
            onToggleVisibility={() => hidePanel('terminal')}
            title="Terminal"
          >
            <TerminalPanel />
          </ResizablePanel>
        )}
      </div>
    </div>
  );

  const renderFullLayout = () => (
    <div className="h-screen flex flex-col">
      {renderStatusBar()}
      <PanelControls />

      <div className="flex-1 flex p-4 space-x-2">
        {/* Left Side - Trace and Code Panels */}
        <div className="flex flex-col space-y-2 min-w-0">
          {/* Trace Panel */}
          {layoutConfig.showTrace && (
            <ResizablePanel
              defaultSize={panels.trace.size}
              minSize={200}
              maxSize={600}
              direction="vertical"
              resizeHandle="bottom"
              isVisible={layoutConfig.showTrace}
              onToggleVisibility={() => hidePanel('trace')}
              title="Trace"
              onResize={(size) => setPanelSize('trace', size)}
            >
              <TracePanel />
            </ResizablePanel>
          )}

          {/* Code Panel */}
          {layoutConfig.showCode && (
            <ResizablePanel
              defaultSize={panels.code.size}
              minSize={200}
              maxSize={800}
              direction="vertical"
              resizeHandle="bottom"
              isVisible={layoutConfig.showCode}
              onToggleVisibility={() => hidePanel('code')}
              title="Code"
              onResize={(size) => setPanelSize('code', size)}
              className="flex-1"
            >
              <CodePanel />
            </ResizablePanel>
          )}
        </div>

        {/* Right Side - Display and Terminal */}
        <div className="flex flex-col space-y-2 min-w-0">
          {/* Display Panel */}
          {layoutConfig.showDisplay && (
            <ResizablePanel
              defaultSize={panels.display.size}
              minSize={250}
              maxSize={500}
              direction="vertical"
              resizeHandle="bottom"
              isVisible={layoutConfig.showDisplay}
              onToggleVisibility={() => hidePanel('display')}
              title="Display"
              onResize={(size) => setPanelSize('display', size)}
              className="flex-1"
            >
              <DisplayPanel />
            </ResizablePanel>
          )}

          {/* Terminal Panel */}
          {layoutConfig.showTerminal && (
            <ResizablePanel
              defaultSize={panels.terminal.size}
              minSize={150}
              maxSize={400}
              direction="vertical"
              resizeHandle="top"
              isVisible={layoutConfig.showTerminal}
              onToggleVisibility={() => hidePanel('terminal')}
              title="Terminal"
              onResize={(size) => setPanelSize('terminal', size)}
            >
              <TerminalPanel />
            </ResizablePanel>
          )}
        </div>
      </div>
    </div>
  );

  // Render appropriate layout based on screen size
  switch (layoutConfig.mode) {
    case 'mobile':
      return renderMobileLayout();
    case 'compact':
      return renderCompactLayout();
    case 'full':
    default:
      return renderFullLayout();
  }
}

// Main wrapper component with PanelManagerProvider
export function DebuggerLayout() {
  return (
    <PanelManagerProvider>
      <DebuggerLayoutInner />
    </PanelManagerProvider>
  );
}