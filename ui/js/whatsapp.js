// Cardul WhatsApp: starea conexiunii, codul QR de scanat, mesajul de probă.
import { api } from "./api.js";
import { clear, guarded, h, toast } from "./dom.js";

const HOW_TO_SCAN = "Pe telefon: WhatsApp → Setări → Dispozitive conectate → Conectează un dispozitiv, apoi scanează codul.";

/** Întoarce { el, update(status, error) }; `update` se apelează la fiecare schimbare de stare. */
export function createWhatsappCard() {
  const body = h("div", {});
  const el = h(
    "section",
    { class: "card" },
    h("h2", {}, "WhatsApp"),
    h("p", { class: "lead" }, "Alertele pot ajunge și pe WhatsApp, în chatul „Mesaj către tine” (către propriul număr). Te conectezi o singură dată, scanând un cod QR."),
    body,
    h("p", { class: "muted small", style: "margin-top:12px" }, "Conexiunea folosește o metodă neoficială (nu API-ul oficial WhatsApp). Pentru câteva mesaje pe zi, trimise chiar către tine, riscul ca WhatsApp să limiteze contul este foarte mic."),
  );

  let busy = false;
  const run = (action) => async () => {
    if (busy) return;
    busy = true;
    await guarded(action);
    busy = false;
  };
  const connect = run(() => api.waConnect());
  const disconnect = run(async () => {
    if (confirm("Deconectezi WhatsApp de la Scadențar? Va trebui să scanezi din nou codul QR ca să-l folosești.")) await api.waDisconnect();
  });
  const sendTest = run(async () => {
    await api.waSendTest();
    toast("Mesajul de probă a plecat. Caută-l în WhatsApp, în chatul „Mesaj către tine”.", { kind: "ok", timeout: 7000 });
  });

  const status = (dot, text) => h("div", { class: "row", style: "margin-bottom:10px" }, h("span", { class: `dot ${dot}` }), h("strong", {}, text));

  function update(wa, waError) {
    clear(body);
    switch (wa.state) {
      case "starting":
        body.append(status("warn", "Se conectează…"));
        break;
      case "qr":
        body.append(
          status("warn", "Scanează codul QR cu telefonul"),
          h("div", { class: "qr" }, h("img", { src: wa.image, alt: "Cod QR WhatsApp" })),
          h("p", { class: "muted small", style: "margin-top:10px;max-width:420px" }, `${HOW_TO_SCAN} Codul se reînnoiește singur.`),
          h("div", { class: "row", style: "margin-top:10px" }, h("button", { class: "btn", onclick: disconnect }, "Renunță")),
        );
        break;
      case "connected":
        body.append(
          status("ok", `Conectat${wa.phone ? ` · ${wa.phone}` : ""}`),
          h("div", { class: "row" }, h("button", { class: "btn primary", onclick: sendTest }, "Trimite un mesaj de probă"), h("button", { class: "btn danger", onclick: disconnect }, "Deconectează")),
        );
        break;
      case "error":
        body.append(status("bad", "Nu s-a putut conecta"), h("p", { class: "muted small" }, wa.message), h("div", { class: "row", style: "margin-top:10px" }, h("button", { class: "btn primary", onclick: connect }, "Încearcă din nou")));
        break;
      case "loggedOut":
        body.append(status("bad", "Dispozitivul a fost deconectat din WhatsApp"), h("div", { class: "row" }, h("button", { class: "btn primary", onclick: connect }, "Conectează din nou")));
        break;
      default:
        body.append(status("", "Neconectat"), h("div", { class: "row" }, h("button", { class: "btn primary", onclick: connect }, "Conectează WhatsApp")));
    }
    if (waError) body.append(h("div", { class: "banner", style: "margin-top:12px" }, `Ultimul mesaj nu a putut fi trimis: ${waError}`));
  }

  update({ state: "stopped" }, null);
  return { el, update };
}
