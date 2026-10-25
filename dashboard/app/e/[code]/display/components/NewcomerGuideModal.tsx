'use client';

import React from 'react';

interface NewcomerGuideModalProps {
  onClose: () => void;
  accentColor?: string;
}

export function NewcomerGuideModal({ onClose, accentColor = '#f59e0b' }: NewcomerGuideModalProps) {
  return (
    <div className="guide-modal-overlay" onClick={onClose} data-testid="guide-modal">
      <div className="guide-modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="guide-header">
          <div>
            <span className="guide-kicker">WELCOME TO WRZDJ</span>
            <h2 className="guide-title">How To Participate</h2>
          </div>
          <button className="guide-close-btn" onClick={onClose} aria-label="Close guide">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <path d="M1 1l12 12M13 1L1 13" />
            </svg>
          </button>
        </div>

        <p className="guide-intro">
          You have direct control over tonight&apos;s playlist. Here is how to request your tracks and get them played by the DJ:
        </p>

        <div className="guide-steps">
          {/* Step 1 */}
          <div className="guide-step-card">
            <div className="step-num" style={{ background: accentColor, color: '#000' }}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="5" y="2" width="14" height="20" rx="3" />
                <line x1="12" y1="18" x2="12.01" y2="18" />
              </svg>
            </div>
            <div className="step-body">
              <h3 className="step-heading">Request a Song</h3>
              <p className="step-desc">
                Scan the <strong>QR code</strong> with your phone camera, or tap <strong>&ldquo;REQUEST A SONG&rdquo;</strong> at the bottom of this touchscreen to search the library.
              </p>
            </div>
          </div>

          {/* Step 2 */}
          <div className="guide-step-card">
            <div className="step-num" style={{ borderColor: accentColor, color: accentColor }}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M14 9V5a3 3 0 0 0-3-3l-4 9v11h11.28a2 2 0 0 0 2-1.7l1.38-9a2 2 0 0 0-2-2.3zM7 22H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2h3" />
              </svg>
            </div>
            <div className="step-body">
              <h3 className="step-heading">Vote &amp; Climb the Queue</h3>
              <p className="step-desc">
                Check the <strong>QUEUE</strong> board. Songs with the highest crowd votes jump to the top rank. Upvote your favorites to get them into the mix sooner.
              </p>
            </div>
          </div>

          {/* Step 3 */}
          <div className="guide-step-card">
            <div className="step-num" style={{ borderColor: 'rgba(255,255,255,0.4)', color: '#fff' }}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10" />
                <circle cx="12" cy="12" r="3" />
                <line x1="12" y1="2" x2="12" y2="4" />
                <line x1="12" y1="20" x2="12" y2="22" />
              </svg>
            </div>
            <div className="step-body">
              <h3 className="step-heading">Hear It Live on Deck</h3>
              <p className="step-desc">
                Watch <strong>NOW PLAYING</strong>. As the DJ mixes live, the interactive turntable spins the current track. You can even touch the platter to scratch!
              </p>
            </div>
          </div>
        </div>

        <div className="guide-footer">
          <div className="guide-hint">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="hint-icon">
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="16" x2="12" y2="12" />
              <line x1="12" y1="8" x2="12.01" y2="8" />
            </svg>
            <span>Tip: Touch the turntable platter on screen to scratch and hear the pitch react in real-time.</span>
          </div>
          <button
            className="guide-got-it-btn"
            onClick={onClose}
            style={{
              background: accentColor,
              color: accentColor === '#ffffff' ? '#000' : '#000',
            }}
          >
            Got It &mdash; Let&apos;s Party
          </button>
        </div>
      </div>

      <style jsx>{`
        .guide-modal-overlay {
          position: fixed;
          inset: 0;
          background: rgba(0, 0, 0, 0.88);
          backdrop-filter: blur(10px);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 1200;
          padding: 24px;
        }
        .guide-modal-content {
          background: #11141c;
          border: 1px solid rgba(255, 255, 255, 0.12);
          border-radius: 24px;
          padding: 36px;
          max-width: 620px;
          width: 100%;
          box-shadow: 0 24px 60px rgba(0, 0, 0, 0.7);
          color: #fff;
          font-family: var(--font-display, 'Plus Jakarta Sans'), sans-serif;
        }
        .guide-header {
          display: flex;
          justify-content: space-between;
          align-items: flex-start;
          margin-bottom: 16px;
        }
        .guide-kicker {
          font-family: var(--font-mono, 'JetBrains Mono', monospace);
          font-size: 11px;
          letter-spacing: 2.5px;
          color: ${accentColor};
          font-weight: 700;
        }
        .guide-title {
          font-size: 32px;
          font-weight: 800;
          margin: 4px 0 0;
          letter-spacing: -0.5px;
        }
        .guide-close-btn {
          background: rgba(255, 255, 255, 0.08);
          border: 1px solid rgba(255, 255, 255, 0.1);
          color: #fff;
          width: 36px;
          height: 36px;
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          cursor: pointer;
          transition: background 0.15s;
        }
        .guide-close-btn:hover {
          background: rgba(255, 255, 255, 0.2);
        }
        .guide-intro {
          color: rgba(255, 255, 255, 0.7);
          font-size: 15px;
          line-height: 1.5;
          margin: 0 0 24px;
        }
        .guide-steps {
          display: flex;
          flex-direction: column;
          gap: 14px;
          margin-bottom: 28px;
        }
        .guide-step-card {
          display: flex;
          align-items: flex-start;
          gap: 16px;
          background: rgba(255, 255, 255, 0.04);
          border: 1px solid rgba(255, 255, 255, 0.08);
          border-radius: 16px;
          padding: 16px;
        }
        .step-num {
          width: 38px;
          height: 38px;
          border-radius: 10px;
          display: flex;
          align-items: center;
          justify-content: center;
          flex-shrink: 0;
          border: 1px solid transparent;
        }
        .step-body {
          flex: 1;
        }
        .step-heading {
          font-size: 18px;
          font-weight: 700;
          margin: 0 0 4px;
        }
        .step-desc {
          font-size: 14px;
          color: rgba(255, 255, 255, 0.65);
          margin: 0;
          line-height: 1.45;
        }
        .step-desc strong {
          color: #fff;
        }
        .guide-footer {
          display: flex;
          flex-direction: column;
          gap: 16px;
        }
        .guide-hint {
          display: flex;
          align-items: center;
          gap: 10px;
          font-size: 13px;
          color: rgba(255, 255, 255, 0.6);
          background: rgba(0, 0, 0, 0.3);
          padding: 10px 14px;
          border-radius: 12px;
          border: 1px dashed rgba(255, 255, 255, 0.15);
        }
        .hint-icon {
          flex-shrink: 0;
          color: ${accentColor};
        }
        .guide-got-it-btn {
          width: 100%;
          border: none;
          padding: 16px;
          border-radius: 999px;
          font-size: 16px;
          font-weight: 800;
          letter-spacing: 0.5px;
          cursor: pointer;
          transition: transform 0.15s, opacity 0.15s;
        }
        .guide-got-it-btn:hover {
          opacity: 0.95;
          transform: scale(1.02);
        }
        .guide-got-it-btn:active {
          transform: scale(0.99);
        }
      `}</style>
    </div>
  );
}
