# Source/Runtime/Core

register_module ()

target_link_libraries (Core PUBLIC BSMath PRIVATE spdlog::spdlog fmt::fmt)