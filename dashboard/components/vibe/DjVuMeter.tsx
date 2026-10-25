'use client';

import React from 'react';

export interface DjVuMeterProps {
  score: number | null; // 0-100
  voteCount: number;
}

export function DjVuMeter({ score, voteCount }: DjVuMeterProps) {
  const displayScore = score === null ? 0 : score;
  const isIdle = score === null || voteCount === 0;

  // Cool -> Warm -> Hot gradient (blue -> purple -> orange/red)
  const gradient = 'linear-gradient(90deg, #00f0ff 0%, #ff2bd6 50%, #ff5607 100%)';
  
  return (
    <div style={{
      background: 'rgba(255, 255, 255, 0.05)',
      border: '1px solid rgba(255, 255, 255, 0.1)',
      borderRadius: '8px',
      padding: '12px 16px',
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
      width: '100%',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={{ fontSize: '12px', fontFamily: 'monospace', color: '#888', letterSpacing: '1px' }}>
          VIBE METER
        </span>
        <span style={{ fontSize: '14px', fontFamily: 'monospace', fontWeight: 'bold', color: isIdle ? '#666' : '#fff' }}>
          {isIdle ? '--' : `${displayScore}%`} ({voteCount} votes)
        </span>
      </div>
      
      {/* Horizontal Bar */}
      <div style={{
        position: 'relative',
        height: '16px',
        background: 'rgba(0, 0, 0, 0.5)',
        borderRadius: '8px',
        overflow: 'hidden'
      }}>
        <div style={{
          position: 'absolute',
          top: 0,
          left: 0,
          height: '100%',
          width: `${displayScore}%`,
          background: gradient,
          transition: 'width 0.3s ease-out',
          opacity: isIdle ? 0.3 : 1
        }} />
      </div>
    </div>
  );
}
