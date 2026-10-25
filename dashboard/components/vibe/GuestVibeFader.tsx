'use client';

import React, { useState, useRef, useEffect, useCallback } from 'react';

const EMOJIS = ['🥶', '😕', '😐', '🙂', '🔥']; // Scores 1 to 5

export interface GuestVibeFaderProps {
  onVote: (score: number) => void;
  disabled?: boolean;
  initialScore?: number | null;
}

export function GuestVibeFader({ onVote, disabled, initialScore }: GuestVibeFaderProps) {
  const [score, setScore] = useState<number>(initialScore || 3);
  const [isDragging, setIsDragging] = useState(false);
  const [cooldown, setCooldown] = useState(0);
  const trackRef = useRef<HTMLDivElement>(null);
  
  const handlePointerMove = useCallback((e: React.PointerEvent | PointerEvent) => {
    if (!isDragging || disabled || !trackRef.current) return;
    
    const rect = trackRef.current.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const percentage = Math.max(0, Math.min(1, 1 - (y / rect.height)));
    
    const rawScore = 1 + percentage * 4;
    const nearestScore = Math.round(rawScore);
    
    setScore(prev => {
      if (prev !== nearestScore) {
        if (typeof navigator !== 'undefined' && navigator.vibrate) {
          navigator.vibrate(10);
        }
      }
      return nearestScore;
    });
  }, [isDragging, disabled]);
  
  const handlePointerUp = useCallback(() => {
    if (isDragging && !disabled && !cooldown) {
      setIsDragging(false);
      onVote(score);
      setCooldown(3);
    }
  }, [isDragging, disabled, score, onVote, cooldown]);

  useEffect(() => {
    if (cooldown > 0) {
      const timer = setTimeout(() => setCooldown(cooldown - 1), 1000);
      return () => clearTimeout(timer);
    }
  }, [cooldown]);

  useEffect(() => {
    if (isDragging) {
      window.addEventListener('pointermove', handlePointerMove);
      window.addEventListener('pointerup', handlePointerUp);
      window.addEventListener('pointercancel', handlePointerUp);
      return () => {
        window.removeEventListener('pointermove', handlePointerMove);
        window.removeEventListener('pointerup', handlePointerUp);
        window.removeEventListener('pointercancel', handlePointerUp);
      };
    }
  }, [isDragging, handlePointerMove, handlePointerUp]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '10px' }}>
      <div 
        ref={trackRef}
        style={{
          position: 'relative',
          width: '32px',
          height: '200px',
          background: 'rgba(255, 255, 255, 0.1)',
          borderRadius: '16px',
          cursor: disabled ? 'not-allowed' : 'pointer',
          touchAction: 'none',
        }}
        onPointerDown={(e) => {
          if (disabled) return;
          setIsDragging(true);
          handlePointerMove(e);
        }}
      >
        {[1, 2, 3, 4, 5].map(s => (
          <div key={s} style={{
            position: 'absolute',
            left: '32px',
            bottom: `${((s - 1) / 4) * 100}%`,
            transform: 'translateY(50%)',
            width: '6px',
            height: '2px',
            background: 'rgba(255,255,255,0.2)',
          }} />
        ))}
        
        <div style={{
          position: 'absolute',
          bottom: 0,
          left: 0,
          width: '100%',
          height: `${((score - 1) / 4) * 100}%`,
          background: 'linear-gradient(to top, #00f0ff, #ff2bd6)',
          borderRadius: '16px',
          transition: isDragging ? 'none' : 'height 0.2s ease',
        }} />
        
        <div style={{
          position: 'absolute',
          bottom: `${((score - 1) / 4) * 100}%`,
          left: '50%',
          transform: 'translate(-50%, 50%)',
          width: '40px',
          height: '40px',
          background: '#1a1a1a',
          border: '2px solid #fff',
          borderRadius: '50%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: '24px',
          boxShadow: '0 2px 8px rgba(0,0,0,0.5)',
          transition: isDragging ? 'none' : 'bottom 0.2s ease',
          zIndex: 2,
        }}>
          {cooldown > 0 ? (
            <span style={{ fontSize: '16px', fontWeight: 'bold', color: '#fff' }}>{cooldown}s</span>
          ) : (
            EMOJIS[score - 1]
          )}
        </div>
      </div>
    </div>
  );
}
