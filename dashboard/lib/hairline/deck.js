// The Deck figure, drawn with the hairline-create skill. Source: vault_D/hairline-deck/deck.js
import HL from './kernel';
/**
 * Deck: a turntable on its plinth, the platter always turning at play speed.
 * A ring of strobe dots rides the platter's edge so the turn can be read; one
 * of them, the cue mark, is bright at rest. The pointer on the platter is a
 * hand on the record: the platter follows the hand's angle round the spindle
 * on a spring, so a still hand stops it and a flick throws it. Let go and
 * friction brings it back to play speed. The read-out is the pitch against
 * play speed. The slider is the coast, in seconds.
 *
 * The pattern: push the camera. A spring while held, friction when released,
 * a hit test on the platter's top plane (which never moves).
 */
const {
  Cam, circ, clamp, facing, fit, prism, proj, rings, ringAt, unproj, spring, stepS,
  flatDot, mk, open, place, pointer, put, register, disposer, solid, reducedMotion,
} = HL;

const W = 150, D = 116, ZB = 8, ZT = 13;          // plinth, platter top
const CX = 58, CY = 60, R = 44;                   // platter
const DOTS = 24, PLAY = 3.49;                     // strobe dots; 33⅓ rpm in rad/s
const shift = (ring, x, y) => ring.map((s) => ({ ...s, u: s.u + x, v: s.v + y }));

function mount({ stage, svg, read }, value) {
  const bag = disposer();
  const C = Cam(45, 0.5, 1.55);
  fit(C, [[0, 0, 0], [W, D, 0], [W, 0, 0], [0, D, 0], [W, 0, 24]], 200, 166);
  const P = proj(C), front = facing(C);
  const play = reducedMotion() ? 0 : PLAY;
  let coast = value, theta = 0, omega = play, hand = null, shown = "";

  const g = mk("g", {}, svg);
  // Plinth, then the fader slot behind its cap, then the platter and its marks.
  const [pr, pi] = rings(0, 0, W, D, 8, 2);
  put(solid(g), prism(P, front, pr, pi, 0, ZB));
  mk("path", { class: "dash nf", d: open([P(134, 50, ZB), P(134, 102, ZB)]) }, g);

  const plat = solid(g);
  put(plat, prism(P, front, shift(circ(R, 64), CX, CY), shift(circ(R - 1.6, 64), CX, CY), ZB, ZT));
  for (const r of [34, 26]) mk("path", { class: "lo nf", d: open(ringAt(P, shift(circ(r, 64), CX, CY), ZT).concat([P(CX + r, CY, ZT)])) }, g);
  const label = solid(g);
  put(label, prism(P, front, shift(circ(13, 48), CX, CY), shift(circ(11.8, 48), CX, CY), ZT, ZT + 0.6));
  const dots = [];
  for (let k = 0; k < DOTS; k++) dots.push(flatDot(g, C, 0.9, k === 0 ? "dot" : "dot off"));
  place(flatDot(g, C, 1.1, "dot m"), P(CX, CY, ZT + 0.6));

  // Pitch fader cap, start button, tonearm post and arm: nearer parts last.
  const [fr, fi] = rings(129, 72, 139, 80, 2, 0.8);
  put(solid(g), prism(P, front, fr, fi, ZB, ZB + 4));
  const [br, bi] = rings(8, 96, 24, 108, 3, 1);
  put(solid(g), prism(P, front, br, bi, ZB, ZB + 2));
  put(solid(g), prism(P, front, shift(circ(6, 32), 128, 22), shift(circ(5, 32), 128, 22), ZB, ZB + 9));
  mk("path", { class: "sil nf", d: open([P(128, 22, ZB + 9), P(104, 24, ZB + 9), P(86, 32, ZT + 2)]) }, g);
  const [hr, hi] = rings(80, 29, 90, 37, 2, 0.8);
  put(solid(g), prism(P, front, hr, hi, ZT + 1, ZT + 3));

  function draw() {
    for (let k = 0; k < DOTS; k++) {
      const a = theta + (k / DOTS) * Math.PI * 2;
      place(dots[k], P(CX + (R - 4) * Math.cos(a), CY + (R - 4) * Math.sin(a), ZT));
    }
    const pitch = (omega / PLAY - 1) * 100;
    const text = hand || Math.abs(pitch) > 0.5 ? `${pitch >= 0 ? "+" : "−"}${Math.abs(pitch).toFixed(1)}%` : "rest";
    if (text !== shown) read.textContent = shown = text;
  }

  const B = register(stage, (dt) => {
    if (dt <= 0) return true;
    if (hand) {
      stepS(hand.sp, dt);
      omega = (hand.sp.x - theta) / dt;
      theta = hand.sp.x;
    } else {
      omega += (play - omega) * (1 - Math.exp(-dt / coast));
      theta += omega * dt;
    }
    draw();
    return hand !== null || play !== 0 || Math.abs(omega) > 1e-3;
  });
  bag.add(B.unregister);

  // One bright place: the cue mark at rest, the platter's edge while held.
  function grip(on) {
    plat.sil.classList.toggle("hi", on);
    dots[0].setAttribute("class", on ? "dot off" : "dot");
  }
  const angle = (p) => Math.atan2(p[1] - CY, p[0] - CX);

  bag.add(pointer(stage, {
    move: (s) => {
      const p = unproj(C, s[0], s[1], ZT);
      if (Math.hypot(p[0] - CX, p[1] - CY) > R) {
        if (hand) { hand = null; grip(false); B.wake(); }
        return;
      }
      const a = angle(p);
      if (!hand) {
        hand = { a, sp: spring(theta) };
        hand.sp.v = omega;
        grip(true);
      }
      let d = a - hand.a;
      d -= Math.round(d / (2 * Math.PI)) * 2 * Math.PI;
      hand.a = a;
      hand.sp.t += d;
      B.wake();
    },
    leave: () => { if (hand) { hand = null; grip(false); B.wake(); } },
  }));
  bag.add(() => svg.replaceChildren());
  draw();

  return {
    set: (v) => { coast = clamp(v, 0.1, 3); },
    destroy: bag.dispose,
  };
}

export const deck = ({
  name: "deck",
  means: "A turntable at play speed. A hand on the platter holds and scratches it; let go and it coasts back.",
  rules: [1, 4, 5, 8],
  range: [0.25, 0.6, 1.4],
  mount,
});
