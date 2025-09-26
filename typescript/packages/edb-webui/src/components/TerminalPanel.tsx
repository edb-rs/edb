import React, { useState, useRef, useEffect } from 'react';
import { useAdvancedEDBStore } from '../store/advanced-edb-store';

interface TerminalEntry {
  id: number;
  type: 'command' | 'output' | 'error';
  content: string;
  timestamp: Date;
}

interface DebugCommand {
  name: string;
  description: string;
  syntax: string;
  category: 'execution' | 'inspection' | 'watch' | 'misc';
}

type InputMode = 'insert' | 'vim';

export function TerminalPanel() {
  const {
    currentSnapshotId,
    setCurrentSnapshot,
    navigateToNextCall,
    navigateToPrevCall,
    getSnapshotCount
  } = useAdvancedEDBStore();

  const [entries, setEntries] = useState<TerminalEntry[]>([
    {
      id: 1,
      type: 'output',
      content: 'EDB Debugger Terminal - Type "help" for available commands',
      timestamp: new Date()
    }
  ]);

  const [input, setInput] = useState('');
  const [inputMode, setInputMode] = useState<InputMode>('insert');
  const [commandHistory, setCommandHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [isExecuting, setIsExecuting] = useState(false);

  const inputRef = useRef<HTMLInputElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const nextEntryId = useRef(2);

  const availableCommands: DebugCommand[] = [
    { name: 'help', description: 'Show available commands', syntax: 'help [command]', category: 'misc' },
    { name: 'step', description: 'Step to next instruction', syntax: 'step [count]', category: 'execution' },
    { name: 'next', description: 'Step over function calls', syntax: 'next [count]', category: 'execution' },
    { name: 'prev', description: 'Step backward', syntax: 'prev [count]', category: 'execution' },
    { name: 'call', description: 'Step into next call', syntax: 'call', category: 'execution' },
    { name: 'rcall', description: 'Step back from call', syntax: 'rcall', category: 'execution' },
    { name: 'goto', description: 'Go to snapshot', syntax: 'goto <snapshot_id>', category: 'execution' },
    { name: 'stack', description: 'Show stack', syntax: 'stack [count]', category: 'inspection' },
    { name: 'memory', description: 'Show memory', syntax: 'memory [offset] [count]', category: 'inspection' },
    { name: 'sload', description: 'Read storage slot', syntax: 'sload <slot>', category: 'inspection' },
    { name: 'address', description: 'Show current address info', syntax: 'address', category: 'inspection' },
    { name: 'watch', description: 'Manage watch expressions', syntax: 'watch <add|remove|list> [expr]', category: 'watch' },
    { name: 'break', description: 'Set breakpoint', syntax: 'break <address:line>', category: 'misc' },
    { name: 'clear', description: 'Clear terminal', syntax: 'clear', category: 'misc' }
  ];

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [entries]);

  const addEntry = (type: TerminalEntry['type'], content: string) => {
    const entry: TerminalEntry = {
      id: nextEntryId.current++,
      type,
      content,
      timestamp: new Date()
    };
    setEntries(prev => [...prev, entry]);
  };

  const executeCommand = async (command: string) => {
    if (!command.trim()) return;

    // Add command to history
    setCommandHistory(prev => [command, ...prev].slice(0, 100));
    setHistoryIndex(-1);

    // Add command to terminal
    addEntry('command', `> ${command}`);
    setIsExecuting(true);

    try {
      const result = await processCommand(command.trim());
      if (result) {
        addEntry('output', result);
      }
    } catch (error) {
      addEntry('error', `Error: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setIsExecuting(false);
    }
  };

  const processCommand = async (command: string): Promise<string | null> => {
    const [cmd, ...args] = command.toLowerCase().split(' ');

    switch (cmd) {
      case 'help':
        if (args.length > 0) {
          const targetCmd = availableCommands.find(c => c.name === args[0]);
          if (targetCmd) {
            return `${targetCmd.name}: ${targetCmd.description}\nSyntax: ${targetCmd.syntax}`;
          }
          return `Unknown command: ${args[0]}`;
        }
        return availableCommands
          .reduce((acc, cmd) => {
            if (!acc[cmd.category]) acc[cmd.category] = [];
            acc[cmd.category].push(cmd);
            return acc;
          }, {} as Record<string, DebugCommand[]>)
          .execution?.map(c => `${c.name.padEnd(12)} ${c.description}`).join('\n') || 'No commands available';

      case 'step':
      case 's':
        navigateToNextCall();
        return `Stepped to snapshot ${currentSnapshotId}`;

      case 'prev':
      case 'p':
        navigateToPrevCall();
        return `Stepped back to snapshot ${currentSnapshotId}`;

      case 'goto':
        if (args.length === 0) return 'Usage: goto <snapshot_id>';
        const targetSnapshot = parseInt(args[0]);
        if (isNaN(targetSnapshot)) return 'Invalid snapshot ID';
        const snapshotCount = getSnapshotCount();
        if (targetSnapshot < 0 || targetSnapshot >= snapshotCount) {
          return `Snapshot ID must be between 0 and ${snapshotCount - 1}`;
        }
        setCurrentSnapshot(targetSnapshot);
        return `Jumped to snapshot ${targetSnapshot}`;

      case 'stack':
        const stackCount = args.length > 0 ? parseInt(args[0]) : 10;
        return `Stack (showing top ${stackCount} items):\n[Mock stack data would be shown here]`;

      case 'memory':
        const offset = args.length > 0 ? args[0] : '0x0';
        const count = args.length > 1 ? parseInt(args[1]) : 32;
        return `Memory at ${offset} (${count} bytes):\n[Mock memory data would be shown here]`;

      case 'sload':
        if (args.length === 0) return 'Usage: sload <slot>';
        return `Storage slot ${args[0]}: 0x1234567890abcdef...`;

      case 'address':
        return `Current address: ${currentSnapshotId !== null ? '0x123...abc' : 'N/A'}\nSnapshot: ${currentSnapshotId}`;

      case 'watch':
        const action = args[0];
        switch (action) {
          case 'add':
            if (args.length < 2) return 'Usage: watch add $<expression>';
            return `Added watch expression: ${args.slice(1).join(' ')}`;
          case 'remove':
            if (args.length < 2) return 'Usage: watch remove <id>';
            return `Removed watch expression ${args[1]}`;
          case 'list':
            return 'Watch expressions:\n1. balance\n2. msg.sender';
          default:
            return 'Usage: watch <add|remove|list> [expr]';
        }

      case 'clear':
        setEntries([]);
        return null;

      case 'break':
        if (args.length === 0) return 'Usage: break <address:line>';
        return `Breakpoint set at ${args[0]}`;

      default:
        // Check if it's an expression (starts with $)
        if (command.startsWith('$')) {
          const expr = command.slice(1);
          return `Evaluating: ${expr}\nResult: [Mock evaluation result]`;
        }
        return `Unknown command: ${cmd}. Type 'help' for available commands.`;
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (inputMode === 'insert') {
      if (e.key === 'Enter') {
        executeCommand(input);
        setInput('');
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (historyIndex < commandHistory.length - 1) {
          const newIndex = historyIndex + 1;
          setHistoryIndex(newIndex);
          setInput(commandHistory[newIndex]);
        }
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (historyIndex > 0) {
          const newIndex = historyIndex - 1;
          setHistoryIndex(newIndex);
          setInput(commandHistory[newIndex]);
        } else if (historyIndex === 0) {
          setHistoryIndex(-1);
          setInput('');
        }
      } else if (e.key === 'Escape') {
        setInputMode('vim');
        inputRef.current?.blur();
      }
    }
  };

  const formatTimestamp = (date: Date) => {
    return date.toLocaleTimeString('en-US', { hour12: false });
  };

  const getEntryColor = (type: TerminalEntry['type']) => {
    switch (type) {
      case 'command': return 'text-blue-600 dark:text-blue-400';
      case 'error': return 'text-red-600 dark:text-red-400';
      case 'output': return 'text-gray-800 dark:text-gray-200';
    }
  };

  return (
    <div className="h-full bg-gray-900 text-green-400 rounded-lg border border-gray-700 overflow-hidden font-mono text-sm">
      {/* Header */}
      <div className="bg-gray-800 px-4 py-2 border-b border-gray-700">
        <div className="flex items-center justify-between">
          <h3 className="font-semibold text-white">Terminal</h3>
          <div className="flex items-center space-x-2 text-xs">
            <span className={`px-2 py-1 rounded ${
              inputMode === 'insert'
                ? 'bg-green-600 text-white'
                : 'bg-blue-600 text-white'
            }`}>
              {inputMode.toUpperCase()}
            </span>
            <span className="text-gray-400">
              Snapshot: {currentSnapshotId ?? 'N/A'}
            </span>
          </div>
        </div>
      </div>

      {/* Terminal Content */}
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-4 space-y-1"
      >
        {entries.map((entry) => (
          <div key={entry.id} className="flex">
            <span className="text-gray-500 text-xs mr-3 w-20 flex-shrink-0">
              {formatTimestamp(entry.timestamp)}
            </span>
            <div className={`flex-1 ${getEntryColor(entry.type)}`}>
              <pre className="whitespace-pre-wrap break-words">
                {entry.content}
              </pre>
            </div>
          </div>
        ))}

        {/* Loading indicator */}
        {isExecuting && (
          <div className="flex items-center space-x-2 text-yellow-400">
            <span className="animate-spin">⚡</span>
            <span>Executing...</span>
          </div>
        )}
      </div>

      {/* Input Area */}
      <div className="bg-gray-800 px-4 py-2 border-t border-gray-700">
        <div className="flex items-center space-x-2">
          <span className="text-green-400">{'>'}</span>
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            className="flex-1 bg-transparent text-green-400 outline-none placeholder-gray-500"
            placeholder={inputMode === 'insert' ? 'Type command... (ESC for vim mode)' : 'Press i for insert mode'}
            disabled={inputMode === 'vim'}
          />
        </div>
      </div>

      {/* Footer with hints */}
      <div className="bg-gray-800 px-4 py-1 border-t border-gray-700">
        <div className="text-xs text-gray-500 space-x-4">
          <span>Enter: Execute</span>
          <span>↑↓: History</span>
          <span>ESC: Vim mode</span>
          <span>help: Show commands</span>
        </div>
      </div>
    </div>
  );
}