// Fereastra principală: prima pornire, pagina de scadențe și pagina de setări.
import { api, on } from "./api.js";
import { clear, guarded, h } from "./dom.js";
import { renderDashboard } from "./dashboard.js";
import { renderSettings } from "./settings.js";
import { createWhatsappCard } from "./whatsapp.js";

const main = document.getElementById("main");
const tabsEl = document.getElementById("tabs");
const chipsEl = document.getElementById("chips");

const store = { settings: null, snapshot: null, wa: { state: "stopped" }, waError: null, version: "", dev: false, tab: "scadente" };
const waCard = createWhatsappCard();

const TABS = [
  ["scadente", "Scadențe"],
  ["setari", "Setări"],
];

function apply(boot) {
  Object.assign(store, { settings: boot.settings, snapshot: boot.snapshot, wa: boot.wa, waError: boot.waError, version: boot.version, dev: boot.dev });
}

function paintChrome() {
  clear(tabsEl);
  clear(chipsEl);
  if (!store.settings.excelPath) return;
  for (const [id, label] of TABS) {
    tabsEl.append(h("button", { class: `tab ${store.tab === id ? "active" : ""}`, onclick: () => { store.tab = id; render(); } }, label));
  }
  const { snapshot, wa } = store;
  const excelChip = snapshot.error ? ["warn", "Excel: nu pot citi fișierul"] : snapshot.loadedAt ? ["ok", `Excel citit la ${snapshot.loadedAt.split(" ")[1]}`] : ["", "Excel: se citește…"];
  const waChip = wa.state === "connected" ? ["ok", "WhatsApp conectat"] : wa.state === "qr" ? ["warn", "WhatsApp: scanează codul"] : ["", "WhatsApp neconectat"];
  for (const [cls, text] of [excelChip, waChip]) chipsEl.append(h("span", { class: `chip ${cls}` }, text));
}

function welcome() {
  clear(main);
  main.append(
    h(
      "div",
      { class: "welcome" },
      h("h1", {}, "Bine ai venit la Scadențar"),
      h("p", {}, "Alege fișierul Excel cu facturile și ratele. Aplicația îl citește, rămâne în fundal și te avertizează înainte să expire o scadență: cu o notificare Windows, o fereastră care nu poate fi ratată și, dacă vrei, un mesaj pe WhatsApp."),
      h("button", { class: "btn primary big", onclick: chooseExcel }, "Alege fișierul Excel"),
      h("p", { class: "small" }, "Fișierul este doar citit, niciodată modificat."),
    ),
  );
}

async function chooseExcel() {
  const boot = await guarded(() => api.chooseExcel());
  if (!boot) return;
  apply(boot);
  store.tab = "scadente";
  render();
}

async function refresh() {
  const snapshot = await guarded(() => api.refreshNow());
  if (snapshot) {
    store.snapshot = snapshot;
    render();
  }
}

function render() {
  const scroll = main.scrollTop; // pagina rămâne unde era când se actualizează datele
  paintChrome();
  if (!store.settings.excelPath) return welcome();
  if (store.tab === "setari") {
    renderSettings(main, {
      store,
      waCard,
      onChooseExcel: chooseExcel,
      // Setările s-au salvat: se păstrează starea nouă, fără să redesenăm formularul sub degetele omului.
      onSaved: (boot) => { apply(boot); paintChrome(); },
    });
    waCard.update(store.wa, store.waError);
  } else {
    renderDashboard(main, store, { refresh, openExcel: () => guarded(() => api.openExcel()) });
  }
  main.scrollTop = scroll;
}

apply(await api.getState());
render();

await on("snapshot", (snapshot) => {
  store.snapshot = snapshot;
  if (store.tab === "scadente") render();
  else paintChrome();
});
await on("wa-status", (wa) => {
  store.wa = wa;
  waCard.update(store.wa, store.waError);
  paintChrome();
});
await on("wa-error", (message) => {
  store.waError = message;
  waCard.update(store.wa, store.waError);
});
