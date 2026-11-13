// Pagina „Scadențe”: ce e depășit, ce scade azi și ce urmează.
import { clear, h } from "./dom.js";
import { URGENCY, dateRo, money, plural, sum, when } from "./fmt.js";

// Filtrele rămân la fel când pagina se redesenează (de exemplu după o recitire a fișierului).
const view = { text: "", showUpcoming: false };

const baseName = (path) => (path ?? "").split(/[\\/]/).pop();

function statCard(urgency, items) {
  const { label, cls } = URGENCY[urgency];
  return h(
    "div",
    { class: `stat ${cls}` },
    h("div", { class: "n" }, items.length),
    h("div", { class: "label" }, `${label} · ${plural(items.length, "factură", "facturi")}`),
    h("div", { class: "sum" }, items.length ? `${money(sum(items, (i) => i.rest))} de plătit` : "—"),
  );
}

function row(item, upcoming = false) {
  const kind = upcoming ? "upcoming" : item.urgency;
  return h(
    "tr",
    { class: kind },
    h("td", {}, dateRo(item.scadenta)),
    h("td", {}, h("span", { class: `badge ${kind}` }, when(item.days))),
    h("td", {}, item.foaie),
    h("td", {}, item.factura || h("span", { class: "muted" }, "—")),
    h("td", { class: "num" }, money(item.rest)),
    h("td", { class: "num muted" }, money(item.suma)),
    h("td", { class: "num muted" }, item.rand),
  );
}

function table(items, upcomingFrom) {
  const head = ["Scadența", "Termen", "Furnizor", "Factura / rata"].map((t) => h("th", {}, t));
  const heads = [...head, h("th", { class: "num" }, "Rest de plată"), h("th", { class: "num" }, "Total factură"), h("th", { class: "num" }, "Rând Excel")];
  return h("div", { class: "tablewrap" }, h("table", {}, h("thead", {}, h("tr", {}, heads)), h("tbody", {}, items.map((item, i) => row(item, i >= upcomingFrom)))));
}

function olderPanel(snapshot, settings) {
  if (!snapshot.older.length) return null;
  const total = sum(snapshot.older, (o) => o.count);
  return h(
    "details",
    { class: "older" },
    h("summary", {}, `Depășite de mai mult de ${settings.overdueMaxDays} de zile · ${plural(total, "factură", "facturi")} (nu mai sunt alertate)`),
    h("p", { class: "muted small" }, "Dacă sunt deja plătite, completează „Rest de plată” cu 0 în Excel și dispar de aici."),
    h(
      "div",
      { class: "tablewrap" },
      h(
        "table",
        {},
        h("thead", {}, h("tr", {}, [h("th", {}, "Furnizor"), h("th", { class: "num" }, "Facturi"), h("th", { class: "num" }, "Rest de plată"), h("th", {}, "Cea mai veche scadență")])),
        h(
          "tbody",
          {},
          snapshot.older.map((o) => h("tr", {}, h("td", {}, o.foaie), h("td", { class: "num" }, o.count), h("td", { class: "num" }, money(o.totalRest)), h("td", {}, dateRo(o.oldest)))),
        ),
      ),
    ),
  );
}

/** Desenează pagina în `root`. `actions`: { refresh, openExcel }. */
export function renderDashboard(root, { settings, snapshot }, actions) {
  clear(root);
  const alerts = snapshot.alerts;
  const by = (urgency) => alerts.filter((a) => a.urgency === urgency);

  const page = h("div", { class: "page" });
  if (snapshot.error) page.append(h("div", { class: "banner" }, snapshot.error));

  page.append(
    h(
      "div",
      { class: "row" },
      h("div", {}, h("strong", {}, baseName(snapshot.file)), h("span", { class: "muted small" }, snapshot.loadedAt ? `  ·  citit la ${snapshot.loadedAt}` : "  ·  se citește…")),
      h("div", { class: "grow" }),
      h("button", { class: "btn", onclick: actions.openExcel }, "Deschide în Excel"),
      h("button", { class: "btn primary", onclick: actions.refresh }, "↻ Verifică acum"),
    ),
  );

  if (alerts.length) {
    page.append(h("div", { class: "stats" }, statCard("overdue", by("overdue")), statCard("today", by("today")), statCard("soon", by("soon"))));
  } else {
    page.append(h("div", { class: "stats" }, h("div", { class: "stat none", style: "grid-column: 1 / -1" }, h("div", { class: "label" }, `Nicio scadență apropiată ✔  (în următoarele ${settings.warnDays} zile și până la ${settings.overdueMaxDays} de zile în urmă)`))));
  }

  const tableHost = h("div", {});
  const count = h("span", { class: "muted small" });
  const paint = () => {
    const q = view.text.trim().toLowerCase();
    const match = (i) => !q || `${i.foaie} ${i.factura}`.toLowerCase().includes(q);
    const shownAlerts = alerts.filter(match);
    const shownUpcoming = view.showUpcoming ? snapshot.upcoming.filter(match) : [];
    count.textContent = `${plural(shownAlerts.length, "scadență", "scadențe")}${view.showUpcoming ? ` + ${shownUpcoming.length} care urmează` : ""}`;
    clear(tableHost);
    tableHost.append(shownAlerts.length + shownUpcoming.length ? table([...shownAlerts, ...shownUpcoming], shownAlerts.length) : h("div", { class: "tablewrap" }, h("div", { class: "empty" }, q ? "Nimic nu se potrivește cu căutarea." : "Nimic de afișat.")));
  };

  page.append(
    h(
      "div",
      { class: "filters" },
      h("input", { type: "search", placeholder: "Caută furnizor sau factură…", value: view.text, oninput: (e) => { view.text = e.target.value; paint(); } }),
      h("label", { class: "check" }, h("input", { type: "checkbox", checked: view.showUpcoming, onchange: (e) => { view.showUpcoming = e.target.checked; paint(); } }), "Arată și ce urmează (până la 60 de zile)"),
      count,
    ),
    tableHost,
  );
  paint();

  const older = olderPanel(snapshot, settings);
  if (older) page.append(older);
  if (snapshot.withoutDueDate > 0) {
    page.append(h("p", { class: "muted small" }, `${plural(snapshot.withoutDueDate, "factură are", "facturi au")} rest de plată dar nu au dată scadentă în Excel, deci nu pot fi urmărite. Completează „Data scadenta” ca să fie incluse.`));
  }
  root.append(page);
}
