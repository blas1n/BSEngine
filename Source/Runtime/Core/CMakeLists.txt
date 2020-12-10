# Source/Runtime/Core

register_module ()

target_link_libraries (Core PUBLIC BSMath fmt::fmt rapidJSON PRIVATE spdlog::spdlog)