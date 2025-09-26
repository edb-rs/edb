/**
 * @fileoverview UI component and state types
 * @description Types for UI components, themes, and application state
 */

import { EDB } from '../index';
import { Engine } from '../engine';
import { Debug } from '../debug';

export namespace UI {
  // Theme and styling
  export interface Theme {
    name: string;
    mode: 'light' | 'dark';
    colors: {
      primary: string;
      secondary: string;
      background: string;
      surface: string;
      text: string;
      textSecondary: string;
      border: string;
      error: string;
      warning: string;
      success: string;
      info: string;
    };
    syntax: {
      keyword: string;
      string: string;
      number: string;
      comment: string;
      function: string;
      variable: string;
      type: string;
      operator: string;
    };
  }

  // Panel types for the debugger UI
  export type PanelType =
    | 'code'
    | 'variables'
    | 'callTrace'
    | 'memory'
    | 'storage'
    | 'terminal'
    | 'breakpoints'
    | 'watchExpressions'
    | 'contractInfo';

  export interface Panel {
    id: string;
    type: PanelType;
    title: string;
    visible: boolean;
    position: PanelPosition;
    size: {
      width?: number;
      height?: number;
      flex?: number;
    };
    minimized: boolean;
  }

  export interface PanelPosition {
    area: 'main' | 'sidebar' | 'bottom' | 'floating';
    order: number;
  }

  // Layout configuration
  export interface Layout {
    name: string;
    panels: Panel[];
    isDefault: boolean;
  }

  // Editor configuration
  export interface EditorConfig {
    fontSize: number;
    fontFamily: string;
    tabSize: number;
    wordWrap: 'on' | 'off' | 'wordWrapColumn';
    lineNumbers: 'on' | 'off' | 'relative';
    minimap: boolean;
    scrollbar: {
      horizontal: 'auto' | 'visible' | 'hidden';
      vertical: 'auto' | 'visible' | 'hidden';
    };
    theme: string;
  }

  // Application state
  export interface AppState {
    // Connection state
    isConnected: boolean;
    connectionUrl: string;

    // Current session
    currentSession?: Debug.Session;

    // UI state
    theme: Theme;
    layout: Layout;
    editorConfig: EditorConfig;

    // Panel states
    panelStates: Record<string, any>;

    // Recently used
    recentSessions: string[];
    recentTransactions: EDB.Hash[];

    // Settings
    settings: UserSettings;
  }

  // User settings and preferences
  export interface UserSettings {
    // General
    autoConnect: boolean;
    defaultRpcUrl: string;

    // Editor
    editor: EditorConfig;

    // Debugging
    debug: {
      autoStep: boolean;
      stepDelay: number;
      maxSnapshots: number;
      autoBreakOnRevert: boolean;
    };

    // UI
    ui: {
      showLineNumbers: boolean;
      showMinimap: boolean;
      compactMode: boolean;
      animationsEnabled: boolean;
    };

    // Advanced
    advanced: {
      enableLogging: boolean;
      logLevel: 'error' | 'warn' | 'info' | 'debug';
      enableTelemetry: boolean;
    };
  }

  // Component props types
  export interface CodePanelProps {
    source?: string;
    language?: string;
    currentLine?: number;
    breakpoints?: Debug.Breakpoint[];
    onBreakpointToggle?: (line: number) => void;
    onLineClick?: (line: number) => void;
    readOnly?: boolean;
  }

  export interface VariablesPanelProps {
    variables: Engine.Variable[];
    watchExpressions: Debug.WatchExpression[];
    onVariableExpand?: (path: string) => void;
    onWatchAdd?: (expression: string) => void;
    onWatchRemove?: (id: string) => void;
  }

  export interface CallTracePanelProps {
    trace: Engine.CallTrace;
    currentFrame?: string;
    onFrameSelect?: (frameId: string) => void;
    expandedNodes?: Set<string>;
    onNodeToggle?: (nodeId: string) => void;
  }

  // Event types for UI interactions
  export type UIEvent =
    | { type: 'THEME_CHANGED'; payload: { theme: Theme } }
    | { type: 'LAYOUT_CHANGED'; payload: { layout: Layout } }
    | { type: 'PANEL_TOGGLED'; payload: { panelId: string; visible: boolean } }
    | { type: 'PANEL_RESIZED'; payload: { panelId: string; size: Panel['size'] } }
    | { type: 'SETTINGS_UPDATED'; payload: { settings: Partial<UserSettings> } }
    | { type: 'CONNECTION_STATE_CHANGED'; payload: { isConnected: boolean; url?: string } };

  // Command palette
  export interface Command {
    id: string;
    title: string;
    description?: string;
    category?: string;
    keybinding?: string;
    icon?: string;
    execute: () => void | Promise<void>;
  }

  // Keyboard shortcuts
  export interface KeyBinding {
    key: string;
    ctrlKey?: boolean;
    shiftKey?: boolean;
    altKey?: boolean;
    metaKey?: boolean;
    commandId: string;
  }

  // Context menu
  export interface ContextMenuItem {
    id: string;
    label: string;
    icon?: string;
    disabled?: boolean;
    separator?: boolean;
    submenu?: ContextMenuItem[];
    onClick?: () => void;
  }

  // Toast notifications
  export interface Toast {
    id: string;
    type: 'info' | 'success' | 'warning' | 'error';
    title: string;
    message?: string;
    duration?: number;
    actions?: Array<{
      label: string;
      onClick: () => void;
    }>;
  }

  // Modal dialogs
  export interface Modal {
    id: string;
    type: 'info' | 'confirm' | 'prompt' | 'custom';
    title: string;
    content: string | unknown; // Generic content type for framework flexibility
    buttons?: Array<{
      label: string;
      variant?: 'primary' | 'secondary' | 'danger';
      onClick: () => void;
    }>;
    onClose?: () => void;
  }
}