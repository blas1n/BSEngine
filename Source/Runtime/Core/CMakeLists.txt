# Source/Runtime/Core

register_library ()

target_link_libraries (Core PUBLIC BSMath fmt::fmt rapidJSON PRIVATE spdlog::spdlog utf8cpp)