'use client';

import React, { useEffect, useRef, useState } from 'react';

// Math and Isometric Projection Helpers from Hairline kernel
const rad = (d: number) => (d * Math.PI) / 180;
const r2 = (n: number) => Math.round(n * 100) / 100;
const poly = (pts: number[][]) => 'M' + pts.map((p) => r2(p[0]) + ' ' + r2(p[1])).join('L') + 'Z';
const open = (pts: number[][]) => (pts.length < 2 ? '' : 'M' + pts.map((p) => r2(p[0]) + ' ' + r2(p[1])).join('L'));

interface Camera {
  az: number;
  k: number;
  S: number;
  ox: number;
  oy: number;
}

function Cam(azDeg: number, k: number, S: number): Camera {
  return { az: rad(azDeg), k, S, ox: 0, oy: 0 };
}

function proj(C: Camera) {
  const c = Math.cos(C.az);
  const s = Math.sin(C.az);
  const zf = Math.sqrt(1 - C.k * C.k);
  return (x: number, y: number, z: number): [number, number] => {
    const X = x * c - y * s;
    const Y = x * s + y * c;
    return [C.ox + C.S * X, C.oy + C.S * (Y * C.k - z * zf)];
  };
}

function unproj(C: Camera, sx: number, sy: number, z: number): [number, number] {
  const c = Math.cos(C.az);
  const s = Math.sin(C.az);
  const zf = Math.sqrt(1 - C.k * C.k);
  const X = (sx - C.ox) / C.S;
  const Y = ((sy - C.oy) / C.S + z * zf) / C.k;
  return [X * c + Y * s, -X * s + Y * c];
}

function fit(C: Camera, pts: number[][], cx: number, cy: number) {
  C.ox = 0;
  C.oy = 0;
  const P = proj(C);
  let a = 1e9, b = -1e9, c = 1e9, d = -1e9;
  for (const p of pts) {
    const q = P(p[0], p[1], p[2]);
    a = Math.min(a, q[0]);
    b = Math.max(b, q[0]);
    c = Math.min(c, q[1]);
    d = Math.max(d, q[1]);
  }
  C.ox = cx - (a + b) / 2;
  C.oy = cy - (c + d) / 2;
}

interface RingSample {
  u: number;
  v: number;
  nu: number;
  nv: number;
}

function rrect(u0: number, v0: number, u1: number, v1: number, r: number, n = 4): RingSample[] {
  r = Math.max(0, Math.min(r, (u1 - u0) / 2, (v1 - v0) / 2));
  const out: RingSample[] = [];
  const corners = [
    [u1 - r, v1 - r, 0],
    [u0 + r, v1 - r, 90],
    [u0 + r, v0 + r, 180],
    [u1 - r, v0 + r, 270],
  ];
  for (const [cu, cv, a0] of corners) {
    for (let k = 0; k <= n; k++) {
      const a = rad(a0 + (90 * k) / n);
      const ca = Math.cos(a);
      const sa = Math.sin(a);
      out.push({ u: cu + r * ca, v: cv + r * sa, nu: ca, nv: sa });
    }
  }
  return out;
}

function circ(R: number, n = 64): RingSample[] {
  const out: RingSample[] = [];
  for (let k = 0; k < n; k++) {
    const a = (k / n) * Math.PI * 2;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    out.push({ u: R * ca, v: R * sa, nu: ca, nv: sa });
  }
  return out;
}

function rings(x0: number, y0: number, x1: number, y1: number, r: number, b: number): [RingSample[], RingSample[]] {
  return [
    rrect(x0, y0, x1, y1, r),
    rrect(x0 + b, y0 + b, x1 - b, y1 - b, Math.max(0.3, r - b)),
  ];
}

