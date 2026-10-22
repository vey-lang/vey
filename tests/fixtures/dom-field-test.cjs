// The value of a field and the state of the program.
//
// The field shows the value of the signal, and the signal takes the value
// of the field when it is written.

const bundle = process.argv[2];
const { root, find, waitFor } = require("./dom-fake.cjs");

require(bundle);

waitFor(
  () => {
    const input = find(root, "input");
    if (!input) return false;
    if (input.value !== "Ada") throw new Error("the field does not show the value: " + input.value);
    return true;
  },
  () => "the field is missing",
  () => {
    const input = find(root, "input");
    // The visitor writes something else.
    input.value = "Grace";
    if (!input.listeners.input) throw new Error("the field does not listen");
    input.listeners.input[0]();
    waitFor(
      () => input.value === "Grace",
      () => "the value was lost: " + input.value,
      () => console.log("DOM_FIELD_TEST_OK")
    );
  }
);
