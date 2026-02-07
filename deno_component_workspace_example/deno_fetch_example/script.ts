async function main() {
  console.log("Fetching from API...");
  try {
    const response = await fetch("https://www.baidu.com");
    const text = await response.text();
    console.log("baidu Response:", text);
  } catch (err) {
    console.error("Fetch failed:", err);
  }
}

main();
