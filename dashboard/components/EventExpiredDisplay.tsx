'use client';

import React from 'react';

export default function EventExpiredDisplay() {
  return (
    <>
      <style dangerouslySetInnerHTML={{ __html: `
        :root {
          --ground: #f3f1ec;
          --ink:    #16130f;
          --muted:  #6b6660;
          --line:   #d4d0c8;
          --spring: cubic-bezier(0.34, 1.56, 0.64, 1);
        }

        @media (prefers-color-scheme: dark) {
          :root {
            --ground: #0b0a08;
            --ink:    #f0ede6;
            --muted:  #8a857c;
            --line:   #2a2823;
          }
        }

        .expired-wrapper {
          background: var(--ground);
          color: var(--ink);
          font-family: var(--font-sans, "Plus Jakarta Sans", sans-serif);
          font-optical-sizing: auto;
          -webkit-font-smoothing: antialiased;
          text-rendering: optimizeLegibility;
          display: grid;
          place-items: center;
          padding: 24px;
          min-height: 100vh;
          width: 100%;
        }

        .expired {
          max-width: 520px;
          width: 100%;
          text-align: center;
        }

        .stage {
          width: 100%;
          aspect-ratio: 5 / 4;
          margin-bottom: 40px;
          cursor: default;
        }

        .stage svg {
          width: 100%;
          height: 100%;
          display: block;
          overflow: visible;
        }

        .stage path,
        .stage line,
        .stage circle {
          fill: none;
          stroke: currentColor;
          stroke-width: 1.5;
          stroke-linecap: round;
          stroke-linejoin: round;
          vector-effect: non-scaling-stroke;
        }
        .stage .sil    { stroke: var(--ink); }
        .stage .crease { stroke: var(--muted); }
        .stage .guide  { stroke: var(--line); stroke-width: 1; }
        .stage .dot-m  { fill: var(--muted); stroke: none; }

        .curtain {
          transform: translateY(0);
          transform-box: view-box;
          transition: transform 800ms var(--spring);
        }
        .expired:hover .curtain,
        .expired:focus-within .curtain {
          transform: translateY(12px);
        }

        .tassel {
          transform-box: view-box;
          transform-origin: 200px 174px;
          transform: rotate(0deg);
          transition: transform 700ms var(--spring);
        }
        .expired:hover .tassel,
        .expired:focus-within .tassel {
          transform: rotate(-16deg);
        }

        @media (prefers-reduced-motion: reduce) {
          .curtain, .tassel { transition: none; }
        }

        .expired h1 {
          font-family: var(--font-display, ui-serif, Georgia, serif);
          font-size: 44px;
          line-height: 1.05;
          font-weight: 600;
          letter-spacing: -1px;
          margin-bottom: 12px;
        }

        .expired p {
          font-size: 17px;
          line-height: 1.4;
          letter-spacing: -0.43px;
          color: var(--muted);
          max-width: 36ch;
          margin: 0 auto 24px;
        }

        .expired .stamp {
          font-family: var(--font-mono, monospace);
          font-size: 12px;
          letter-spacing: 0.08em;
          text-transform: uppercase;
          color: var(--muted);
        }
      `}} />

      <div className="expired-wrapper">
        <section className="expired" tabIndex={0} aria-label="Event expired">
          <div className="stage" role="img" aria-label="An isometric concert stage. The curtain is down. The stage is empty.">
            <svg viewBox="0 0 400 320" aria-hidden="true">
              {/* Light beams */}
              <line className="guide" x1="158" y1="128" x2="214" y2="214"/>
              <line className="guide" x1="270" y1="132" x2="214" y2="214"/>

              {/* Curtain */}
              <g className="curtain">
                <path className="sil" d="M 200 174
                                     L 200 138
                                     Q 208.4 152.8  216.8 147.6
                                     Q 225.2 162.4  233.6 157.2
                                     Q 242.0 172.0  250.4 166.8
                                     Q 258.8 181.6  267.2 176.4
                                     Q 275.6 191.2  284.0 186.0
                                     L 284 222
                                     L 200 174 Z"/>
                <line className="crease" x1="214" y1="146" x2="214" y2="182"/>
                <line className="crease" x1="228" y1="154" x2="228" y2="190"/>
                <line className="crease" x1="242" y1="162" x2="242" y2="198"/>
                <line className="crease" x1="256" y1="170" x2="256" y2="206"/>
                <line className="crease" x1="270" y1="178" x2="270" y2="214"/>
              </g>

              {/* Stage slab */}
              <path className="sil" d="M 200 174 L 284 222 L 228 254 L 144 206 Z"/>
              <path className="crease" d="M 284 222 L 228 254 L 228 260 L 284 228 Z"/>
              <path className="crease" d="M 228 254 L 144 206 L 144 212 L 228 260 Z"/>

              {/* Mic stand */}
              <line className="sil" x1="214" y1="214" x2="214" y2="198"/>
              <circle className="sil" cx="214" cy="196" r="3"/>

              {/* Tassel */}
              <g className="tassel">
                <line className="crease" x1="200" y1="174" x2="200" y2="188"/>
                <circle className="dot-m" cx="200" cy="192" r="3"/>
              </g>

              {/* Light sources */}
              <circle className="dot-m" cx="158" cy="128" r="4"/>
              <circle className="dot-m" cx="270" cy="132" r="4"/>
            </svg>
          </div>

          <h1>Event expired</h1>
          <p>This event has ended and is no longer accepting requests.</p>
          <div className="stamp">Ended</div>
        </section>
      </div>
    </>
  );
}
