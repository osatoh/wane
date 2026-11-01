// Hover-only number formatting (Spec §5).

// Remaining time, rounded up so "1 min" shows until the switch actually happens.
export function formatRemaining(secs) {
  const total = Math.max(0, Math.ceil(secs / 60));
  if (total < 60) return `${total} min`;
  const h = Math.floor(total / 60);
  const m = String(total % 60).padStart(2, "0");
  return `${h} h ${m} min`;
}

