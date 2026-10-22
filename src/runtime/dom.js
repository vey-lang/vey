// The markup and the browser.
function veyDom(ctx) {
  var doc = ctx.doc;
  var flatten = ctx.flatten;
  var ownProps = ctx.ownProps;
  var cssPropertyName = ctx.cssPropertyName;
  var domStyleName = ctx.domStyleName;
  var veyError = ctx.veyError;
  var escapeHtml = ctx.escapeHtml;

  var VOID_TAGS = Object.assign(Object.create(null), {
    area: 1, base: 1, br: 1, col: 1, embed: 1, hr: 1, img: 1, input: 1,
    link: 1, meta: 1, param: 1, source: 1, track: 1, wbr: 1
  });

  function h(tag, props, children, args) {
    var kids = flatten(children || []);
    var attrs = props || {};

    if (typeof tag === "function") {
      // Components receive a marked props object so that a plain positional
      // call (`Card("title")`) stays distinguishable from a named one
      // (`Card title="title"`), even when the first argument is an object.
      var named = Object.assign(Object.create(null), attrs);
      // The marker is hidden, so that it does not show up in the props.
      Object.defineProperty(named, "__veyProps", { value: true, enumerable: false });
      if (kids.length) named.children = kids;
      var callArgs = [named];
      if (args) {
        for (var i = 0; i < args.length; i++) callArgs.push(args[i]);
      }
      return tag.apply(null, callArgs);
    }

    if (doc) {
      var el = doc.createElement(tag);
      applyProps(el, attrs);
      for (var j = 0; j < kids.length; j++) el.appendChild(toDom(kids[j]));
      return el;
    }

    return { __vey: true, tag: tag, props: attrs, children: kids };
  }

  function applyProps(el, attrs) {
    var props = ownProps(attrs);
    for (var key in props) {
      var value = props[key];
      if (value === null || value === undefined || value === false) continue;
      if (key.indexOf("on") === 0 && typeof value === "function") {
        var name = key.slice(2).toLowerCase();
        el.addEventListener(name, value);
        // The listener is kept, so that a page that is changed in place can
        // move the listener of the new node to the node that stays.
        if (!el.__veyListeners) el.__veyListeners = {};
        el.__veyListeners[name] = value;
        continue;
      }
      if (key === "key") {
        // The name of an item of a list, so that the list may be changed and
        // put in another order without trouble.
        Object.defineProperty(el, "__veyKey", {
          value: String(value),
          writable: true,
          enumerable: false,
          configurable: true
        });
        continue;
      }
      if (key === "ref") {
        // A reference to the node, so that the program may hold it.
        Object.defineProperty(el, "__veyRef", {
          value: value,
          writable: true,
          enumerable: false,
          configurable: true
        });
        if (value && typeof value.set === "function") value.set(el);
        continue;
      }
      if (key === "class") {
        el.setAttribute("class", value);
        continue;
      }
      if (key === "style" && typeof value === "object") {
        var styles = ownProps(value);
        for (var name in styles) {
          var prop = domStyleName(name);
          if (prop.indexOf("--") === 0) el.style.setProperty(prop, styles[name]);
          else el.style[prop] = styles[name];
        }
        continue;
      }
      if (key === "value" || key === "checked") {
        el[key] = value;
        continue;
      }
      el.setAttribute(key, value === true ? "" : String(value));
    }
  }

  function toDom(value) {
    if (value === null || value === undefined) return doc.createTextNode("");
    if (typeof value === "string" || typeof value === "number") {
      return doc.createTextNode(String(value));
    }
    if (Array.isArray(value)) {
      var fragment = doc.createDocumentFragment();
      for (var i = 0; i < value.length; i++) fragment.appendChild(toDom(value[i]));
      return fragment;
    }
    if (value.nodeType) return value;
    if (value.__veyRaw) {
      var template = doc.createElement("template");
      template.innerHTML = String(value.value);
      return template.content;
    }
    if (value.__vey) {
      var el = doc.createElement(value.tag);
      applyProps(el, value.props || {});
      var children = value.children || [];
      for (var j = 0; j < children.length; j++) {
        el.appendChild(toDom(children[j]));
      }
      return el;
    }
    return doc.createTextNode(String(value));
  }

  function html(node) {
    if (node === null || node === undefined || node === false || node === true) return "";
    if (Array.isArray(node)) {
      var out = "";
      for (var i = 0; i < node.length; i++) out += html(node[i]);
      return out;
    }
    if (node.__veyRaw) return String(node.value);
    if (typeof node === "string") return escapeHtml(node);
    if (typeof node === "number") return String(node);
    if (node.nodeType) return node.outerHTML || "";
    if (!node.__vey) return escapeHtml(node);

    var attrs = "";
    var props = ownProps(node.props || {});
    for (var key in props) {
      var value = props[key];
      if (value === null || value === undefined || value === false) continue;
      if (key.indexOf("on") === 0 || typeof value === "function") continue;
      // The key of an item and the reference to a node are of the program,
      // not of the page.
      if (key === "key" || key === "ref") continue;
      if (key === "style" && typeof value === "object") {
        var styles = ownProps(value);
        var css = "";
        for (var name in styles) css += cssPropertyName(name) + ": " + styles[name] + "; ";
        attrs += " style=\"" + escapeHtml(css) + "\"";
        continue;
      }
      if (value === true) {
        attrs += " " + key;
      } else {
        attrs += " " + key + "=\"" + escapeHtml(value) + "\"";
      }
    }

    if (VOID_TAGS[node.tag]) return "<" + node.tag + attrs + ">";
    var children = "";
    var inner = node.children || [];
    for (var j = 0; j < inner.length; j++) children += html(inner[j]);
    return "<" + node.tag + attrs + ">" + children + "</" + node.tag + ">";
  }

  function raw(value) {
    return {
      __veyRaw: true,
      value: value === null || value === undefined ? "" : String(value)
    };
  }

  // `bind` is the short way to put a page on the screen. It goes through
  // `mount`, so that the routes of the browser and the changes of a signal
  // know the page as well.
  function bind(target, node) {
    if (!doc) {
      throw veyError(
        "bind() is only available in the browser. Open the page with `vey dev` " +
          "or build it with `vey web`."
      );
    }
    if (typeof ctx.mount === "function") {
      return ctx.mount(target, node).element;
    }
    var el = typeof target === "string" ? doc.querySelector(target) : target;
    if (!el) throw new Error("element not found: " + target);
    var nodes = flatten([node]).map(toDom);
    el.replaceChildren.apply(el, nodes);
    return el;
  }

  // The title of the page, which the tab of the browser shows.
  function title(text) {
    if (!doc) {
      throw veyError("title() is only available in the browser.");
    }
    doc.title = String(text);
  }

  // The clipboard of the browser, so that a command may be copied.
  var clipboard = {
    write: function (text) {
      var view = doc && doc.defaultView;
      var board = view && view.navigator && view.navigator.clipboard;
      if (board && board.writeText) {
        return board.writeText(String(text));
      }
      // A browser that has no clipboard of its own still copies with the old
      // command of the page, as long as the text is in a field of its own.
      if (!doc || !doc.body) {
        return Promise.reject(veyError("the clipboard is not available"));
      }
      var area = doc.createElement("textarea");
      area.value = String(text);
      area.setAttribute("readonly", "");
      area.style.position = "fixed";
      area.style.top = "0";
      area.style.opacity = "0";
      doc.body.appendChild(area);
      area.select();
      var copied = false;
      try {
        copied = !!doc.execCommand("copy");
      } catch (err) {
        copied = false;
      }
      doc.body.removeChild(area);
      if (!copied) {
        return Promise.reject(veyError("the clipboard is not available"));
      }
      return Promise.resolve();
    }
  };

  function cls() {
    var parts = [];
    for (var i = 0; i < arguments.length; i++) {
      var value = arguments[i];
      if (value) parts.push(String(value));
    }
    return parts.join(" ");
  }

  ctx.h = h;
  ctx.html = html;
  ctx.raw = raw;
  ctx.bind = bind;
  ctx.cls = cls;
  ctx.clipboard = clipboard;
  ctx.title = title;
  ctx.toDom = toDom;

  // The name of an item of a list, so that the list may be changed and put
  // in another order without trouble.
  function withKey(nodes, key) {
    var list = Array.isArray(nodes) ? nodes : [nodes];
    for (var i = 0; i < list.length; i++) {
      var node = list[i];
      if (!node || typeof node !== "object") continue;
      try {
        Object.defineProperty(node, "__veyKey", {
          value: String(key),
          writable: true,
          enumerable: false,
          configurable: true
        });
      } catch (err) {
        node.__veyKey = String(key);
      }
    }
    return list;
  }

  return {
    // The builder of the markup, which the compiler calls and a program
    // does not.
    __h: h,
    withKey: withKey,
    html: html,
    raw: raw,
    bind: bind,
    cls: cls,
    clipboard: clipboard,
    title: title,
  };
}
