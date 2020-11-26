# Source/Runtime/Core

register_module ()

target_link_libraries (${MODULE_ID} PUBLIC BSMath PRIVATE spdlog::spdlog fmt::fmt)