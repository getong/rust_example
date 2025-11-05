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
    console.log(body);
  } catch (err) {
    console.error("Error:", err);
  }
}

main();
