// Pagina „Setări”: fișierul Excel, când și cum ești avertizat, WhatsApp, furnizorii urmăriți.
import { api } from "./api.js";
import { clear, debounce, guarded, h } from "./dom.js";
import { plural } from "./fmt.js";
import { checkForUpdates } from "./updater.js";

const HOURS = Array.from({ length: 25 }, (_, i) => i);

/**
 * Desenează pagina în `root`. `ctx`: { store, waCard, onSaved(boot), onChooseExcel() }.
 * Câmpurile se salvează singure la scurt timp după fiecare modificare.
 */
export function renderSettings(root, ctx) {
  clear(root);
  const { store } = ctx;
  const draft = structuredClone(store.settings);

  const save = debounce(async () => {
    const boot = await guarded(() => api.saveSettings(draft));
    if (boot) ctx.onSaved(boot);
  }, 600);
  const change = (apply) => (e) => {
    apply(e.target);
    save();
  };

  const number = (label, hint, key, min, max) =>
    h(
      "div",
      { class: "field" },
      h("label", { class: "text" }, label, hint && h("div", { class: "hint" }, hint)),
      h("input", { type: "number", min, max, value: draft[key], oninput: change((t) => { draft[key] = Number(t.value); }) }),
    );
  const hourSelect = (key) =>
    h("select", { onchange: change((t) => { draft[key] = Number(t.value); }) }, HOURS.map((hour) => h("option", { value: hour, selected: hour === draft[key] }, `${String(hour).padStart(2, "0")}:00`)));
  const check = (label, key, hint) =>
    h("label", { class: "check" }, h("input", { type: "checkbox", checked: draft[key], onchange: change((t) => { draft[key] = t.checked; }) }), h("span", {}, label, hint && h("span", { class: "muted small" }, `  ${hint}`)));

  const page = h("div", { class: "page" });

  page.append(
    h(
      "section",
      { class: "card" },
      h("h2", {}, "Fișierul Excel"),
      h("p", { class: "lead" }, "Aplicația doar citește fișierul; nu îl modifică. Îl poți ține deschis în Excel. Când îl salvezi, modificările sunt preluate în cel mult 30 de secunde. Foile și rândurile noi sunt găsite singure."),
      h("div", { class: "path" }, store.settings.excelPath ?? "—"),
      h("div", { class: "row", style: "margin-top:10px" }, h("button", { class: "btn", onclick: ctx.onChooseExcel }, "Alege alt fișier"), h("button", { class: "btn", onclick: () => guarded(() => api.openExcel()) }, "Deschide în Excel")),
    ),
  );

  page.append(
    h(
      "section",
      { class: "card" },
      h("h2", {}, "Când ești avertizat"),
      number("Cu câte zile înainte de scadență", "Facturile care scad în această perioadă intră în alertă.", "warnDays", 0, 90),
      number("Cât timp după scadență mai ești avertizat", "Zile. După aceea factura trece la „Depășite de mult” și nu mai deranjează. 0 = fără limită.", "overdueMaxDays", 0, 3650),
      number("Repetă alerta la fiecare … ore", "Pentru scadențele de azi și cele depășite, cât timp nu sunt achitate în Excel. Cele care urmează se reamintesc o dată pe zi.", "repeatHours", 1, 72),
      number("Verifică fișierul la fiecare … minute", "În plus, fișierul e recitit imediat ce îl salvezi.", "checkMinutes", 1, 1440),
      number("Ignoră un rest de plată sub", "Diferențe de câțiva bani nu contează.", "minRest", 0, 1000),
      h("div", { class: "field" }, h("label", { class: "text" }, "Alertele apar doar între orele", h("div", { class: "hint" }, "În afara lor aștepți până dimineața.")), hourSelect("activeFrom"), "–", hourSelect("activeTo")),
    ),
  );

  page.append(
    h(
      "section",
      { class: "card" },
      h("h2", {}, "Cum primești alertele"),
      check("Notificare Windows", "notifyToast"),
      check("Fereastră deasupra tuturor celorlalte", "notifyPopup", "— rămâne pe ecran până o închizi sau o amâni"),
      check("Sunet", "notifySound"),
      check("Mesaj pe WhatsApp", "notifyWhatsapp", "— după ce te conectezi mai jos"),
      h("div", { class: "row", style: "margin-top:10px" }, h("button", { class: "btn primary", onclick: () => guarded(() => api.testAlert()) }, "Testează alerta"), h("span", { class: "muted small" }, "Folosește datele reale (sau un exemplu, dacă nu e nimic scadent).")),
    ),
  );

  page.append(ctx.waCard.el);

  const sheetsBox = h("div", { class: "sheets" });
  const filter = h("input", { type: "search", placeholder: "Caută furnizor…", oninput: () => paintSheets() });
  const sheets = store.snapshot.sheets.filter((s) => s.hasTable).sort((a, b) => a.name.localeCompare(b.name, "ro"));
  function paintSheets() {
    const q = filter.value.trim().toLowerCase();
    clear(sheetsBox);
    for (const s of sheets.filter((x) => x.name.toLowerCase().includes(q))) {
      const tracked = !draft.excludedSheets.includes(s.name);
      sheetsBox.append(
        h(
          "label",
          { class: "check" },
          h("input", {
            type: "checkbox",
            checked: tracked,
            onchange: change((t) => {
              draft.excludedSheets = draft.excludedSheets.filter((n) => n !== s.name);
              if (!t.checked) draft.excludedSheets.push(s.name);
            }),
          }),
          h("span", {}, s.name),
          h("span", { class: "count" }, [s.openItems ? plural(s.openItems, "deschisă", "deschise") : "", s.withoutDueDate ? `${s.withoutDueDate} fără scadență` : ""].filter(Boolean).join(" · ")),
        ),
      );
    }
    if (!sheetsBox.children.length) sheetsBox.append(h("div", { class: "empty" }, "Niciun furnizor."));
  }
  const setAll = (tracked) => () => {
    draft.excludedSheets = tracked ? [] : sheets.map((s) => s.name);
    paintSheets();
    save();
  };
  page.append(
    h(
      "section",
      { class: "card" },
      h("h2", {}, "Furnizori urmăriți"),
      h("p", { class: "lead" }, "Fiecare foaie din Excel e un furnizor. Debifează-i pe cei pe care nu vrei să-i urmărești. Furnizorii noi apar aici și sunt urmăriți automat."),
      h("div", { class: "row" }, filter, h("button", { class: "btn", onclick: setAll(true) }, "Toți"), h("button", { class: "btn", onclick: setAll(false) }, "Niciunul")),
      sheetsBox,
    ),
  );
  paintSheets();

  const version = h("span", { class: "muted small" }, `Versiunea ${store.version}${store.dev ? " (dezvoltare)" : ""}`);
  page.append(
    h(
      "section",
      { class: "card" },
      h("h2", {}, "Aplicația"),
      check("Pornește singură când se deschide Windows-ul", "autostart", store.dev ? "(în versiunea de dezvoltare nu are efect)" : "— rămâne în fundal, lângă ceas"),
      h("div", { class: "row", style: "margin-top:8px" }, h("button", { class: "btn", onclick: checkForUpdates }, "Caută actualizări"), version),
    ),
  );
  root.append(page);
}
