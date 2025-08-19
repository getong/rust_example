import * as http from "node:http"

http.createServer((req, res) => {
	console.log(req.rawHeaders)
	console.log(req.rawHeaders.toString())
	console.log(JSON.stringify(req.rawHeaders))
	console.log(typeof req.rawHeaders)
	console.log(Object.keys(req.rawHeaders))
	console.log(Object.getOwnPropertyNames(req.rawHeaders))
	console.log(Array.isArray(req.rawHeaders))
	console.log(Object.getPrototypeOf(req.rawHeaders))
	console.log("-------------")
	res.end()
}).listen(80)