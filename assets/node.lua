node.alias("gdq")

gl.setup(NATIVE_WIDTH, NATIVE_HEIGHT)

local json = require "json"
local text = require "text"

util.resource_loader{
    "dejavu_sans.ttf"
}

local write = text{font=dejavu_sans, width=WIDTH, height=HEIGHT, r=0, g=0, b=0}

local base_time = N.base_time or 0

util.data_mapper{
    ["time/set"] = function(time)
        base_time = tonumber(time) - sys.now()
        N.base_time = base_time
    end;
}

local hostname = nil
local loading = nil
local mode = nil
local schedule = nil

function is_null(value)
    return value == nil or value == json.null
end

function now()
    return base_time + sys.now()
end

util.file_watch("data.json", function(data_text)
    local data = json.decode(data_text) -- don't use json_watch since it's not available in the open-source version
    hostname = data.hostname
    loading = data.loading
    mode = data.mode
    schedule = data.schedule
end)

function node.render()
    gl.clear(0, 0, 0, 1)

    if is_null(mode) then
        gl.clear(1, 0, 0, 1)
        write{text={{"?"}}, size=200, r=1, g=1, b=1}
        return
    elseif mode == "loading" then
        write{text={{hostname}}, r=1, g=1, b=1}
        write{text={loading}, size=50, valign="bottom", r=1, g=1, b=1}
    elseif mode == "schedule" then
        gl.clear(1, 1, 1, 1)
        local y = 0
        for i = 1, #schedule do
            if schedule[i].start_time + schedule[i].run_time + schedule[i].setup_time > now() then
                local dimensions = write{text=schedule[i].game, halign="left", valign="top", min_y=y, simulate=true}
                local next_y = y + dimensions.height
                if now() > schedule[i].start_time then
                    resource.create_colored_texture(0, 1, 0, 1):draw(0, y, WIDTH * (now() - schedule[i].start_time) / schedule[i].run_time, next_y)
                end
                write{text=schedule[i].game, halign="left", valign="top", min_y=y}
                y = next_y
            end
        end
    else
        gl.clear(1, 0, 0, 1)
        write{text={{"unknown", "mode:", mode}}, r=1, g=1, b=1}
    end
end
