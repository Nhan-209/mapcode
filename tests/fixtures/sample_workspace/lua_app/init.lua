-- tests/fixtures/sample_workspace/lua_app/init.lua

local helper = require("utils.helper")

function main()
    local msg = helper.greet("MapCode")
    print(msg)
    local result = helper.calculate_sum(10, 20)
    print("Sum: " .. result)
end

main()
