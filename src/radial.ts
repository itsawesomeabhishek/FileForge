// Port of the macOS `RadialGeometry` and `WedgeShape`. Angles are measured
// counter-clockwise from +x as in the original; screen y grows downward, so
// a point at (radius, angle) is (cos a * radius, -sin a * radius).

export const DISC_RADIUS = 170;
/** Matches `EDGE_MARGIN` in `wheel.rs`. Keeps the disc on screen. */
export const EDGE_MARGIN = DISC_RADIUS + 8;

/** Same clamp as the Rust overlay: pull the centre in from the edges. */
export function clampCenter(x: number, y: number, width: number, height: number): Point {
  const clamp = (value: number, extent: number) => {
    if (extent <= EDGE_MARGIN * 2) return extent / 2;
    return Math.min(extent - EDGE_MARGIN, Math.max(EDGE_MARGIN, value));
  };
  return { x: clamp(x, width), y: clamp(y, height) };
}

export interface Point {
  x: number;
  y: number;
}

export class RadialGeometry {
  readonly innerRadius = 64;
  readonly outerRadius = 150;
  readonly gap = 4;

  constructor(readonly count: number) {}

  get slice(): number {
    return (2 * Math.PI) / Math.max(this.count, 1);
  }

  get corner(): number {
    const wanted = this.count > 8 ? 10 : 14;
    const s = Math.sin(this.slice * 0.34);
    const limit = (this.innerRadius * s - this.gap / 2) / (1 - s);
    return Math.max(4, Math.min(wanted, limit));
  }

  get labelRadius(): number {
    return this.innerRadius + (this.outerRadius - this.innerRadius) * 0.56;
  }

  get labelWidth(): number {
    const chord = 2 * this.labelRadius * Math.sin(this.slice / 2);
    return Math.max(24, chord - 6);
  }

  /** Wedge 0 is centred on the top; indexes run clockwise. */
  angles(index: number): { start: number; end: number } {
    const end = Math.PI / 2 - this.slice * index + this.slice / 2;
    return { start: end - this.slice, end };
  }

  /** Label anchor for a wedge, relative to the wheel centre. */
  midpoint(index: number): Point {
    const { start, end } = this.angles(index);
    const mid = (start + end) / 2;
    return { x: Math.cos(mid) * this.labelRadius, y: -Math.sin(mid) * this.labelRadius };
  }

  /** Wedge under `offset` (relative to the centre), or null. */
  index(offset: Point): number | null {
    if (this.count <= 0) return null;
    const distance = Math.hypot(offset.x, offset.y);
    if (distance < this.innerRadius || distance > this.outerRadius + 24) return null;
    // Screen y is down; the geometry's angle is measured with y up.
    let angle = Math.atan2(-offset.y, offset.x);
    if (angle < 0) angle += 2 * Math.PI;
    let relative = Math.PI / 2 + this.slice / 2 - angle;
    while (relative < 0) relative += 2 * Math.PI;
    while (relative >= 2 * Math.PI) relative -= 2 * Math.PI;
    return Math.min(this.count - 1, Math.floor(relative / this.slice));
  }

  /** SVG path for a wedge with rounded corners, centred on (0, 0). */
  wedgePath(index: number): string {
    const r0 = this.innerRadius;
    const r1 = this.outerRadius;
    if (this.count <= 1) {
      // A single item is a full ring.
      const ring = (r: number) =>
        `M ${r} 0 A ${r} ${r} 0 1 1 ${-r} 0 A ${r} ${r} 0 1 1 ${r} 0 Z`;
      return `${ring(r1)} ${ring(r0)}`;
    }

    const { start: a0, end: a1 } = this.angles(index);
    const k = this.corner;
    const half = this.gap / 2;
    const phiOuter = Math.asin((k + half) / (r1 - k));
    const phiInner = Math.asin((k + half) / (r0 + k));

    const pt = (radius: number, angle: number): Point => ({
      x: Math.cos(angle) * radius,
      y: -Math.sin(angle) * radius,
    });
    // Unit step toward the a1 side of the wedge (and its opposite for a0).
    const toward = (angle: number, sign: 1 | -1): Point => ({
      x: -Math.sin(angle) * sign,
      y: -Math.cos(angle) * sign,
    });
    const add = (p: Point, v: Point, d: number): Point => ({ x: p.x + v.x * d, y: p.y + v.y * d });
    const f = (p: Point) => `${p.x.toFixed(3)} ${p.y.toFixed(3)}`;
    const arc = (radius: number, sweep: 0 | 1, to: Point, large = 0) =>
      `A ${radius} ${radius} 0 ${large} ${sweep} ${f(to)}`;

    const outerStart = pt(r1, a1 - phiOuter);
    const outerEnd = pt(r1, a0 + phiOuter);
    const innerStart = pt(r0, a0 + phiInner);
    const innerEnd = pt(r0, a1 - phiInner);

    // Corner circle centres and the points where each side edge meets them.
    const cOuterA0 = pt(r1 - k, a0 + phiOuter);
    const cInnerA0 = pt(r0 + k, a0 + phiInner);
    const cInnerA1 = pt(r0 + k, a1 - phiInner);
    const cOuterA1 = pt(r1 - k, a1 - phiOuter);
    const sideOuterA0 = add(cOuterA0, toward(a0, -1), k);
    const sideInnerA0 = add(cInnerA0, toward(a0, -1), k);
    const sideInnerA1 = add(cInnerA1, toward(a1, 1), k);
    const sideOuterA1 = add(cOuterA1, toward(a1, 1), k);

    const outerSpan = this.slice - 2 * phiOuter;
    const innerSpan = this.slice - 2 * phiInner;

    return [
      `M ${f(outerStart)}`,
      arc(r1, 1, outerEnd, outerSpan > Math.PI ? 1 : 0),
      arc(k, 1, sideOuterA0),
      `L ${f(sideInnerA0)}`,
      arc(k, 1, innerStart),
      arc(r0, 0, innerEnd, innerSpan > Math.PI ? 1 : 0),
      arc(k, 1, sideInnerA1),
      `L ${f(sideOuterA1)}`,
      arc(k, 1, outerStart),
      "Z",
    ].join(" ");
  }
}
