'use client';

import React from 'react';

export interface KioskVibeMeterProps {
  score: number | null; // 0-100
}

export function KioskVibeMeter({ score }: KioskVibeMeterProps) {
  const displayScore = score === null ? 0 : score;
  const isIdle = score === null;

  // Cool -> Warm -> Hot
  const gradient = 'linear-gradient(90deg, #00f0ff 0%, #ff2bd6 50%, #ff5607 100%)';
  
  return (
    <div style={{
      width: '100%',
      height: '64px',
      background: 'rgba(0, 0, 0, 0.4)',
      position: 'relative',
      overflow: 'hidden'
    }}>
      <div style={{
        position: 'absolute',
        top: 0,
        left: 0,
        height: '100%',
        width: `${displayScore}%`,
        background: gradient,
        transition: 'width 0.4s cubic-bezier(0.2, 0.8, 0.2, 1)',
        opacity: isIdle ? 0 : 0.8,
        boxShadow: '0 0 20px rgba(255, 43, 214, 0.4)',
      }} />
      
      {/* Glow overlay */}
      <div style={{
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'linear-gradient(to bottom, rgba(255,255,255,0.1) 0%, transparent 50%, rgba(0,0,0,0.2) 100%)',
        pointerEvents: 'none'
      }} />
    </div>
  );
}
