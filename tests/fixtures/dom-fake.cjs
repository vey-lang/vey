// A small DOM, so that the tests of a page may run without a browser.
//
// The nodes know the one that comes after them and the one that holds
// them, so that the difference between two pages may walk through them.

function makeElement(tag) {
  const el = {
    nodeType: 1,
    tagName: tag.toUpperCase(),
    children: [],
    attributes: {},
    style: {},
    listeners: {},
    value: "",
    checked: false,
    appendChild(child) {
      // A node that is already in the page is moved to the end of it.
      const index = this.children.indexOf(child);
      if (index !== -1) this.children.splice(index, 1);
      this.children.push(child);
      link(this);
      return child;
    },
    insertBefore(child, before) {
      // A node that is already in the page is moved to the place it is
      // given, the way a real page does it.
      const index = this.children.indexOf(child);
      if (index !== -1) this.children.splice(index, 1);
      const at = before ? this.children.indexOf(before) : -1;
      if (at === -1) this.children.push(child);
      else this.children.splice(at, 0, child);
      link(this);
      return child;
    },
    removeChild(child) {
      const index = this.children.indexOf(child);
      if (index !== -1) this.children.splice(index, 1);
      // The child is not in the page any more, so it has no place in it.
      unlink(child);
      link(this);
      return child;
    },
    replaceChild(next, old) {
      const index = this.children.indexOf(old);
      if (index === -1) this.children.push(next);
      else this.children[index] = next;
      // The old child is not in the page any more.
      unlink(old);
      link(this);
      return old;
    },
    replaceChildren(...nodes) {
      for (const child of this.children) unlink(child);
      this.children = nodes;
      link(this);
    },
    setAttribute(name, value) {
      this.attributes[name] = String(value);
    },
    getAttribute(name) {
      return this.attributes[name];
    },
    hasAttribute(name) {
      return name in this.attributes;
    },
    removeAttribute(name) {
      delete this.attributes[name];
    },
    addEventListener(name, fn) {
      if (!this.listeners[name]) this.listeners[name] = [];
      this.listeners[name].push(fn);
    },
    removeEventListener(name, fn) {
      const list = this.listeners[name] || [];
      const index = list.indexOf(fn);
      if (index !== -1) list.splice(index, 1);
    },
    focus() {
      document.activeElement = this;
    },
    get textContent() {
      if (this.__veyText !== undefined) return this.__veyText;
      return this.children.map((child) => child.textContent ?? "").join("");
    },
    set textContent(value) {
      // The text of the node is written, and the children are taken away.
      this.__veyText = String(value);
      this.children = [];
    },
    get outerHTML() {
      const attrs = Object.entries(this.attributes)
        .map(([key, value]) => ` ${key}="${value}"`)
        .join("");
      return `<${this.tagName.toLowerCase()}${attrs}>${this.textContent}</${this.tagName.toLowerCase()}>`;
    }
  };
  Object.defineProperty(el, "firstChild", {
    get() {
      return el.children[0] || null;
    }
  });
  Object.defineProperty(el, "lastChild", {
    get() {
      return el.children[el.children.length - 1] || null;
    }
  });
  return el;
}

function link(parent) {
  parent.children.forEach((child, index) => {
    Object.defineProperty(child, "nextSibling", {
      get() {
        return parent.children[index + 1] || null;
      },
      configurable: true
    });
    Object.defineProperty(child, "parentNode", {
      get() {
        return parent;
      },
      configurable: true
    });
  });
}

// A node that is not in the page any more has no place in it, the way a
// node of a real page has none.
function unlink(child) {
  if (!child || typeof child !== "object") return;
  Object.defineProperty(child, "parentNode", {
    get() {
      return null;
    },
    configurable: true
  });
  Object.defineProperty(child, "nextSibling", {
    get() {
      return null;
    },
    configurable: true
  });
}

const root = makeElement("div");
// A place of the page that is outside the page that is written, so that a
// portal may be written in it.
const side = makeElement("div");
// The address of the page, so that the routes of the browser may be shown.
const location = { pathname: "/", search: "", href: "http://localhost/" };
const head = makeElement("head");
const document = {
  head,
  createElement: makeElement,
  createTextNode(text) {
    return { nodeType: 3, textContent: String(text) };
  },
  createDocumentFragment() {
    const fragment = makeElement("#fragment");
    fragment.nodeType = 11;
    return fragment;
  },
  querySelector(selector) {
    if (selector === "#app") return root;
    if (selector === "#side") return side;
    return null;
  },
  activeElement: null,
  defaultView: { location }
};

global.document = document;
global.location = location;
global.window = {
  document,
  console,
  location,
  addEventListener() {}
};

// The node of the page, and the way to find a node in it.
function find(node, tag) {
  if (node.tagName === tag.toUpperCase()) return node;
  for (const child of node.children || []) {
    const found = find(child, tag);
    if (found) return found;
  }
  return null;
}

module.exports = { root, side, document, makeElement, find, waitFor };

// Wait until the page is the way it should be.
//
// The tests do not wait for a fixed time, because a machine that is busy
// needs more time. They ask the page again and again, until it is right
// or the time is over.
function waitFor(test, what, done) {
  const limit = Date.now() + 5000;
  (function ask() {
    let ok = false;
    try {
      ok = test();
    } catch (err) {
      ok = false;
    }
    if (ok) {
      done();
      return;
    }
    if (Date.now() > limit) {
      throw new Error("the page is not right: " + what());
    }
    setTimeout(ask, 20);
  })();
}
