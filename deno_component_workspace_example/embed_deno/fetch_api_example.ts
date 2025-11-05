// deno -A fetch_api_example.ts

async function main() {
  const url = "https://www.baidu.com";
  console.log(`Fetching: ${url}\n`);

  try {
    const res = await fetch(url, {
      method: "GET",
      headers: {
        "User-Agent": "curl/8.6.0", // 模拟 curl UA，防止返回压缩或脚本重定向
        Accept: "*/*",
      },
    });

    console.error(`HTTP ${res.status} ${res.statusText}`);

    const body = await res.text();

    // Return structured data back to Rust
    return JSON.stringify(
      {
        url: url,
        status: res.status,
        statusText: res.statusText,
        bodyLength: body.length,
        body: body,
      },
      null,
      2,
    );
  } catch (err) {
    return JSON.stringify(
      {
        error: String(err),
      },
      null,
      2,
    );
  }
}

// Await main() - Rust wrapper will capture the return value
let result = await main();
console.log(result);
return result;
