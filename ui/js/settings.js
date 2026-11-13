// Pagina „Setări”: fișierul Excel, când și cum ești avertizat, WhatsApp, furnizorii urmăriți.
import { api } from "./api.js";
import { clear, debounce, guarded, h } from "./dom.js";
import { plural } from "./fmt.js";
import { checkForUpdates } from "./updater.js";

const HOURS = Array.from({ length: 25 }, (_, i) => i);

/**
 * Listă de foi (furnizori) cu bife și căutare. `isOn(nume)` / `setOn(nume, bifat)` / `setAll(bifat)` leagă lista
 * de setări; `buttons`: [[text, bifat]] pentru butoanele „Toți” / „Niciunul”. Întoarce { el }.
 */
function sheetPicker({ sheets, isOn, setOn, setAll, buttons, save, change }) {
  const box = h("div", { class: "sheets" });
  const filter = h("input", { type: "search", placeholder: "Caută furnizor…", oninput: () => paint() });
  function paint() {
    const q = filter.value.trim().toLowerCase();
    clear(box);
    for (const s of sheets.filter((x) => x.name.toLowerCase().includes(q))) {
      box.append(
        h(
          "label",
          { class: "check" },
          h("input", { type: "checkbox", checked: isOn(s.name), onchange: change((t) => setOn(s.name, t.checked)) }),
          h("span", {}, s.name),
          h("span", { class: "count" }, [s.openItems ? plural(s.openItems, "deschisă", "deschise") : "", s.withoutDueDate ? `${s.withoutDueDate} fără scadență` : ""].filter(Boolean).join(" · ")),
        ),
      );
    }
    if (!box.children.length) box.append(h("div", { class: "empty" }, "Niciun furnizor."));
  }
  const buttonEls = buttons.map(([text, on]) => h("button", { class: "btn", onclick: () => { setAll(on); paint(); save(); } }, text));
  paint();
  return { el: h("div", {}, h("div", { class: "row" }, filter, buttonEls), box) };
}

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
      number("Verifică fișierul la fiecare … minute", "În plus, fișierul e recitit imediat ce îl salvezi.", "checkMinutes", 1, 1440),
      number("Ignoră un rest de plată sub", "Diferențe de câțiva bani nu contează.", "minRest", 0, 1000),
    ),
  );

  // — Când pleacă alerta: la interval sau la ore fixe —
  const intervalBlock = h(
    "div",
    {},
    number("Repetă alerta la fiecare … ore", "Pentru scadențele de azi și cele depășite, cât timp nu sunt achitate în Excel. Cele care urmează se reamintesc o dată pe zi.", "repeatHours", 1, 72),
    h("div", { class: "field" }, h("label", { class: "text" }, "Alertele apar doar între orele", h("div", { class: "hint" }, "În afara lor aștepți până dimineața.")), hourSelect("activeFrom"), "–", hourSelect("activeTo")),
  );
  const timesList = h("div", { class: "times" });
  const paintTimes = () => {
    clear(timesList);
    draft.alertTimes.forEach((time, i) => {
      timesList.append(
        h(
          "div",
          { class: "time-row" },
          h("input", { type: "time", value: time, oninput: change((t) => { if (t.value) draft.alertTimes[i] = t.value; }) }),
          h("button", { class: "btn", title: "Șterge ora", disabled: draft.alertTimes.length <= 1, onclick: () => { draft.alertTimes.splice(i, 1); paintTimes(); save(); } }, "×"),
        ),
      );
    });
  };
  const timesBlock = h(
    "div",
    {},
    h("div", { class: "muted small", style: "margin: 4px 0 8px" }, "În fiecare zi, la aceste ore, primești alerta cu tot ce e scadent (pe toate căile bifate mai jos). Între ele nu te deranjează nimeni. Dacă aplicația nu rula la ora respectivă, alerta vine la pornire."),
    timesList,
    h("button", { class: "btn", onclick: () => { draft.alertTimes.push("15:00"); paintTimes(); save(); } }, "+ Adaugă o oră"),
  );
  const showMode = () => {
    intervalBlock.hidden = draft.alertMode !== "interval";
    timesBlock.hidden = draft.alertMode !== "times";
  };
  const modeSelect = h(
    "select",
    { onchange: change((t) => { draft.alertMode = t.value; showMode(); }) },
    [["interval", "Imediat ce apare ceva, apoi reamintiri la interval"], ["times", "La o oră fixă, în fiecare zi"]].map(([value, text]) => h("option", { value, selected: value === draft.alertMode }, text)),
  );
  paintTimes();
  showMode();
  page.append(
    h(
      "section",
      { class: "card" },
      h("h2", {}, "Când pleacă alerta"),
      h("div", { class: "field" }, h("label", { class: "text" }, "Cum se hotărăște momentul"), modeSelect),
      intervalBlock,
      timesBlock,
    ),
  );

  // — Contracte importante —
  const contractSheets = store.snapshot.sheets.filter((s) => s.hasTable).sort((a, b) => a.name.localeCompare(b.name, "ro"));
  const criticalPicker = sheetPicker({
    sheets: contractSheets,
    isOn: (name) => draft.criticalSheets.includes(name),
    setOn: (name, on) => { draft.criticalSheets = draft.criticalSheets.filter((n) => n !== name); if (on) draft.criticalSheets.push(name); },
    setAll: (on) => { draft.criticalSheets = on ? contractSheets.map((s) => s.name) : []; },
    buttons: [["Niciunul", false]],
    save,
    change,
  });
  page.append(
    h(
      "section",
      { class: "card" },
      h("h2", {}, "Contracte importante (mașini, leasing)"),
      h("p", { class: "lead" }, "Bifează foile din Excel pentru care o scadență depășită e o problemă serioasă, de exemplu ratele de leasing ale mașinilor. Pentru ele, când data scadentă trece, primești o alertă serioasă: ecran roșu, sunet mai lung și un mesaj WhatsApp cu avertizare, repetat mai des. Cele neselectate rămân alerte obișnuite."),
      number("Repetă alerta serioasă la fiecare … ore", "Doar în modul „imediat ce apare ceva”; la ore fixe vine la fiecare oră aleasă.", "criticalRepeatHours", 1, 72),
      criticalPicker.el,
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
      h(
        "div",
        { class: "row", style: "margin-top:10px" },
        h("button", { class: "btn primary", onclick: () => guarded(() => api.testAlert(false)) }, "Testează alerta"),
        h("button", { class: "btn danger", onclick: () => guarded(() => api.testAlert(true)) }, "Testează alerta serioasă"),
        h("span", { class: "muted small" }, "Cu datele reale, sau cu un exemplu dacă nu e nimic scadent."),
      ),
    ),
  );

  page.append(ctx.waCard.el);

  const trackedPicker = sheetPicker({
    sheets: contractSheets,
    isOn: (name) => !draft.excludedSheets.includes(name),
    setOn: (name, on) => { draft.excludedSheets = draft.excludedSheets.filter((n) => n !== name); if (!on) draft.excludedSheets.push(name); },
    setAll: (on) => { draft.excludedSheets = on ? [] : contractSheets.map((s) => s.name); },
    buttons: [["Toți", true], ["Niciunul", false]],
    save,
    change,
  });
  page.append(
    h(
      "section",
      { class: "card" },
      h("h2", {}, "Furnizori urmăriți"),
      h("p", { class: "lead" }, "Fiecare foaie din Excel e un furnizor. Debifează-i pe cei pe care nu vrei să-i urmărești. Furnizorii noi apar aici și sunt urmăriți automat."),
      trackedPicker.el,
    ),
  );

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
