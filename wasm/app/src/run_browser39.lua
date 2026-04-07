local io = require("io")
local funcs = require("funcs")
local http = require("http_client")

local function main()
	local url = "https://nullonerror.org/2026/02/13/hiding-information-inside-images/"
	local selector = "article"

	io.print("browser39-wasm (scraper): fetching " .. url)

	local response, err = http.get(url, {
		headers = { ["User-Agent"] = "browser39-wasm/0.1", ["Accept"] = "text/html,*/*" },
	})
	if err then io.print("HTTP ERROR: " .. tostring(err)); return 1 end

	local html = response.body
	io.print("fetched " .. #html .. " bytes, status " .. tostring(response.status_code))
	io.print("converting with selector: " .. selector)
	io.print("")

	local result, wasm_err = funcs.call("app.browser39:html_to_markdown", html, selector)
	if wasm_err then io.print("WASM ERROR: " .. tostring(wasm_err)); return 1 end

	io.print(tostring(result))
	return 0
end

return { main = main }
