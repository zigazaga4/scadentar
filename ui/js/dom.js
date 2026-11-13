// Mici ajutoare pentru construirea interfeței.

/** Creează un element: h("button", { class: "btn", onclick }, "Text", altElement). */
export function h(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (value == null || value === false) continue;
    if (key === "class") node.className = value;
    else if (key.startsWith("on") && typeof value === "function") node.addEventListener(key.slice(2), value);
    else if (key === "checked" || key === "value" || key === "disabled" || key === "selected") node[key] = value;
    else node.setAttribute(key, value === true ? "" : value);
  }
  for (const child of children.flat()) {
    if (child == null || child === false) continue;
    node.append(child.nodeType ? child : document.createTextNode(String(child)));
  }
  return node;
}

export const clear = (node) => node.replaceChildren();

/** Mesaj scurt în colțul ferestrei. `timeout: 0` = rămâne până e închis; `action` adaugă un buton. */
export function toast(message, { kind = "info", timeout = 4500, action = null } = {}) {
  let host = document.getElementById("toasts");
  if (!host) {
    host = h("div", { id: "toasts" });
    document.body.append(host);
  }
  const close = () => item.remove();
  const item = h(
    "div",
    { class: `toast ${kind}` },
    h("span", {}, message),
    action && h("button", { class: "btn", onclick: () => { close(); action.run(); } }, action.label),
    h("button", { class: "toast-x", title: "Închide", onclick: close }, "×"),
  );
  host.append(item);
  if (timeout) setTimeout(close, timeout);
}

/** Rulează `action` și arată eroarea ca mesaj dacă pică. */
export async function guarded(action) {
  try {
    return await action();
  } catch (err) {
    toast(String(err), { kind: "error", timeout: 8000 });
    return undefined;
  }
}

/** Execută `fn` după ce utilizatorul a oprit modificările pentru `ms` milisecunde. */
export function debounce(fn, ms = 500) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}
