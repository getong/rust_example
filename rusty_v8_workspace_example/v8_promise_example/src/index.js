async function add(a, b) {
  return new Promise((resolve, reject) => {
    if (a > b) {
      resolve(a + b);
    } else {
      reject(new Error(`Condition not met: ${a} must be greater than ${b}`));
    }
  });
}