// A small DOM, enough to try the parts of the browser.
//
// The parts of the browser need a page with nodes that may be put in
// another order, so the nodes here know their parent and their brothers.
function makeElement(tag) {
  const element = {
    nodeType: 1,
    tagName: tag.toUpperCase(),
    childNodes: [],
    parentNode: null,
    attributes: [],
    listeners: {},
    get firstChild() {
      return this.childNodes[0] || null;
    },
    get nextSibling() {
      if (!this.parentNode) return null;
      const list = this.parentNode.childNodes;
      const index = list.indexOf(this);
      return index === -1 ? null : list[index + 1] || null;
    },
    appendChild(child) {
      if (child.parentNode) child.parentNode.removeChild(child);
      child.parentNode = this;
      this.childNodes.push(child);
      return child;
    },
    insertBefore(child, before) {
      if (child.parentNode) child.parentNode.removeChild(child);
      child.parentNode = this;
      const index = before ? this.childNodes.indexOf(before) : -1;
      if (index === -1) this.childNodes.push(child);
      else this.childNodes.splice(index, 0, child);
      return child;
    },
    removeChild(child) {
      const index = this.childNodes.indexOf(child);
      if (index !== -1) this.childNodes.splice(index, 1);
      child.parentNode = null;
      return child;
    },
    replaceChild(next, old) {
      const index = this.childNodes.indexOf(old);
      if (index === -1) return old;
      if (next.parentNode) next.parentNode.removeChild(next);
      next.parentNode = this;
      this.childNodes[index] = next;
      old.parentNode = null;
      return old;
    },
    replaceChildren(...nodes) {
      this.childNodes = [];
      for (const node of nodes) this.appendChild(node);
    },
    setAttribute(name, value) {
      for (const attribute of this.attributes) {
        if (attribute.name === name) {
          attribute.value = String(value);
          return;
        }
      }
      this.attributes.push({ name, value: String(value) });
    },
    getAttribute(name) {
      for (const attribute of this.attributes) {
        if (attribute.name === name) return attribute.value;
      }
      return null;
    },
    hasAttribute(name) {
      return this.getAttribute(name) !== null;
    },
    removeAttribute(name) {
      this.attributes = this.attributes.filter((attribute) => attribute.name !== name);
    },
    addEventListener(name, fn) {
      if (!this.listeners[name]) this.listeners[name] = [];
      this.listeners[name].push(fn);
    },
    removeEventListener(name, fn) {
      if (!this.listeners[name]) return;
      this.listeners[name] = this.listeners[name].filter((item) => item !== fn);
    },
    matches(selector) {
      if (selector.startsWith(".")) {
        return this.attributes.some(
          (attribute) =>
            attribute.name === "class" &&
            attribute.value.split(" ").includes(selector.slice(1))
        );
      }
      return this.tagName === selector.toUpperCase();
    },
    get textContent() {
      return this.childNodes.map((child) => child.textContent || "").join("");
    }
  };
  return element;
}

function makeText(value) {
  return {
    nodeType: 3,
    textContent: String(value),
    parentNode: null,
    childNodes: []
  };
}

const root = makeElement("div");
root.setAttribute("id", "app");

const document = {
  activeElement: null,
  title: "",
  createElement: (tag) => makeElement(tag),
  createTextNode: (value) => makeText(value),
  createDocumentFragment: () => {
    const fragment = makeElement("#fragment");
    fragment.nodeType = 11;
    return fragment;
  },
  querySelector: (selector) => (selector === "#app" ? root : null)
};

const view = {
  document,
  location: { href: "http://localhost/", pathname: "/", search: "" },
  history: {
    state: null,
    pushState(state, title, url) {
      this.state = state;
      view.location.pathname = url;
    },
    replaceState(state, title, url) {
      this.state = state;
      view.location.pathname = url;
    },
    back() {},
    forward() {},
    go() {}
  },
  addEventListener() {},
  localStorage: {
    store: {},
    getItem(key) {
      return key in this.store ? this.store[key] : null;
    },
    setItem(key, value) {
      this.store[key] = String(value);
    },
    removeItem(key) {
      delete this.store[key];
    },
    clear() {
      this.store = {};
    },
    key(index) {
      return Object.keys(this.store)[index] ?? null;
    }
  },
  sessionStorage: {
    store: {},
    getItem(key) {
      return key in this.store ? this.store[key] : null;
    },
    setItem(key, value) {
      this.store[key] = String(value);
    },
    removeItem(key) {
      delete this.store[key];
    },
    clear() {
      this.store = {};
    },
    key(index) {
      return Object.keys(this.store)[index] ?? null;
    }
  },
  matchMedia(query) {
    return { matches: true, media: query, addEventListener() {} };
  },
  requestAnimationFrame() {
    return 1;
  },
  cancelAnimationFrame() {},
  setTimeout,
  clearTimeout,
  setInterval,
  clearInterval,
  console
};
view.defaultView = view;
document.defaultView = view;

module.exports = { document, view, root, makeElement, makeText };
