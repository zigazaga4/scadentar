// Actualizări: caută o versiune nouă pe GitHub și o instalează la cerere (în fundal se instalează singură).
import { api } from "./api.js";
import { toast } from "./dom.js";

/** Caută o versiune nouă. Pe cale manuală spune și „ai deja cea mai nouă” sau de ce n-a mers. */
export async function checkForUpdates() {
  let update;
  try {
    update = await api.checkUpdate();
  } catch (err) {
    toast(`Nu am putut verifica actualizările: ${err}`, { kind: "warn" });
    return;
  }
  if (!update) {
    toast("Ai deja cea mai nouă versiune.", { kind: "ok" });
    return;
  }
  toast(`Versiune nouă disponibilă: ${update.version}.`, {
    timeout: 0,
    action: {
      label: "Actualizează acum",
      run: async () => {
        toast("Se descarcă și se instalează versiunea nouă; aplicația se repornește singură…", { timeout: 0 });
        try {
          await api.installUpdate();
        } catch (err) {
          toast(`Actualizarea nu a reușit: ${err}`, { kind: "error", timeout: 0 });
        }
      },
    },
  });
}
