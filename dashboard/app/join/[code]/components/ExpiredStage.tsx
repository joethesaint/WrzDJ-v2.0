import styles from './ExpiredStage.module.css';

/**
 * The empty stage shown when an event has expired: curtain down, a lone mic,
 * two spotlights. Isometric 2:1, drawn in a 400 x 320 viewBox.
 */
export function ExpiredStage() {
  return (
    <div
      className={styles.stage}
      role="img"
      aria-label="An isometric concert stage. The curtain is down. The stage is empty."
    >
      <svg viewBox="0 0 400 320" aria-hidden="true">
        <g className={styles.curtain}>
          <path
            className={styles.sil}
            d="M 200 174 L 200 138 Q 208.4 152.8 216.8 147.6 Q 225.2 162.4 233.6 157.2 Q 242 172 250.4 166.8 Q 258.8 181.6 267.2 176.4 Q 275.6 191.2 284 186 L 284 222 L 200 174 Z"
          />
          {[214, 228, 242, 256, 270].map((x, i) => (
            <line key={x} className={styles.crease} x1={x} y1={146 + i * 8} x2={x} y2={182 + i * 8} />
          ))}
        </g>

        {/* Stage slab: top, right and front faces. */}
        <path className={styles.sil} d="M 200 174 L 284 222 L 228 254 L 144 206 Z" />
        <path className={styles.crease} d="M 284 222 L 228 254 L 228 260 L 284 228 Z" />
        <path className={styles.crease} d="M 228 254 L 144 206 L 144 212 L 228 260 Z" />

        {/* One spotlight, hanging front-left, its cone landing in a pool on the stage. */}
        <line className={styles.crease} x1="158" y1="96" x2="158" y2="118" />
        {/* The lamp: a short can aimed down the cone, lens at the front. */}
        <g transform="rotate(58.7 158 124)">
          <rect className={styles.sil} x="148" y="118" width="18" height="12" rx="2" />
          <ellipse className={styles.sil} cx="166" cy="124" rx="2.5" ry="6" />
        </g>
        <line className={styles.beam} x1="157" y1="134" x2="194" y2="216" />
        <line className={styles.beam} x1="167" y1="128" x2="234" y2="216" />
        <ellipse className={styles.crease} cx="214" cy="216" rx="20" ry="10" />

        {/* Mic stand alone in the light. */}
        <line className={styles.sil} x1="214" y1="214" x2="214" y2="198" />
        <circle className={styles.sil} cx="214" cy="196" r="3" />
      </svg>
    </div>
  );
}

export function ExpiredStamp({ label = 'Ended' }: { label?: string }) {
  return <div className={styles.stamp}>{label}</div>;
}
