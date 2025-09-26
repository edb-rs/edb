import React, { createContext, useContext, useState, ReactNode } from 'react';

type PanelType = 'trace' | 'code' | 'display' | 'terminal';

interface PanelState {
  isVisible: boolean;
  size: number;
  order: number;
}

interface PanelManagerState {
  panels: Record<PanelType, PanelState>;
  togglePanel: (panel: PanelType) => void;
  showPanel: (panel: PanelType) => void;
  hidePanel: (panel: PanelType) => void;
  setPanelSize: (panel: PanelType, size: number) => void;
  resetLayout: () => void;
  getVisiblePanels: () => PanelType[];
  isPanelVisible: (panel: PanelType) => boolean;
}

const defaultPanelState: Record<PanelType, PanelState> = {
  trace: { isVisible: true, size: 400, order: 0 },
  code: { isVisible: true, size: 500, order: 1 },
  display: { isVisible: true, size: 350, order: 2 },
  terminal: { isVisible: true, size: 200, order: 3 }
};

const PanelManagerContext = createContext<PanelManagerState | null>(null);

export function PanelManagerProvider({ children }: { children: ReactNode }) {
  const [panels, setPanels] = useState<Record<PanelType, PanelState>>(defaultPanelState);

  const togglePanel = (panel: PanelType) => {
    setPanels(prev => ({
      ...prev,
      [panel]: {
        ...prev[panel],
        isVisible: !prev[panel].isVisible
      }
    }));
  };

  const showPanel = (panel: PanelType) => {
    setPanels(prev => ({
      ...prev,
      [panel]: {
        ...prev[panel],
        isVisible: true
      }
    }));
  };

  const hidePanel = (panel: PanelType) => {
    setPanels(prev => ({
      ...prev,
      [panel]: {
        ...prev[panel],
        isVisible: false
      }
    }));
  };

  const setPanelSize = (panel: PanelType, size: number) => {
    setPanels(prev => ({
      ...prev,
      [panel]: {
        ...prev[panel],
        size
      }
    }));
  };

  const resetLayout = () => {
    setPanels(defaultPanelState);
  };

  const getVisiblePanels = (): PanelType[] => {
    return Object.entries(panels)
      .filter(([_, state]) => state.isVisible)
      .sort((a, b) => a[1].order - b[1].order)
      .map(([panel, _]) => panel as PanelType);
  };

  const isPanelVisible = (panel: PanelType): boolean => {
    return panels[panel].isVisible;
  };

  const contextValue: PanelManagerState = {
    panels,
    togglePanel,
    showPanel,
    hidePanel,
    setPanelSize,
    resetLayout,
    getVisiblePanels,
    isPanelVisible
  };

  return (
    <PanelManagerContext.Provider value={contextValue}>
      {children}
    </PanelManagerContext.Provider>
  );
}

export function usePanelManager(): PanelManagerState {
  const context = useContext(PanelManagerContext);
  if (!context) {
    throw new Error('usePanelManager must be used within a PanelManagerProvider');
  }
  return context;
}

// Panel visibility controls component
export function PanelControls() {
  const { panels, togglePanel, resetLayout, getVisiblePanels } = usePanelManager();

  const panelInfo = [
    { id: 'trace' as PanelType, name: 'Trace', icon: '🔍', key: 'F1' },
    { id: 'code' as PanelType, name: 'Code', icon: '📄', key: 'F2' },
    { id: 'display' as PanelType, name: 'Display', icon: '📊', key: 'F3' },
    { id: 'terminal' as PanelType, name: 'Terminal', icon: '💻', key: 'F4' }
  ];

  const visibleCount = getVisiblePanels().length;

  return (
    <div className="bg-gray-100 dark:bg-gray-700 px-4 py-2 border-b border-gray-200 dark:border-gray-600">
      <div className="flex items-center justify-between">
        <div className="flex items-center space-x-2">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            Panels ({visibleCount}/4):
          </span>
          <div className="flex space-x-1">
            {panelInfo.map((panel) => (
              <button
                key={panel.id}
                onClick={() => togglePanel(panel.id)}
                className={`flex items-center space-x-1 px-2 py-1 text-xs rounded transition-colors ${
                  panels[panel.id].isVisible
                    ? 'bg-blue-500 text-white'
                    : 'bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-500'
                }`}
                title={`Toggle ${panel.name} panel (${panel.key})`}
              >
                <span>{panel.icon}</span>
                <span>{panel.name}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="flex items-center space-x-2">
          <button
            onClick={resetLayout}
            className="px-3 py-1 text-xs bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded hover:bg-gray-300 dark:hover:bg-gray-500 transition-colors"
            title="Reset panel layout"
          >
            Reset Layout
          </button>
        </div>
      </div>
    </div>
  );
}