function hull(input: number[][]): number[][] {
  const pts = input.slice().sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const cross = (o: number[], a: number[], b: number[]) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const lo: number[][] = [];
  const up: number[][] = [];
  for (const p of pts) {
    while (lo.length > 1 && cross(lo[lo.length - 2], lo[lo.length - 1], p) <= 0) lo.pop();
    lo.push(p);
  }
  for (let i = pts.length - 1; i >= 0; i--) {
    const p = pts[i];
    while (up.length > 1 && cross(up[up.length - 2], up[up.length - 1], p) <= 0) up.pop();
    up.push(p);
  }
  lo.pop();
  up.pop();
  return lo.concat(up);
}

const ringAt = (P: (x: number, y: number, z: number) => [number, number], ring: RingSample[], z: number) =>
  ring.map((q) => P(q.u, q.v, z));

const facing = (C: Camera) => {
  const s = Math.sin(C.az);
  const c = Math.cos(C.az);
  return (q: RingSample) => q.nu * s + q.nv * c >= -1e-6;
};

function run(ring: RingSample[], keep: (q: RingSample) => boolean) {
  const n = ring.length;
  let s = -1;
  for (let i = 0; i < n; i++) {
    if (keep(ring[i]) && !keep(ring[(i + n - 1) % n])) {
      s = i;
      break;
    }
  }
  if (s < 0) return keep(ring[0]) ? ring.slice() : [];
  const out: RingSample[] = [];
  for (let k = 0; k < n && keep(ring[(s + k) % n]); k++) {
    out.push(ring[(s + k) % n]);
  }
  return out;
}

function prism(
  P: (x: number, y: number, z: number) => [number, number],
  front: (q: RingSample) => boolean,
  ring: RingSample[],
  inner: RingSample[] | null,
  z0: number,
  z1: number
) {
  return {
    sil: poly(hull(ringAt(P, ring, z1).concat(ringAt(P, ring, z0)))),
    crease: inner ? open(ringAt(P, run(inner, front), z1)) : '',
  };
}

interface Spring {
  x: number;
  v: number;
  t: number;
  k: number;
  c: number;
  m: number;
  eps: number;
}

function spring(x: number, o: Partial<Spring> = {}): Spring {
  return { x, v: 0, t: x, k: o.k ?? 100, c: o.c ?? 18, m: o.m ?? 1, eps: o.eps ?? 0.01 };
}

function stepS(sp: Spring, dt: number): boolean {
  const n = Math.max(1, Math.ceil(dt * 240));
  const h = dt / n;
  for (let i = 0; i < n; i++) {
    const a = (-sp.k * (sp.x - sp.t) - sp.c * sp.v) / sp.m;
    sp.v += a * h;
    sp.x += sp.v * h;
  }
  if (Math.abs(sp.x - sp.t) < sp.eps && Math.abs(sp.v) < sp.eps * 10) {
    sp.x = sp.t;
    sp.v = 0;
    return false;
  }
  return true;
}

const W = 150, D = 116, ZB = 8, ZT = 13;
const CX = 58, CY = 60, R = 44;
const DOTS = 24, PLAY = 3.49;
const shift = (ringArr: RingSample[], x: number, y: number): RingSample[] =>
  ringArr.map((s) => ({ ...s, u: s.u + x, v: s.v + y }));

export interface HairlineTurntableProps {
  size?: 'compact' | 'full';
  isPlaying?: boolean;
  theme?: 'amber' | 'violet' | 'monochrome';
  title?: string;
  artist?: string;
}

