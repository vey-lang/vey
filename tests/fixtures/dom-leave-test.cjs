// The goodbye of a node.
//
// A node that is taken away may say goodbye first, so that a transition may
// be seen. The node waits until the work is done.

const bundle = process.argv[2];
const { root, waitFor } = require("./dom-fake.cjs");

require(bundle);

waitFor(
  () => root.textContent.includes("here"),
  () => "the node is missing: " + root.textContent,
  () => {
    waitFor(
      () => !root.textContent.includes("here"),
      () => "the node was not taken away: " + root.textContent,
      () => console.log("DOM_LEAVE_TEST_OK")
    );
  }
);
