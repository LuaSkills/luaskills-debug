-- Return one greeting; replace it with your first real tool.
-- 返回一条问候语；请替换为你的第一个真实工具。

--- Resolve the name greeted by the tool.
--- 解析工具要问候的名称。
--- @param args table|nil
--- @return string
local function greeting_name(args)
  if type(args) == "table" and type(args.name) == "string" and args.name ~= "" then
    return args.name
  end
  return "world"
end

--- Main tool entry for the minimal hello tool.
--- 最小 hello 工具的主入口。
--- @param args table|nil
--- @return string
return function(args)
  return vulcan.json.encode({
    ok = true,
    message = "Hello, " .. greeting_name(args) .. "!",
  })
end
