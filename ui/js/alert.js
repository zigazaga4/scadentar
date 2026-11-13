// Fereastra de alertă: apare deasupra tuturor ferestrelor când sunt scadențe.
import { api, on } from "./api.js";
import { clear, guarded, h } from "./dom.js";
import { URGENCY, dateRo, money, plural, when } from "./fmt.js";

const root = document.getElementById("root");

function section(urgency, groups) {
  if (!groups.length) return null;
  const { label, cls } = URGENCY[urgency];
  return h(
    "div",
    {},
    h("h3", { class: cls }, `${label} (${groups.length})`),
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
  root.append(
    h("div", { class: `alert-head ${empty ? "calm" : ""}` }, h("h1", {}, empty ? "Nicio scadență acum" : "Scadențe de plată"), h("p", {}, empty ? "Totul este la zi." : `${digest.title} · ${digest.date}`)),
    h("div", { class: "alert-body" }, empty ? null : ["overdue", "today", "soon"].map((u) => section(u, digest.groups.filter((g) => g.urgency === u)))),
    h(
      "div",
      { class: "alert-foot" },
      h("button", { class: "btn primary", onclick: () => guarded(() => api.dismissAlert()) }, "Am văzut"),
      h("button", { class: "btn", onclick: () => guarded(() => api.snoozeAlerts(60)) }, "Amână 1 oră"),
      h("button", { class: "btn", onclick: () => guarded(() => api.snoozeAlerts(240)) }, "Amână 4 ore"),
      h("button", { class: "btn", onclick: async () => { await guarded(() => api.showMain()); } }, "Deschide aplicația"),
      h("div", { class: "grow" }),
      empty ? null : h("span", { class: "alert-total" }, `Total: ${money(digest.totalRest)}`),
    ),
  );
}

render(await api.getAlert());
await on("alert-updated", render);
