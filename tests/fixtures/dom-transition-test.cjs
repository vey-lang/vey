// A page whose node comes in and goes out little by little.
//
// The node is written with the value it begins with, then with the value it
// ends with, and it stays in the page until the time of the going out is
// over.

const bundle = process.argv[2];
const { root, waitFor, find } = require("./dom-fake.cjs");

require(bundle);

const span = () => find(root, "span");

waitFor(
  () => span() && span().style.opacity === "1",
  () => "the node did not come in: " + JSON.stringify(span() && span().style),
  () =>
    waitFor(
      () => span() === null,
      () => "the node is still there after it went out",
      () => console.log("DOM_TRANSITION_TEST_OK")
    )
);
