-- tests/fixtures/sample_workspace/lua_app/utils/helper.lua

local Helper = {}

function Helper.greet(name)
    return "Hello " .. (name or "World")
end

function Helper.calculate_sum(a, b)
    return a + b
end

return Helper
