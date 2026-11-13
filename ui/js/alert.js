// Fereastra de alertă: apare deasupra tuturor ferestrelor când sunt scadențe.
import { api, on } from "./api.js";
import { clear, guarded, h } from "./dom.js";
import { URGENCY, dateRo, money, plural, when } from "./fmt.js";

const root = document.getElementById("root");

function section(urgency, groups, title = null) {
  if (!groups.length) return null;
  const { label, cls } = URGENCY[urgency];
  return h(
    "div",
    {},
    h("h3", { class: cls }, title ?? `${label} (${groups.length})`),
    groups.map((g) =>
      h(
        "div",
        { class: "alert-line", title: g.facturi.join(", ") },
        h("span", { class: "who" }, g.foaie, g.count > 1 && h("span", { class: "muted small" }, `  · ${plural(g.count, "factură", "facturi")}`)),
        h("span", { class: "when" }, `${dateRo(g.scadenta)} · ${when(g.days)}`),
        h("span", { class: "amount" }, money(g.totalRest)),
      ),
    ),
  );
}

function render(digest) {
  clear(root);
  const empty = !digest || !digest.groups.length;
  const serious = !empty && digest.serious > 0;
  const isSerious = (g) => g.critical && g.urgency === "overdue";
  const others = empty ? [] : digest.groups.filter((g) => !isSerious(g));
  document.title = serious ? "🚨 ATENȚIE — scadență depășită la un contract important" : "Scadențe de plată";
  root.append(
    h(
      "div",
      { class: `alert-head ${empty ? "calm" : ""} ${serious ? "serious" : ""}` },
      h("h1", {}, empty ? "Nicio scadență acum" : serious ? "🚨 ATENȚIE — scadență depășită" : "Scadențe de plată"),
      h("p", {}, empty ? "Totul este la zi." : serious ? `${plural(digest.serious, "factură depășită", "facturi depășite")} la contracte importante · ${digest.date}` : `${digest.title} · ${digest.date}`),
    ),
    h(
      "div",
      { class: "alert-body" },
      serious && section("overdue", digest.groups.filter(isSerious), "🚨 Contracte importante — depășite"),
      serious && h("p", { class: "serious-note" }, "Plătește cât mai repede. După plată, treci „Rest de plată” la 0 în Excel și alerta se oprește."),
      ["overdue", "today", "soon"].map((u) => section(u, others.filter((g) => g.urgency === u), serious && u === "overdue" ? `Alte scadențe depășite (${others.filter((g) => g.urgency === u).length})` : null)),
    ),
    h(
      "div",
      { class: "alert-foot" },
      h("button", { class: "btn primary", onclick: () => guarded(() => api.dismissAlert()) }, "Am văzut"),
      h("button", { class: "btn", onclick: () => guarded(() => api.snoozeAlerts(60)) }, "Amână 1 oră"),
      !serious && h("button", { class: "btn", onclick: () => guarded(() => api.snoozeAlerts(240)) }, "Amână 4 ore"),
      h("button", { class: "btn", onclick: async () => { await guarded(() => api.showMain()); } }, "Deschide aplicația"),
      h("div", { class: "grow" }),
      empty ? null : h("span", { class: "alert-total" }, `Total: ${money(digest.totalRest)}`),
    ),
  );
}

render(await api.getAlert());
await on("alert-updated", render);
