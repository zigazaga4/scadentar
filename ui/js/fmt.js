// Formatări pentru afișare, în stil românesc.

/** 1234.5 → "1.234,50" (la fel ca în mesajele trimise de aplicație). */
export function money(value) {
  const [whole, cents] = Math.abs(value).toFixed(2).split(".");
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ".");
  return `${value < 0 && Number(value.toFixed(2)) !== 0 ? "-" : ""}${grouped},${cents}`;
}

/** "2026-11-13" → "13.11.2026". */
export function dateRo(iso) {
  if (!iso) return "";
  const [y, m, d] = iso.split("-");
  return `${d}.${m}.${y}`;
}

export function plural(n, one, many) {
  return n === 1 ? `1 ${one}` : `${n} ${many}`;
}

/** Zile până la scadență → "azi", "mâine", "în 3 zile", "ieri", "depășită cu 4 zile". */
export function when(days) {
  if (days === 0) return "azi";
  if (days === 1) return "mâine";
  if (days === -1) return "ieri";
  return days > 1 ? `în ${days} zile` : `depășită cu ${-days} zile`;
}

export const URGENCY = {
  overdue: { label: "Depășite", cls: "overdue", icon: "🔴" },
  today: { label: "Azi", cls: "today", icon: "🟠" },
  soon: { label: "În curând", cls: "soon", icon: "🟡" },
};

export const sum = (items, pick) => items.reduce((total, item) => total + pick(item), 0);
