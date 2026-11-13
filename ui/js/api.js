// Legătura cu partea Rust (comenzi și evenimente Tauri).
const tauri = window.__TAURI__;
const invoke = (command, args) => tauri.core.invoke(command, args);

export const api = {
  getState: () => invoke("get_state"),
  saveSettings: (settings) => invoke("save_settings", { settings }),
  chooseExcel: () => invoke("choose_excel"),
  refreshNow: () => invoke("refresh_now"),
  openExcel: () => invoke("open_excel_file"),
  testAlert: (serious = false) => invoke("test_alert", { serious }),
  getAlert: () => invoke("get_alert"),
  dismissAlert: () => invoke("dismiss_alert"),
  snoozeAlerts: (minutes) => invoke("snooze_alerts", { minutes }),
  showMain: () => invoke("show_main_window"),
  waConnect: () => invoke("wa_connect"),
  waDisconnect: () => invoke("wa_disconnect"),
  waSendTest: () => invoke("wa_send_test"),
  checkUpdate: () => invoke("check_update"),
  installUpdate: () => invoke("install_update"),
};

/** Ascultă un eveniment trimis de Rust (`snapshot`, `wa-status`, `wa-error`, `alert-updated`). */
export const on = (event, handler) => tauri.event.listen(event, (e) => handler(e.payload));
