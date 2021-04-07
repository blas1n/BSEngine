# Source/Runtime/Core

register_library ()

target_link_libraries (Core INTERFACE BSMath PUBLIC fmt::fmt rapidJSON PRIVATE spdlog::spdlog)