export function HairlineTurntable({
  size = 'full',
  isPlaying = true,
  theme = 'amber',
  title,
  artist,
}: HairlineTurntableProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [pitchText, setPitchText] = useState('33⅓ RPM');
  const [isGripped, setIsGripped] = useState(false);

  const dotsRef = useRef<SVGEllipseElement[]>([]);
  const platterSilRef = useRef<SVGPathElement>(null);

  useEffect(() => {
    const C = Cam(45, 0.5, size === 'compact' ? 1.45 : 1.6);
    fit(C, [[0, 0, 0], [W, D, 0], [W, 0, 0], [0, D, 0], [W, 0, 24]], 200, 160);
    const P = proj(C);
    const targetPlay = isPlaying ? PLAY : 0;
    let coast = 0.6;
    let theta = 0;
    let omega = targetPlay;
    let hand: { a: number; sp: Spring } | null = null;
    let animId = 0;
    let lastTime = performance.now();

    const dots = dotsRef.current;
    const platSil = platterSilRef.current;

    const angle = (p: [number, number]) => Math.atan2(p[1] - CY, p[0] - CX);

    const updateGrip = (on: boolean) => {
      setIsGripped(on);
      if (platSil) {
        if (on) platSil.classList.add('hl-hi');
        else platSil.classList.remove('hl-hi');
      }
      if (dots[0]) {
        dots[0].setAttribute('class', on ? 'hl-dot-off' : 'hl-dot');
      }
    };

    const draw = () => {
      for (let k = 0; k < DOTS; k++) {
        const dotEl = dots[k];
        if (!dotEl) continue;
        const a = theta + (k / DOTS) * Math.PI * 2;
        const pt = P(CX + (R - 4) * Math.cos(a), CY + (R - 4) * Math.sin(a), ZT);
        dotEl.setAttribute('cx', String(r2(pt[0])));
        dotEl.setAttribute('cy', String(r2(pt[1])));
      }
      const pitch = (omega / PLAY - 1) * 100;
      const text = hand || Math.abs(pitch) > 0.5
        ? `${pitch >= 0 ? '+' : '−'}${Math.abs(pitch).toFixed(1)}%`
        : isPlaying ? '33⅓ RPM' : 'PAUSED';
      setPitchText(text);
    };

    const tick = (now: number) => {
      const dt = Math.min(0.05, Math.max(0, (now - lastTime) / 1000));
      lastTime = now;
      if (dt > 0) {
        if (hand) {
          stepS(hand.sp, dt);
          omega = (hand.sp.x - theta) / dt;
          theta = hand.sp.x;
        } else {
          omega += (targetPlay - omega) * (1 - Math.exp(-dt / coast));
          theta += omega * dt;
        }
        draw();
      }
      animId = requestAnimationFrame(tick);
    };

    animId = requestAnimationFrame(tick);

    // Pointer handlers on container
    const stage = containerRef.current;
    if (!stage) return () => cancelAnimationFrame(animId);

    const getStagePoint = (e: PointerEvent): [number, number] => {
      const r = stage.getBoundingClientRect();
      return [((e.clientX - r.left) / r.width) * 400, ((e.clientY - r.top) / r.height) * 320];
    };

    const handlePointerMove = (e: PointerEvent) => {
      const s = getStagePoint(e);
      const p = unproj(C, s[0], s[1], ZT);
      if (Math.hypot(p[0] - CX, p[1] - CY) > R) {
        if (hand) {
          hand = null;
          updateGrip(false);
        }
        return;
      }
      const a = angle(p);
      if (!hand) {
        hand = { a, sp: spring(theta) };
        hand.sp.v = omega;
        updateGrip(true);
      }
      let d = a - hand.a;
      d -= Math.round(d / (2 * Math.PI)) * 2 * Math.PI;
      hand.a = a;
      hand.sp.t += d;
    };

    const handlePointerDown = (e: PointerEvent) => {
      handlePointerMove(e);
    };

    const handlePointerLeave = () => {
      if (hand) {
        hand = null;
        updateGrip(false);
      }
    };

    stage.addEventListener('pointermove', handlePointerMove);
    stage.addEventListener('pointerdown', handlePointerDown);
    stage.addEventListener('pointerleave', handlePointerLeave);
    stage.addEventListener('pointerup', handlePointerLeave);

    return () => {
      cancelAnimationFrame(animId);
      stage.removeEventListener('pointermove', handlePointerMove);
      stage.removeEventListener('pointerdown', handlePointerDown);
      stage.removeEventListener('pointerleave', handlePointerLeave);
      stage.removeEventListener('pointerup', handlePointerLeave);
    };
  }, [size, isPlaying]);

  // Precompute static geometry
  const C = Cam(45, 0.5, size === 'compact' ? 1.45 : 1.6);
  fit(C, [[0, 0, 0], [W, D, 0], [W, 0, 0], [0, D, 0], [W, 0, 24]], 200, 160);
  const P = proj(C);
  const front = facing(C);

  const [pr, pi] = rings(0, 0, W, D, 8, 2);
  const plinth = prism(P, front, pr, pi, 0, ZB);
  const faderSlotD = open([P(134, 50, ZB), P(134, 102, ZB)]);
  const plat = prism(P, front, shift(circ(R, 64), CX, CY), shift(circ(R - 1.6, 64), CX, CY), ZB, ZT);
  const groove1 = open(ringAt(P, shift(circ(34, 64), CX, CY), ZT).concat([P(CX + 34, CY, ZT)]));
  const groove2 = open(ringAt(P, shift(circ(26, 64), CX, CY), ZT).concat([P(CX + 26, CY, ZT)]));
  const labelPrism = prism(P, front, shift(circ(13, 48), CX, CY), shift(circ(11.8, 48), CX, CY), ZT, ZT + 0.6);

  const [fr, fi] = rings(129, 72, 139, 80, 2, 0.8);
  const faderCap = prism(P, front, fr, fi, ZB, ZB + 4);
  const [br, bi] = rings(8, 96, 24, 108, 3, 1);
  const startBtn = prism(P, front, br, bi, ZB, ZB + 2);
  const armPost = prism(P, front, shift(circ(6, 32), 128, 22), shift(circ(5, 32), 128, 22), ZB, ZB + 9);
  const armTube = open([P(128, 22, ZB + 9), P(104, 24, ZB + 9), P(86, 32, ZT + 2)]);
  const [hr, hi] = rings(80, 29, 90, 37, 2, 0.8);
  const headshell = prism(P, front, hr, hi, ZT + 1, ZT + 3);

  const spindleCenter = P(CX, CY, ZT + 0.6);
  const dotRx = r2(0.9 * C.S);
  const dotRy = r2(0.9 * C.S * C.k);

  return (
    <div
      ref={containerRef}
      className={`hairline-turntable hairline-${size} theme-${theme}`}
      aria-label={title ? `Turntable playing ${title}${artist ? ` by ${artist}` : ''}` : 'Interactive DJ Turntable'}
      style={{
        position: 'relative',
        width: '100%',
        maxWidth: size === 'compact' ? '280px' : '440px',
        aspectRatio: '400 / 320',
        touchAction: 'none',
        userSelect: 'none',
      }}
    >
      <style jsx>{`
        .hairline-turntable {
          --hl-plate: #0d0f12;
          --hl-edge: #5b5d64;
          --hl-mid: #2b2d35;
          --hl-lo: #1c1d24;
          --hl-hi: #f59e0b;
        }
        .theme-amber {
          --hl-plate: #0d0d10;
          --hl-edge: #846332;
          --hl-mid: #382c18;
          --hl-lo: #20180d;
          --hl-hi: #f59e0b;
          --hl-glow: rgba(245, 158, 11, 0.35);
        }
        .theme-violet {
          --hl-plate: #0b0f19;
          --hl-edge: #6d5494;
          --hl-mid: #2c2144;
          --hl-lo: #171226;
          --hl-hi: #a855f7;
          --hl-glow: rgba(168, 85, 247, 0.35);
        }
        .theme-monochrome {
          --hl-plate: #0e0e0e;
          --hl-edge: #888888;
          --hl-mid: #333333;
          --hl-lo: #1e1e1e;
          --hl-hi: #ffffff;
          --hl-glow: rgba(255, 255, 255, 0.35);
        }
        svg {
          width: 100%;
          height: 100%;
          display: block;
          filter: drop-shadow(0 14px 28px rgba(0, 0, 0, 0.55));
        }
        path, polygon, ellipse {
          fill: var(--hl-plate);
          stroke: var(--hl-mid);
          stroke-width: 0.95;
          stroke-linejoin: round;
          stroke-linecap: round;
          vector-effect: non-scaling-stroke;
        }
        .hl-sil {
          stroke: var(--hl-edge);
        }
        .hl-sil.hl-hi {
          stroke: var(--hl-hi);
          filter: drop-shadow(0 0 6px var(--hl-glow));
        }
        .hl-crease {
          fill: none;
          stroke: var(--hl-mid);
        }
        .hl-groove {
          fill: none;
          stroke: var(--hl-lo);
        }
        .hl-dash {
          fill: none;
          stroke: var(--hl-mid);
          stroke-dasharray: 2 3;
        }
        .hl-dot {
          stroke: none;
          fill: var(--hl-hi);
        }
        .hl-dot-m {
          stroke: none;
          fill: var(--hl-edge);
        }
        .hl-dot-off {
          stroke: none;
          fill: var(--hl-lo);
        }
        .deck-badge {
          position: absolute;
          top: 10px;
          right: 14px;
          font-family: var(--font-mono, 'JetBrains Mono', monospace);
          font-size: 11px;
          font-weight: 700;
          letter-spacing: 1.5px;
          padding: 4px 9px;
          border-radius: 6px;
          background: rgba(0, 0, 0, 0.55);
          border: 1px solid rgba(255, 255, 255, 0.1);
          color: var(--hl-hi);
          pointer-events: none;
          backdrop-filter: blur(4px);
        }
        .scratch-hint {
          position: absolute;
          bottom: 10px;
          left: 50%;
          transform: translateX(-50%);
          font-family: var(--font-mono, 'JetBrains Mono', monospace);
          font-size: 10px;
          letter-spacing: 1px;
          color: rgba(255, 255, 255, 0.45);
          pointer-events: none;
          white-space: nowrap;
          transition: opacity 0.2s;
        }
        .hairline-turntable:hover .scratch-hint {
          color: var(--hl-hi);
        }
      `}</style>

      <span className="deck-badge">{pitchText}</span>
      <span className="scratch-hint">{isGripped ? 'SCRATCHING' : 'TOUCH TO SCRATCH'}</span>

      <svg viewBox="0 0 400 320">
        {/* Plinth */}
        <g>
          <path className="hl-sil" d={plinth.sil} />
          <path className="hl-crease" d={plinth.crease} />
          <path className="hl-dash" d={faderSlotD} />
        </g>

        {/* Platter */}
        <g>
          <path ref={platterSilRef} className="hl-sil" d={plat.sil} />
          <path className="hl-crease" d={plat.crease} />
          <path className="hl-groove" d={groove1} />
          <path className="hl-groove" d={groove2} />
        </g>

        {/* Center Label */}
        <g>
          <path className="hl-sil" d={labelPrism.sil} />
          <path className="hl-crease" d={labelPrism.crease} />
        </g>

        {/* Strobe dots around platter rim */}
        <g>
          {Array.from({ length: DOTS }).map((_, i) => (
            <ellipse
              key={i}
              ref={(el) => {
                if (el) dotsRef.current[i] = el;
              }}
              rx={dotRx}
              ry={dotRy}
              className={i === 0 ? 'hl-dot' : 'hl-dot-off'}
            />
          ))}
          {/* Spindle center dot */}
          <ellipse
            cx={spindleCenter[0]}
            cy={spindleCenter[1]}
            rx={dotRx * 1.2}
            ry={dotRy * 1.2}
            className="hl-dot-m"
          />
        </g>

        {/* Pitch fader cap */}
        <g>
          <path className="hl-sil" d={faderCap.sil} />
          <path className="hl-crease" d={faderCap.crease} />
        </g>

        {/* Start / Stop button */}
        <g>
          <path className="hl-sil" d={startBtn.sil} />
          <path className="hl-crease" d={startBtn.crease} />
        </g>

        {/* Tonearm post and arm */}
        <g>
          <path className="hl-sil" d={armPost.sil} />
          <path className="hl-crease" d={armPost.crease} />
          <path className="hl-sil" d={armTube} fill="none" />
        </g>

        {/* Headshell / cartridge */}
        <g>
          <path className="hl-sil" d={headshell.sil} />
          <path className="hl-crease" d={headshell.crease} />
        </g>
      </svg>
    </div>
  );
}
