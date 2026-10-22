// The markup and the browser.
function veyDom(ctx) {
  var doc = ctx.doc;
  var flatten = ctx.flatten;
  var ownProps = ctx.ownProps;
  var cssPropertyName = ctx.cssPropertyName;
  var domStyleName = ctx.domStyleName;
  var veyError = ctx.veyError;
  var escapeHtml = ctx.escapeHtml;
  var log = ctx.log;

  var VOID_TAGS = Object.assign(Object.create(null), {
    area: 1, base: 1, br: 1, col: 1, embed: 1, hr: 1, img: 1, input: 1,
    link: 1, meta: 1, param: 1, source: 1, track: 1, wbr: 1
  });

  // The tags of SVG, which live in a space of their own.
  //
  // A node of SVG is made with the space of SVG, or the browser does not
  // draw it.
  var SVG_NS = "http://www.w3.org/2000/svg";
  var SVG_TAGS = (
    "animate animateMotion animateTransform circle clipPath defs desc ellipse " +
    "feBlend feColorMatrix feComponentTransfer feComposite feConvolveMatrix " +
    "feDiffuseLighting feDisplacementMap feDistantLight feDropShadow feFlood " +
    "feFuncA feFuncB feFuncG feFuncR feGaussianBlur feImage feMerge feMergeNode " +
    "feMorphology feOffset fePointLight feSpecularLighting feSpotLight feTile " +
    "feTurbulence filter foreignObject g image line linearGradient marker " +
    "mask metadata mpath path pattern polygon polyline radialGradient rect " +
    "set stop svg switch symbol text textPath tspan use view"
  ).split(" ");

  function isSvgTag(tag) {
    return SVG_TAGS.indexOf(tag) !== -1;
  }

  // A node is made, in the space it lives in.
  function makeElement(tag, parent) {
    if (!doc) return null;
    var svg = parent && parent.namespaceURI === SVG_NS;
    if (isSvgTag(tag) || svg) {
      if (doc.createElementNS) return doc.createElementNS(SVG_NS, tag);
    }
    return doc.createElement(tag);
  }

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
      // The work of the life of the component belongs to the nodes it
      // gives, so that it is done when they are taken away.
      var works = { mount: [], destroy: [], done: false };
      var previous = ctx.currentComponent;
      ctx.currentComponent = works;
      // The place of the component in the page, so that its values are the
      // same when the page is written again.
      var previousPath = ctx.currentPath;
      var previousIndex = ctx.childIndex;
      var path = previousPath + "/" + previousIndex;
      // The children of this component are counted from the beginning.
      ctx.currentPath = path;
      ctx.childIndex = 0;
      // The values that this component shares with the ones under it.
      var context = new Map();
      ctx.contexts.push(context);
      var result;
      try {
        result = tag.apply(null, callArgs);
      } finally {
        ctx.currentComponent = previous;
        // The place of the parent is the one that was, and it counts this
        // component.
        ctx.currentPath = previousPath;
        ctx.childIndex = previousIndex + 1;
        ctx.contexts.pop();
      }
      var own = Array.isArray(result) ? result : (result ? [result] : []);
      for (var k = 0; k < own.length; k++) {
        var node = own[k];
        if (!node || typeof node !== "object") continue;
        try {
          Object.defineProperty(node, "__veyWorks", {
            value: works,
            writable: true,
            enumerable: false,
            configurable: true
          });
        } catch (err) {
          node.__veyWorks = works;
        }
      }
      return result;
    }

    if (doc) {
      // The marks of the head live in the head of the page, so they are
      // not in the body.
      if (isHeadTag(tag)) {
        headElement(tag, attrs, kids);
        return doc.createTextNode("");
      }
      // The parent is not known here, so the space of the node is the one
      // of its tag.
      var el = makeElement(tag, null);
      applyProps(el, attrs);
      for (var j = 0; j < kids.length; j++) {
        el.appendChild(toDom(kids[j], el));
      }
      return el;
    }

    return { __vey: true, tag: tag, props: attrs, children: kids };
  }

  // The tags that belong to the head of the page.
  //
  // They are made once and then changed, so that the head of the page
  // does not fill up with the same mark again and again.
  var HEAD_TAGS = {
    title: 1,
    meta: 1,
    link: 1,
    base: 1
  };

  // The elements of the head, by their name.
  var headMarks = {};

  function isHeadTag(tag) {
    return HEAD_TAGS[tag] === 1;
  }

  // The element of the head is made once, and then it is changed.
  function headElement(tag, props, children) {
    var name = tag;
    if (props && props.key !== undefined && props.key !== null) {
      name = tag + ":" + props.key;
    } else if (props && props.name !== undefined && props.name !== null) {
      name = tag + ":" + props.name;
    }
    var el = headMarks[name];
    if (!el) {
      el = doc.createElement(tag);
      headMarks[name] = el;
      var head = doc.head || doc.documentElement;
      if (head) head.appendChild(el);
    }
    var attrs = {};
    for (var key in props || {}) {
      if (key === "key" || key === "name") continue;
      attrs[key] = props[key];
    }
    applyProps(el, attrs);
    // The text of the mark is written again, so that it follows the
    // state of the page.
    el.textContent = textOf(children);
    return el;
  }

  // The nodes that are written outside the place they stand in.
  //
  // A dialogue, a menu and a tooltip are written at the end of the page, so
  // that the box they belong to does not cut them. The box of a portal is
  // kept by its name, so that the page may be written again without it
  // being made a second time.
  var portals = {};
  var portalIndex = 0;

  // The portals of the page are waited for, before it is written again.
  function beginPortals() {
    for (var key in portals) portals[key].used = false;
    portalIndex = 0;
  }

  // A portal that the new page does not hold is taken away.
  function endPortals() {
    for (var key in portals) {
      var held = portals[key];
      if (held.used) continue;
      if (held.box.parentNode) held.box.parentNode.removeChild(held.box);
      delete portals[key];
    }
  }

  // Every portal is taken away, when the page is taken away.
  function clearPortals() {
    for (var key in portals) {
      var held = portals[key];
      if (held.box.parentNode) held.box.parentNode.removeChild(held.box);
    }
    portals = {};
  }

  // The way a node comes to the page and goes from it.
  //
  // The value of the node is written as `from` at first, and then taken to
  // `to` in the time that is given. A node that goes is taken to `leave`
  // before it is taken away, and the work is done when the time is over, so
  // that the visitor sees it.
  function transition(duration, options) {
    var opts = options || {};
    var ms = Number(duration);
    if (!(ms >= 0)) ms = 150;
    var easing = opts.easing === undefined ? "ease" : String(opts.easing);
    var from = opts.from === undefined ? { opacity: 0 } : opts.from;
    var to = opts.to === undefined ? { opacity: 1 } : opts.to;
    var leaving = opts.leave === undefined ? { opacity: 0 } : opts.leave;
    var timer = ctx.hostSetTimeout || setTimeout;

    // The values of the styles are written in the node.
    function write(node, values) {
      if (!node || !node.style || !values) return;
      for (var name in values) {
        if (Object.prototype.hasOwnProperty.call(values, name)) {
          node.style[name] = String(values[name]);
        }
      }
    }

    // The node is taken from one value to the other in the time that was
    // given, and the work is done when it is there.
    function animate(node, values, first) {
      return new Promise(function (resolve) {
        if (!node || !node.style || ms === 0) {
          write(node, first);
          write(node, values);
          resolve();
          return;
        }
        node.style.transition = "all " + ms + "ms " + easing;
        write(node, first);
        // The value the node is going to is written in the next frame, so
        // that the browser has a value to move from.
        timer(function () {
          write(node, values);
          timer(function () {
            // The move is over, so the value is the one it went to.
            if (node && node.style && values) node.style.transition = "";
            resolve();
          }, ms);
        }, 0);
      });
    }

    return {
      enter: function (node) {
        return animate(node, to, from);
      },
      leave: function (node) {
        return animate(node, leaving, null);
      }
    };
  }

  // A value that is written in another place of the page.
  //
  // The target is an element or the name of one. A portal with a name of its
  // own keeps its place in the page, so that two of them do not write in the
  // same box.
  function portal(content, target, key) {
    return {
      __veyPortal: true,
      content: content,
      target: target,
      key: key === undefined || key === null ? null : String(key)
    };
  }

  // The box of a portal holds the nodes of the page.
  function portalBox(value) {
    if (!doc) return null;
    var key = value.key === null ? "portal:" + portalIndex++ : "portal:" + value.key;
    var held = portals[key];
    if (!held) {
      var place =
        typeof value.target === "string" ? doc.querySelector(value.target) : value.target;
      // A target that is not there yet is not a place to write in.
      if (!place) return null;
      var box = doc.createElement("div");
      box.setAttribute("data-vey", "portal");
      if (value.key !== null) box.setAttribute("data-vey-key", value.key);
      place.appendChild(box);
      held = { box: box, used: false };
      portals[key] = held;
    }
    held.used = true;
    var content = Array.isArray(value.content) ? value.content : [value.content];
    var nodes = [];
    for (var i = 0; i < content.length; i++) nodes.push(toDom(content[i], held.box));
    if (typeof ctx.patchChildren === "function") {
      // The box is written the way the rest of the page is written, so that
      // the fields of a dialogue keep what the visitor wrote in them.
      ctx.patchChildren(held.box, nodes);
    } else {
      held.box.replaceChildren.apply(held.box, nodes);
    }
    return held.box;
  }

  // The words of a value, as they are written in the page.
  function textOf(value) {
    if (value === null || value === undefined || value === false) return "";
    if (typeof value === "string" || typeof value === "number") return String(value);
    if (Array.isArray(value)) {
      var words = [];
      for (var i = 0; i < value.length; i++) words.push(textOf(value[i]));
      return words.join("");
    }
    if (value.__vey) return textOf(value.children || []);
    return "";
  }

  // Whether a value is a signal, so that it may be tied to a field.
  function isSignal(value) {
    return !!value
      && typeof value === "object"
      && typeof value.subscribe === "function"
      && typeof value.set === "function";
  }

  // Tie the state of a box to a signal.
  //
  // The box is checked when the signal is true, and the signal takes the
  // state of the box when it is clicked.
  function bindChecked(el, signal) {
    if (!isSignal(signal)) return false;
    var shown = signal.value === true;
    if (el.checked !== shown) el.checked = shown;
    el.__veyUnbind = signal.subscribe(function (value) {
      var next = value === true;
      if (el.checked !== next) el.checked = next;
    });
    function take() {
      signal.set(el.checked);
    }
    el.addEventListener("change", take);
    return true;
  }

  // Tie the value of a field to a signal.
  //
  // The field shows the value of the signal, and the signal takes the
  // value of the field when it is written.
  function bindValue(el, signal) {
    if (!isSignal(signal)) return false;
    var shown = signal.value === undefined || signal.value === null ? "" : String(signal.value);
    if (el.value !== shown) el.value = shown;
    // The subscription of the last time is let go, so that the signal does
    // not keep the field of a page that is gone.
    if (typeof el.__veyUnbind === "function") el.__veyUnbind();
    el.__veyUnbind = signal.subscribe(function (value) {
      var next = value === undefined || value === null ? "" : String(value);
      if (el.value !== next) el.value = next;
    });
    function take() {
      signal.set(el.value);
    }
    el.addEventListener("input", take);
    el.addEventListener("change", take);
    return true;
  }

  // The classes of a node, as they are written in the attribute.
  //
  // A list of classes is joined with a space, as the class of an element
  // is a list of names.
  function classText(value) {
    if (!Array.isArray(value)) return value;
    var out = [];
    for (var i = 0; i < value.length; i++) {
      var item = value[i];
      if (item === null || item === undefined || item === false || item === "") continue;
      out.push(String(item));
    }
    return out.join(" ");
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
        el.setAttribute("class", classText(value));
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
      if (key === "value") {
        // The value of a field may follow a signal, and the signal may
        // follow the field.
        if (bindValue(el, value)) continue;
        el.__veyHasValue = true;
        el.__veyValue = value;
        el.value = value;
        continue;
      }
      if (key === "checked") {
        // A box may follow a signal as well.
        if (bindChecked(el, value)) continue;
        el.__veyHasChecked = true;
        el.__veyChecked = value;
        el.checked = value;
        continue;
      }
      // The work that is done when a node comes and when it goes.
      if (key === "enter" || key === "leave") {
        // A value that comes and goes, such as the one of `transition`, has
        // the work of both of its parts, so it may be given as it is.
        var work = null;
        if (typeof value === "function") {
          work = value;
        } else if (value && typeof value[key] === "function") {
          work = value[key];
        }
        if (work) {
          Object.defineProperty(el, "__vey" + key.charAt(0).toUpperCase() + key.slice(1), {
            value: work,
            writable: true,
            enumerable: false,
            configurable: true
          });
        }
        continue;
      }
      // The marks of the space of SVG have a space of their own.
      if (key.indexOf("xlink:") === 0) {
        if (el.setAttributeNS) {
          el.setAttributeNS("http://www.w3.org/1999/xlink", key, String(value));
        } else {
          el.setAttribute(key, String(value));
        }
        continue;
      }
      // A mark of the accessibility that is true or false is written as it
      // is, so that a screen reader hears it.
      if (value === true || value === false) {
        el.setAttribute(key, value ? "true" : "false");
        continue;
      }
      el.setAttribute(key, value === true ? "" : String(value));
    }
  }

  function toDom(value, parent) {
    if (value === null || value === undefined) return doc.createTextNode("");
    if (typeof value === "string" || typeof value === "number") {
      return doc.createTextNode(String(value));
    }
    if (typeof value === "function") {
      // A function is not a text of the page, so it is written as nothing.
      return doc.createTextNode("");
    }
    if (Array.isArray(value)) {
      var fragment = doc.createDocumentFragment();
      for (var i = 0; i < value.length; i++) {
        fragment.appendChild(toDom(value[i], parent));
      }
      return fragment;
    }
    if (value.nodeType) return value;
    if (value.__veyRaw) {
      var template = doc.createElement("template");
      template.innerHTML = String(value.value);
      return template.content;
    }
    if (value.__veyPortal) {
      portalBox(value);
      // The place of the portal in the page is empty: what it holds is in
      // another place.
      return doc.createTextNode("");
    }
    if (value.__vey) {
      // The marks of the head live in the head of the page.
      if (isHeadTag(value.tag)) {
        headElement(value.tag, value.props || {}, value.children || []);
        return doc.createTextNode("");
      }
      var el = makeElement(value.tag, parent);
      applyProps(el, value.props || {});
      // The work of the life of the node belongs to it, and not to the
      // value it was made of.
      if (value.__veyWorks !== undefined) {
        try {
          Object.defineProperty(el, "__veyWorks", {
            value: value.__veyWorks,
            writable: true,
            enumerable: false,
            configurable: true
          });
        } catch (err) {
          el.__veyWorks = value.__veyWorks;
        }
      }
      var children = value.children || [];
      for (var j = 0; j < children.length; j++) {
        el.appendChild(toDom(children[j], el));
      }
      return el;
    }
    return doc.createTextNode(String(value));
  }

  function html(node) {
    if (node === null || node === undefined || node === false || node === true) return "";
    if (typeof node === "function") return "";
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
      if (key === "class") {
        attrs += " class=\"" + escapeHtml(classText(value)) + "\"";
        continue;
      }
      if (key === "style" && typeof value === "object") {
        var styles = ownProps(value);
        var css = "";
        for (var name in styles) css += cssPropertyName(name) + ": " + styles[name] + "; ";
        attrs += " style=\"" + escapeHtml(css) + "\"";
        continue;
      }
      if (key === "value" && isSignal(value)) {
        var shown = value.value === undefined || value.value === null ? "" : String(value.value);
        attrs += " value=\"" + escapeHtml(shown) + "\"";
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

  // A value that the components under this one may read.
  //
  // The value is kept by the component that gives it, so that the ones
  // under it may read it without it being given to each of them.
  function provide(name, value) {
    var context = ctx.contexts[ctx.contexts.length - 1];
    if (context) context.set(String(name), value);
    return value;
  }

  // The value that the component above gave.
  function inject(name) {
    var key = String(name);
    for (var i = ctx.contexts.length - 1; i >= 0; i--) {
      var context = ctx.contexts[i];
      if (context.has(key)) return context.get(key);
    }
    return null;
  }

  // The content of a slot.
  //
  // The content that the caller gives takes the place of the content of
  // the slot. When the caller gives nothing, the content of the slot is
  // there.
  function slotContent(given, fallback) {
    if (given === undefined || given === null) return fallback;
    return given;
  }

  // The component that catches the mistakes of the ones under it.
  //
  // The compiler writes the children of a boundary inside a piece of code
  // that holds them, so that a mistake in one of them does not take the
  // whole page away.
  function Boundary(props) {
    var named = props || {};
    return named.children ? named.children : [];
  }

  // The page that takes the place of a mistake.
  //
  // The fallback may be a component or a value. When there is none, the
  // mistake is said in the console and the page is empty.
  function fallback(err, replace) {
    log.error(err);
    if (replace === null || replace === undefined) return [];
    if (typeof replace === "function") {
      return [h(replace, null, [])];
    }
    return [replace];
  }

  // A value of a component, which is kept when the page is written again.
  //
  // The value is made the first time the component is written, and the
  // same value is given every time after that.
  function state(index, make) {
    var key = (ctx.currentPath || "") + "#" + index;
    var states = ctx.states;
    if (states.has(key)) return states.get(key);
    var value = make();
    states.set(key, value);
    return value;
  }

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
  ctx.portal = portal;
  ctx.transition = transition;
  ctx.beginPortals = beginPortals;
  ctx.endPortals = endPortals;
  ctx.clearPortals = clearPortals;

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
    // A value of a component, which is kept when the page is written
    // again.
    __state: state,
    // The page that takes the place of a mistake.
    __fallback: fallback,
    // The content of a slot.
    __slot: slotContent,
    // The values that the components share.
    provide: provide,
    inject: inject,
    // A value that is written in another place of the page.
    portal: portal,
    // The way a node comes to the page and goes from it.
    transition: transition,
    // The component that catches the mistakes of the ones under it.
    Boundary: Boundary,
    withKey: withKey,
    html: html,
    raw: raw,
    bind: bind,
    cls: cls,
    clipboard: clipboard,
    title: title,
  };
}
