import { addToHostState } from "builtin:state";

export default async function demo() {
  // Try multiple IP services with fallback
  const ipServices = [
    { url: "https://httpbin.org/ip", parser: (data: any) => data.origin },
    { url: "https://api.ipify.org", parser: (data: string) => data.trim() },
  ];

  for (const service of ipServices) {
    try {
      console.log(`Trying ${service.url}...`);
      const response = await fetch(service.url);
      const data =
        await (service.url.includes("json") || service.url.includes("httpbin")
          ? response.json()
          : response.text());
      const ip = service.parser(data);
      console.log("IP:", ip);
      break; // Success, exit loop
    } catch (error) {
      console.log(`Failed to get IP from ${service.url}:`, error.message);
    }
  }

  console.log("State:", await addToHostState(2));
  console.log("State:", await addToHostState(3));
}
