import React, { useState, useRef, useEffect, ReactNode } from 'react';

interface ResizablePanelProps {
  children: ReactNode;
  defaultSize?: number;
  minSize?: number;
  maxSize?: number;
  direction?: 'horizontal' | 'vertical';
  className?: string;
  isVisible?: boolean;
  onToggleVisibility?: () => void;
  title?: string;
  resizeHandle?: 'left' | 'right' | 'top' | 'bottom';
  onResize?: (size: number) => void;
}

export function ResizablePanel({
  children,
  defaultSize = 300,
  minSize = 150,
  maxSize = 800,
  direction = 'horizontal',
  className = '',
  isVisible = true,
  onToggleVisibility,
  title,
  resizeHandle = 'right',
  onResize
}: ResizablePanelProps) {
  const [size, setSize] = useState(defaultSize);
  const [isResizing, setIsResizing] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const startPosRef = useRef(0);
  const startSizeRef = useRef(0);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return;

      const delta = direction === 'horizontal'
        ? e.clientX - startPosRef.current
        : e.clientY - startPosRef.current;

      let newSize = startSizeRef.current;

      if (resizeHandle === 'right' || resizeHandle === 'bottom') {
        newSize = startSizeRef.current + delta;
      } else {
        newSize = startSizeRef.current - delta;
      }

      newSize = Math.max(minSize, Math.min(maxSize, newSize));
      setSize(newSize);
      onResize?.(newSize);
    };

    const handleMouseUp = () => {
      setIsResizing(false);
    };

    if (isResizing) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = direction === 'horizontal' ? 'col-resize' : 'row-resize';
      document.body.style.userSelect = 'none';
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isResizing, direction, minSize, maxSize, resizeHandle, onResize]);

  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
    startPosRef.current = direction === 'horizontal' ? e.clientX : e.clientY;
    startSizeRef.current = size;
  };

  const getResizeHandleStyle = () => {
    const baseStyle = 'absolute bg-gray-300 dark:bg-gray-600 hover:bg-blue-500 dark:hover:bg-blue-400 transition-colors cursor-';

    switch (resizeHandle) {
      case 'right':
        return `${baseStyle}col-resize right-0 top-0 w-1 h-full`;
      case 'left':
        return `${baseStyle}col-resize left-0 top-0 w-1 h-full`;
      case 'bottom':
        return `${baseStyle}row-resize bottom-0 left-0 h-1 w-full`;
      case 'top':
        return `${baseStyle}row-resize top-0 left-0 h-1 w-full`;
      default:
        return `${baseStyle}col-resize right-0 top-0 w-1 h-full`;
    }
  };

  const getPanelStyle = () => {
    if (!isVisible) return { display: 'none' };

    const sizeStyle = direction === 'horizontal'
      ? { width: `${size}px` }
      : { height: `${size}px` };

    return sizeStyle;
  };

  if (!isVisible) {
    return null;
  }

  return (
    <div
      ref={panelRef}
      className={`relative bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden ${className}`}
      style={getPanelStyle()}
    >
      {/* Panel Header with Hide Button */}
      {(title || onToggleVisibility) && (
        <div className="bg-gray-50 dark:bg-gray-700 px-3 py-2 border-b border-gray-200 dark:border-gray-600 flex items-center justify-between">
          {title && (
            <h3 className="font-medium text-gray-800 dark:text-white text-sm">
              {title}
            </h3>
          )}
          {onToggleVisibility && (
            <button
              onClick={onToggleVisibility}
              className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 text-xs p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
              title="Hide panel"
            >
              ✕
            </button>
          )}
        </div>
      )}

      {/* Panel Content */}
      <div className="h-full overflow-hidden">
        {children}
      </div>

      {/* Resize Handle */}
      <div
        className={getResizeHandleStyle()}
        onMouseDown={handleMouseDown}
        title={`Resize ${direction === 'horizontal' ? 'horizontally' : 'vertically'}`}
      >
        {/* Visual indicator for resize handle */}
        <div className="absolute inset-0 flex items-center justify-center opacity-0 hover:opacity-100 transition-opacity">
          {direction === 'horizontal' ? (
            <div className="flex flex-col space-y-0.5">
              <div className="w-0.5 h-1 bg-white rounded"></div>
              <div className="w-0.5 h-1 bg-white rounded"></div>
              <div className="w-0.5 h-1 bg-white rounded"></div>
            </div>
          ) : (
            <div className="flex space-x-0.5">
              <div className="w-1 h-0.5 bg-white rounded"></div>
              <div className="w-1 h-0.5 bg-white rounded"></div>
              <div className="w-1 h-0.5 bg-white rounded"></div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}