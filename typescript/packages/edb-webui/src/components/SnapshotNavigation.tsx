import React from 'react';
import { useAdvancedEDBStore } from '../store/advanced-edb-store';

export function SnapshotNavigation() {
  const {
    currentSnapshotId,
    getSnapshotCount,
    setCurrentSnapshot,
    navigateToNextCall,
    navigateToPrevCall
  } = useAdvancedEDBStore();

  const snapshotCount = getSnapshotCount();
  const isAtStart = currentSnapshotId === 0;
  const isAtEnd = currentSnapshotId === snapshotCount - 1;

  const handleStepForward = () => {
    if (currentSnapshotId !== null && currentSnapshotId < snapshotCount - 1) {
      setCurrentSnapshot(currentSnapshotId + 1);
    }
  };

  const handleStepBackward = () => {
    if (currentSnapshotId !== null && currentSnapshotId > 0) {
      setCurrentSnapshot(currentSnapshotId - 1);
    }
  };

  const handleGoToStart = () => {
    setCurrentSnapshot(0);
  };

  const handleGoToEnd = () => {
    if (snapshotCount > 0) {
      setCurrentSnapshot(snapshotCount - 1);
    }
  };

  const handleSnapshotInput = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(e.target.value);
    if (!isNaN(value) && value >= 0 && value < snapshotCount) {
      setCurrentSnapshot(value);
    }
  };

  if (snapshotCount === 0) {
    return (
      <div className="flex items-center space-x-2 text-gray-500">
        <span className="text-xs">No snapshots available</span>
      </div>
    );
  }

  return (
    <div className="flex items-center space-x-1 bg-gray-700 rounded px-2 py-1">
      {/* Go to Start */}
      <button
        onClick={handleGoToStart}
        disabled={isAtStart}
        className="p-1 rounded hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        title="Go to first snapshot"
      >
        <svg className="w-4 h-4 text-white" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M15.707 15.707a1 1 0 01-1.414 0l-5-5a1 1 0 010-1.414l5-5a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 010 1.414zm-6 0a1 1 0 01-1.414 0l-5-5a1 1 0 010-1.414l5-5a1 1 0 011.414 1.414L5.414 10l4.293 4.293a1 1 0 010 1.414z" clipRule="evenodd" />
        </svg>
      </button>

      {/* Step Backward */}
      <button
        onClick={handleStepBackward}
        disabled={isAtStart}
        className="p-1 rounded hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        title="Previous snapshot"
      >
        <svg className="w-4 h-4 text-white" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clipRule="evenodd" />
        </svg>
      </button>

      {/* Current Snapshot Input */}
      <div className="flex items-center space-x-2 bg-gray-800 px-2 py-1 rounded">
        <span className="text-xs text-gray-400">Snapshot:</span>
        <input
          type="number"
          min="0"
          max={snapshotCount - 1}
          value={currentSnapshotId ?? 0}
          onChange={handleSnapshotInput}
          className="w-14 px-1 py-0.5 text-xs bg-gray-900 text-white border border-gray-600 rounded focus:outline-none focus:border-blue-500 text-center"
          title="Current snapshot ID"
        />
        <span className="text-xs text-gray-400">
          of {snapshotCount - 1}
        </span>
      </div>

      {/* Step Forward */}
      <button
        onClick={handleStepForward}
        disabled={isAtEnd}
        className="p-1 rounded hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        title="Next snapshot"
      >
        <svg className="w-4 h-4 text-white" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clipRule="evenodd" />
        </svg>
      </button>

      {/* Go to End */}
      <button
        onClick={handleGoToEnd}
        disabled={isAtEnd}
        className="p-1 rounded hover:bg-gray-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        title="Go to last snapshot"
      >
        <svg className="w-4 h-4 text-white" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M10.293 15.707a1 1 0 010-1.414L14.586 11H3a1 1 0 110-2h11.586l-4.293-4.293a1 1 0 011.414-1.414l6 6a1 1 0 010 1.414l-6 6a1 1 0 01-1.414 0z" clipRule="evenodd" />
        </svg>
      </button>

      <div className="w-px h-4 bg-gray-600 mx-2"></div>

      {/* Call Navigation */}
      <button
        onClick={() => navigateToPrevCall()}
        className="p-1 rounded hover:bg-gray-600 transition-colors"
        title="Previous call"
      >
        <svg className="w-4 h-4 text-blue-400" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clipRule="evenodd" />
        </svg>
      </button>

      <button
        onClick={() => navigateToNextCall()}
        className="p-1 rounded hover:bg-gray-600 transition-colors"
        title="Next call"
      >
        <svg className="w-4 h-4 text-blue-400" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M3 10a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1z" clipRule="evenodd" />
        </svg>
      </button>

      {/* Playback Controls */}
      <div className="w-px h-4 bg-gray-600 mx-2"></div>

      <button
        className="p-1 rounded hover:bg-gray-600 transition-colors"
        title="Play/Pause execution"
      >
        <svg className="w-4 h-4 text-green-400" fill="currentColor" viewBox="0 0 20 20">
          <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clipRule="evenodd" />
        </svg>
      </button>

      {/* Step Controls */}
      <div className="text-xs text-gray-400 ml-2">
        <div className="flex space-x-1">
          <span title="Step: s">[s]</span>
          <span title="Next: n">[n]</span>
          <span title="Continue: c">[c]</span>
        </div>
      </div>
    </div>
  );
}