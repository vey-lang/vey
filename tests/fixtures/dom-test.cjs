// The page of a program: it is written, and it is written again.

const bundle = process.argv[2];
const { root, find, waitFor } = require("./dom-fake.cjs");

require(bundle);

waitFor(
  () => {
    const button = find(root, "button");
    if (!button) return false;
    if (button.attributes.class !== "btn") throw new Error("class attribute missing");
    return root.textContent.includes("Clicked 0 times");
  },
  () => "the page is not written: " + root.textContent,
  () => {
    const button = find(root, "button");
    button.listeners.click[0]();
    waitFor(
      () => root.textContent.includes("Clicked 1 times"),
      () => "the page was not written again: " + root.textContent,
      () => console.log("DOM_TEST_OK")
    );
  }
);
