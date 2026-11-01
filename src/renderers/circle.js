// Circle gauge: a disc whose filled wedge shrinks clockwise as time passes.
//
// Renderer interface (shared by future renderers such as an hourglass):
//   const r = createXxxRenderer(container);
//   r.update(fraction, { refill });  // fraction in [0, 1]; refill = task switch

const SVG_NS = "http://www.w3.org/2000/svg";
const R = 50;

// SVG path of the remaining wedge, starting at 12 o'clock and going clockwise.
export function wedgePath(fraction, r) {
  const f = clamp(fraction);
  const angle = f * 2 * Math.PI;
  const x = round(r + r * Math.sin(angle));
  const y = round(r - r * Math.cos(angle));
  // Near 0 or 1 the end point rounds onto the start, where an arc draws nothing.
  if (x === r && y === 0 && f < 0.5) return "";
  if (x === r && y === 0) {
    // A single arc cannot draw a full circle, so use two half arcs.
    return `M ${r} 0 A ${r} ${r} 0 1 1 ${r} ${2 * r} A ${r} ${r} 0 1 1 ${r} 0 Z`;
  }
  const largeArc = f > 0.5 ? 1 : 0;
  return `M ${r} ${r} L ${r} 0 A ${r} ${r} 0 ${largeArc} 1 ${x} ${y} Z`;
}

function clamp(f) {
  return Math.min(1, Math.max(0, f));
}

function round(n) {
  return Math.round(n * 1000) / 1000;
}

// Duration of the short transitions between minute steps and on refill.
const STEP_MS = 400;
const REFILL_MS = 700;

export function createCircleRenderer(container) {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${2 * R} ${2 * R}`);
  const track = document.createElementNS(SVG_NS, "circle");
  track.setAttribute("cx", R);
  track.setAttribute("cy", R);
  track.setAttribute("r", R);
  track.classList.add("gauge-track");
  const fill = document.createElementNS(SVG_NS, "path");
  fill.classList.add("gauge-fill");
  svg.append(track, fill);
  container.replaceChildren(svg);

  const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
  let shown = null;
  let frame = 0;

  function draw(f) {
    shown = f;
    fill.setAttribute("d", wedgePath(f, R));
  }

  // Animate once per update; nothing runs between minute steps (Spec §6).
  function animateTo(target, ms) {
    cancelAnimationFrame(frame);
    const from = shown;
    const start = performance.now();
    const tick = (t) => {
      const p = Math.min(1, (t - start) / ms);
      const eased = 1 - (1 - p) ** 3;
      draw(from + (target - from) * eased);
      if (p < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
  }

  return {
    update(fraction, { refill = false } = {}) {
      const target = clamp(fraction);
      if (shown === null || reducedMotion.matches) {
        cancelAnimationFrame(frame);
        draw(target);
      } else {
        animateTo(target, refill ? REFILL_MS : STEP_MS);
      }
    },
  };
}
