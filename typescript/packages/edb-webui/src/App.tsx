import React from 'react';
import { Routes, Route } from 'react-router-dom';
import { SessionDashboard } from './components/SessionDashboard';
import { DebugLayout } from './components/DebugLayout';

export function App() {
  return (
    <Routes>
      <Route path="/" element={<SessionDashboard />} />
      <Route path="/debug/:sessionId" element={<DebugLayout />} />
    </Routes>
  );
}